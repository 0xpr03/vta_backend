use chrono::{Timelike, Utc};
use futures::TryStreamExt;
use rand::distributions::Alphanumeric;
use rand::Rng;
use sqlx::{Connection, Executor, MySql, MySqlConnection, Transaction};
use std::borrow::Cow;
use std::collections::{HashMap, HashSet};
use std::iter::repeat;

use super::models::*;
use super::*;
use crate::prelude::*;

//#[instrument(skip(state,data))]
pub async fn update_deleted_lists(
    sql: &mut DbConn,
    data: ListDeletedRequest,
    user: &UserId,
) -> Result<ListDeletedResponse> {
    let t_now = Utc::now().naive_utc();

    let mut transaction = sql.begin().await?;

    update_last_seen(&mut transaction, user, t_now).await?;

    let since = data.since.map(|v| v.with_nanosecond(0));

    let time_cond = if since.is_none() {
        ""
    } else {
        "AND created >= ?"
    };
    let sql_fetch = format!(
        "SELECT list FROM deleted_list
    WHERE user = ? {time}
    UNION
    SELECT list FROM deleted_list_shared
    WHERE user = ? {time}",
        time = time_cond
    );
    let sql_t = sqlx::query_scalar::<_, Uuid>(sql_fetch.as_str());
    let stream = match since {
        Some(time) => {
            dbg!(time);
            sql_t.bind(user.0).bind(time).bind(user.0).bind(time)
        }
        None => sql_t.bind(user.0).bind(user.0),
    }
    .fetch(&mut transaction);

    let mut return_lists: HashSet<Uuid> = stream
        .try_collect()
        .await
        .context("retrieving changes to return")?;

    // four loops to retain statement cache in the transaction connection

    // remove entries without permissions
    // FIXLATER no async in iterators, stream::iter().try_filter_map(|mut v| async move {}) lifetime issues
    let mut unknown = Vec::new();
    let mut unowned = Vec::new();
    let mut filtered = Vec::with_capacity(data.lists.len());
    for v in data.lists.into_iter() {
        // don't process deletions we already know
        if !return_lists.remove(&v) {
            let owner = sqlx::query_as::<_, (Uuid,)>("SELECT owner FROM lists WHERE uuid = ?")
                .bind(v)
                .fetch_optional(&mut transaction)
                .await
                .context("retrieving owner of lists")?;
            if let Some((owner,)) = owner {
                // only owners can delete lists
                if owner == user.0 {
                    filtered.push(v);
                } else {
                    trace!(list=%v,"Ignoring non-owned list deletion request");
                    unowned.push(v);
                }
            } else {
                trace!(list=%v,"Ignoring unknown list deletion request");
                unknown.push(v);
            }
        }
    }
    // add tombstone for owner
    for v in filtered.iter() {
        sqlx::query("INSERT IGNORE INTO deleted_list (user,list,created) VALUES(?,?,?)")
            .bind(user.0)
            .bind(v)
            .bind(t_now)
            .execute(&mut transaction)
            .await
            .context("inserting deleted_list")?;
    }
    // add tombstone for shared users
    for v in filtered.iter() {
        sqlx::query(
            "INSERT INTO deleted_list_shared (user,list,created) 
            SELECT user,list,? FROM list_permissions WHERE list = ?",
        )
        .bind(t_now)
        .bind(v)
        .execute(&mut transaction)
        .await
        .context("inserting deleted_list_shared")?;
    }
    // delete list
    for v in filtered.iter() {
        sqlx::query("DELETE FROM lists WHERE owner = ? AND uuid = ?")
            .bind(user.0)
            .bind(v)
            .execute(&mut transaction)
            .await
            .context("deleting lists")?;
    }

    transaction.commit().await?;

    Ok(ListDeletedResponse {
        delta: return_lists,
        unowned,
        unknown,
    })
}

//#[instrument(skip(state,data))]
pub async fn update_changed_lists(
    sql: &mut MySqlConnection,
    data: ListChangedRequest,
    user: &UserId,
) -> Result<ListChangedResponse> {
    let t_now = Utc::now().naive_utc();
    let mut transaction = sql.begin().await?;
    let since = data.since.map(|v| v.with_nanosecond(0));

    // resolve all changed entries we should send back
    let time_cond_lists = if since.is_none() {
        ""
    } else {
        "AND l.changed >= ?"
    };
    let time_cond_shared = if since.is_none() {
        ""
    } else {
        "AND ( l.changed >= ? OR p.changed >= ?)"
    };
    let sql_fetch_resp = format!(
        "SELECT -1 as permissions,uuid,name,name_a,name_b,changed,created
    FROM lists l WHERE owner = ? {time_cond_lists}
    UNION
    SELECT p.write as permissions,uuid,name,name_a,name_b,l.changed,l.created
    FROM lists l
    JOIN list_permissions p ON p.list = l.uuid
    WHERE p.user = ? {time_cond_shared}
    ",
        time_cond_lists = time_cond_lists,
        time_cond_shared = time_cond_shared
    );
    let sql_t = sqlx::query_as::<_, ListChangedEntrySend>(sql_fetch_resp.as_str());
    let stream = match since {
        Some(time) => sql_t
            .bind(&user.0)
            .bind(time)
            .bind(&user.0)
            .bind(time)
            .bind(time),
        None => sql_t.bind(&user.0).bind(&user.0),
    }
    .fetch(&mut transaction);
    let return_lists: HashMap<Uuid, ListChangedEntrySend> = stream
        .map_ok(|v| (v.uuid, v))
        .try_collect()
        .await
        .context("requesting changes")?;
    trace!(amount = return_lists.len(), "fetched return data");

    let mut failure = Vec::new();
    let mut deleted: Vec<Uuid> = Vec::new(); // TODO: use this actually in the return
    let sql_del_check = format!(
        "SELECT 1 FROM deleted_list d WHERE d.list = ? AND d.user = ?
    UNION
    SELECT 1 FROM deleted_list_shared WHERE list = ? AND user = ?"
    );
    let sql_owner_changed = "SELECT owner,changed FROM lists WHERE uuid = ? FOR UPDATE";
    let sql_foreign_perm = "SELECT `write` FROM list_permissions WHERE list = ? AND list = ?";
    let query_insert_list = "INSERT INTO lists (uuid,name,name_a,name_b,changed,created,owner)
                VALUES (?,?,?,?,?,?,?)";
    let query_update_list =
        "UPDATE lists SET name=?, name_a = ?, name_b = ?, changed = ? WHERE list = ?";
    let amount = data.lists.len();
    let mut updated = 0;
    let mut inserted = 0;
    let mut outdated = 0;
    for v in data.lists.into_iter() {
        if v.changed > t_now {
            info!(%v.changed,%t_now,"ignoring change date in future");
            failure.push(EntrySyncFailure {
                id: v.uuid,
                error: Cow::Owned(format!(
                    "Invalid changed date: {} current time: {}",
                    v.changed, t_now
                )),
            });
            continue;
        }
        // remove entries for deleted lists ones
        let res: Option<bool> = sqlx::query_scalar(sql_del_check.as_str())
            .bind(v.uuid)
            .bind(&user.0)
            .bind(v.uuid)
            .bind(&user.0)
            .fetch_optional(&mut transaction)
            .await
            .context("checking tombstones")?;
        if res.is_some() {
            deleted.push(v.uuid);
            continue;
        }
        let res = sqlx::query_as::<_, (Uuid, Timestamp)>(sql_owner_changed)
            .bind(v.uuid)
            .fetch_optional(&mut transaction)
            .await
            .context("fetching owner + changed")?;
        if let Some((owner, changed)) = res {
            // check permissions
            if owner != user.0 {
                let change_perm = sqlx::query_scalar(sql_foreign_perm)
                    .bind(v.uuid)
                    .bind(&user.0)
                    .fetch_optional(&mut transaction)
                    .await
                    .context("fetching foreign perms")?;
                if change_perm != Some(true) {
                    failure.push(EntrySyncFailure {
                        id: v.uuid,
                        error: Cow::Borrowed("missing permissions"),
                    });
                    continue;
                }
            }
            // remove outdated
            if v.changed <= changed {
                outdated += 1;
                continue;
            }

            // TODO: remove from return lists

            // update existing list
            sqlx::query(query_update_list)
                .bind(v.name)
                .bind(v.name_a)
                .bind(v.name_b)
                .bind(v.changed)
                .bind(v.uuid)
                .execute(&mut transaction)
                .await
                .context("updating list")?;

            updated += 1;
        } else {
            // insert new list
            sqlx::query(query_insert_list)
                .bind(v.uuid)
                .bind(v.name)
                .bind(&v.name_a)
                .bind(&v.name_b)
                .bind(&v.changed)
                .bind(v.created)
                .bind(&user.0)
                .execute(&mut transaction)
                .await
                .context("inserting list")?;
            inserted += 1;
        }
    }
    trace!(
        changed = updated,
        new = inserted,
        outdated = outdated,
        from = amount,
        "filtered deleted"
    );

    transaction.commit().await?;

    trace!(
        "Found {} changes to send back, failures {}",
        return_lists.len(),
        failure.len()
    );
    let response = ListChangedResponse {
        delta: return_lists,
        failures: failure,
    };

    Ok(response)
}

//#[instrument(skip(state,data))]
pub async fn update_deleted_entries(
    sql: &mut MySqlConnection,
    data: EntryDeletedRequest,
    user: &UserId,
) -> Result<EntryDeletedResponse> {
    let t_now = Utc::now().naive_utc();

    let mut transaction = sql.begin().await?;
    let since = data.since.map(|v| v.with_nanosecond(0));

    // first retrieve deleted entries to send back
    let time_addition = if since.is_some() {
        "AND d.created >= ?"
    } else {
        ""
    };
    let sql_fetch = format!(
        "SELECT d.list,d.entry FROM deleted_entry d
        JOIN lists l ON d.list = l.uuid
        WHERE l.owner = ? {time}
        UNION
        SELECT d.list,d.entry FROM deleted_entry d
        JOIN list_permissions p ON d.list = p.list
        WHERE p.user = ? {time}",
        time = time_addition
    );
    let q = sqlx::query_as::<_, EntryDeleteEntry>(sql_fetch.as_str());
    let stream = match since {
        // Not for update
        Some(time) => q.bind(user.0).bind(time).bind(user.0).bind(time),
        None => q.bind(user.0).bind(user.0),
    }
    .fetch(&mut transaction);

    let mut return_delta: HashMap<Uuid, EntryDeleteEntry> = stream
        .map_ok(|v| (v.entry, v))
        .try_collect()
        .await
        .context("fetching deleted_entry to send back")?;
    trace!(affected = return_delta.len(), "fetched send-back");

    let sqlt_deleted_list = "SELECT 1 from `deleted_list` WHERE list = ?";
    let sqlt_owner = "SELECT owner FROM lists WHERE uuid = ?";
    let sqlt_perms_shared = "SELECT `write` FROM list_permissions WHERE list = ? AND user = ?";
    let sqlt_delete_entry = "DELETE FROM entries WHERE uuid = ?";
    let sqlt_tombstone = "INSERT INTO deleted_entry (list,`entry`,created) VALUES (?,?,?)";
    let mut list_deleted = HashSet::new();
    // map of lists and whether we have change permissions
    let mut list_perm: HashMap<Uuid, bool> = HashMap::new();
    let mut invalid = Vec::new();
    let mut ignored = Vec::new();
    for e in data.entries.into_iter() {
        // remove entries from return data that we got already send
        // if not in return set, insert to temp table, otherwise known
        if return_delta.remove(&e.entry).is_some() {
            continue;
        }
        // check list not deleted
        if list_deleted.contains(&e.list) {
            ignored.push(e.entry);
            continue;
        }
        let res = sqlx::query(sqlt_deleted_list)
            .bind(e.list)
            .fetch_optional(&mut transaction)
            .await
            .context("fetching list deletion")?;
        if res.is_some() {
            list_deleted.insert(e.list);
            ignored.push(e.entry);
            continue;
        }

        // check permissions
        if let Some(has_perm) = list_perm.get(&e.list) {
            // cached value
            match has_perm {
                true => (),
                false => {
                    invalid.push(e.entry);
                    continue;
                }
            }
        } else {
            let res: Option<Uuid> = sqlx::query_scalar(sqlt_owner)
                .bind(e.list)
                .fetch_optional(&mut transaction)
                .await
                .context("fetching list owner")?;
            if let Some(owner) = res {
                if owner != user.0 {
                    // not owner, check for write perm
                    let res: Option<bool> = sqlx::query_scalar(sqlt_perms_shared)
                        .bind(e.list)
                        .bind(user.0)
                        .fetch_optional(&mut transaction)
                        .await
                        .context("fetching shared write perms")?;
                    let has_perms = res == Some(true);
                    // cache
                    list_perm.insert(e.list.clone(), has_perms);
                    if !has_perms {
                        invalid.push(e.entry);
                        continue;
                    }
                } else {
                    // cache
                    list_perm.insert(e.list.clone(), true);
                }
            }
        }

        // delete entry
        let res = sqlx::query(sqlt_delete_entry)
            .bind(e.entry)
            .execute(&mut transaction)
            .await
            .context("deleting entry")?;
        if res.rows_affected() != 0 {
            // tombstone only for existing entries
            sqlx::query(sqlt_tombstone)
                .bind(e.list)
                .bind(e.entry)
                .bind(t_now)
                .execute(&mut transaction)
                .await
                .context("inserting tombstone")?;
        } else {
            ignored.push(e.entry);
        }
    }

    transaction.commit().await?;

    Ok(EntryDeletedResponse {
        delta: return_delta,
        ignored,
        invalid,
    })
}

//#[instrument(skip(state,data))]
pub async fn update_changed_entries(
    sql: &mut MySqlConnection,
    data: EntryChangedRequest,
    user: &UserId,
) -> Result<EntryChangedResponse> {
    let mut transaction = sql.begin().await?;
    let res = _update_changed_entries(&mut transaction, data, user).await;
    if res.is_ok() {
        transaction.commit().await?;
    } else {
        transaction.rollback().await?;
    }
    res
}

async fn _update_changed_entries(
    transaction: &mut Transaction<'_, MySql>,
    data: EntryChangedRequest,
    user: &UserId,
) -> Result<EntryChangedResponse> {
    let t_now = Utc::now().naive_utc();
    let since = data.since.map(|v| v.with_nanosecond(0));

    // fetch data to return
    // don't request meanings already, we can do that after checking for newer data in the payload
    let time_addition = if since.is_some() {
        "AND e.updated >= ?"
    } else {
        ""
    };
    let sql_t = format!(
        "SELECT e.list,e.uuid,e.changed,tip FROM entries e
    JOIN list_permissions p ON e.list = p.list
    WHERE p.user = ? {time}
    UNION
    SELECT e.list,e.uuid,e.changed,tip FROM entries e
    JOIN lists l ON e.list = l.uuid
    WHERE l.owner = ? {time}",
        time = time_addition
    );
    let q = sqlx::query_as::<_, EntryChangedEntryBlank>(sql_t.as_str());
    let stream = match since {
        Some(time) => q.bind(user.0).bind(time).bind(user.0).bind(time),
        None => q.bind(user.0).bind(user.0),
    }
    .fetch(&mut *transaction);
    // don't fetch meanings here, postponed after handling incoming changes
    let mut delta_entries_raw: HashMap<Uuid, EntryChangedEntryBlank> = stream
        .map_ok(|v| (v.uuid, v))
        .try_collect()
        .await
        .context("requesting changes")?;
    trace!(
        amount = delta_entries_raw.len(),
        "retrieved changes to send back"
    );

    let mut list_perm: HashMap<Uuid, bool> = HashMap::new();
    let mut list_not_existing: HashSet<Uuid> = HashSet::new();
    let mut ignored = Vec::new();
    let mut invalid = Vec::new();

    let sqlt_owner = "SELECT owner FROM lists WHERE uuid = ?";
    let sqlt_perms_shared = "SELECT `write` FROM list_permissions WHERE list = ? AND user = ?";
    let sqlt_update_entry = "UPDATE entries SET tip = ?, changed = ?, updated = ? WHERE uuid = ?";
    let sqlt_entry_deleted = "SELECT 1 FROM deleted_entry WHERE entry = ?";
    let sqlt_entry_changed_date = "SELECT changed FROM entries WHERE uuid = ? FOR UPDATE";
    let sqlt_insert_entry =
        "INSERT INTO entries (list,uuid,changed,updated,tip) VALUES (?,?,?,?,?)";
    let sqlt_delete_meanings = "DELETE FROM entry_meaning WHERE entry = ?";
    let sqlt_insert_meaning = "INSERT INTO entry_meaning (entry,`value`,is_a) VALUES (?,?,?)";

    for e in data.entries.into_iter() {
        if e.changed > t_now {
            info!(%e.changed,%t_now,"ignoring change date in future");
            //v.changed = t_now;
            invalid.push(e.uuid);
            continue;
        }

        if let Some(return_entry) = delta_entries_raw.get(&e.uuid) {
            if return_entry.changed > e.changed {
                // ignore outdated incoming changes
                continue;
            } else {
                // don't send back data with newer incoming changes
                delta_entries_raw.remove(&e.uuid);
            }
        }

        //check permissions
        match list_perm.get(&e.list) {
            Some(true) => (),
            Some(false) => {
                invalid.push(e.list);
                continue;
            }
            None => {
                let res: Option<Uuid> = sqlx::query_scalar(sqlt_owner)
                    .bind(e.list)
                    .fetch_one(&mut *transaction)
                    .await
                    .context("fetching list owner")?;
                if let Some(owner) = res {
                    if owner == user.0 {
                        list_perm.insert(e.list, true);
                    } else {
                        // not owner, check for write perm
                        let res: Option<bool> = sqlx::query_scalar(sqlt_perms_shared)
                            .bind(e.list)
                            .bind(user.0)
                            .fetch_optional(&mut *transaction)
                            .await
                            .context("fetching shared write perms")?;
                        let has_perms = res == Some(true);
                        // cache
                        list_perm.insert(e.list.clone(), has_perms);
                        if !has_perms {
                            invalid.push(e.uuid);
                            continue;
                        }
                    }
                } else {
                    list_not_existing.insert(e.list);
                    ignored.push(e.uuid);
                    continue;
                }
            }
        }
        // check entry isn't deleted
        let res: Option<i32> = sqlx::query_scalar(sqlt_entry_deleted)
            .bind(e.uuid)
            .fetch_optional(&mut *transaction)
            .await
            .context("checking for entry tombstone")?;
        if res.is_some() {
            trace!(%e.uuid,"ignoring deleted entry");
            ignored.push(e.uuid);
            continue;
        }
        // fetch last changed
        let res: Option<Timestamp> = sqlx::query_scalar(sqlt_entry_changed_date)
            .bind(e.uuid)
            .fetch_optional(&mut *transaction)
            .await
            .context("fetching changed date")?;
        if let Some(changed) = res {
            // do not take over outdated entries
            if changed >= e.changed {
                trace!(%e.uuid,%changed,%e.changed,"ignoring outdated entry");
                ignored.push(e.uuid);
                continue;
            }
            sqlx::query(sqlt_update_entry)
                .bind(e.tip)
                .bind(e.changed)
                .bind(t_now)
                .bind(e.uuid)
                .execute(&mut *transaction)
                .await
                .context("updating entry")?;
        } else {
            // or insert new entry
            sqlx::query(sqlt_insert_entry)
                .bind(e.list)
                .bind(e.uuid)
                .bind(e.changed)
                .bind(t_now)
                .bind(e.tip)
                .execute(&mut *transaction)
                .await
                .context("inserting entry")?;
        }
        // now update meanings
        sqlx::query(sqlt_delete_meanings)
            .bind(e.uuid)
            .execute(&mut *transaction)
            .await
            .context("deleting entry meaings")?;
        for m in e.meanings.into_iter() {
            sqlx::query(sqlt_insert_meaning)
                .bind(e.uuid)
                .bind(m.value)
                .bind(m.is_a)
                .execute(&mut *transaction)
                .await
                .context("inserting meaning")?;
        }
    }

    // now fetch the meanings of returned delta
    let mut return_entries = HashMap::with_capacity(delta_entries_raw.len());
    for (id, e) in delta_entries_raw.into_iter() {
        let meanings: Vec<Meaning> =
            sqlx::query_as::<_, Meaning>("SELECT value,is_a FROM entry_meaning WHERE entry = ?")
                .bind(e.uuid)
                .fetch(&mut *transaction)
                .try_collect()
                .await
                .context("fetching meanings")?;
        return_entries.insert(id, e.into_full(meanings));
    }

    Ok(EntryChangedResponse {
        delta: return_entries,
        ignored,
        invalid,
    })
}
