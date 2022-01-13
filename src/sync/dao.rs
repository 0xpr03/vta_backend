use std::borrow::Cow;
use std::collections::{HashSet, HashMap};
use std::iter::repeat;
use chrono::Utc;
use futures::TryStreamExt;
use rand::Rng;
use rand::distributions::Alphanumeric;
use sqlx::{Executor, MySqlConnection, Connection};

use crate::prelude::*;
use super::*;
use super::models::*;

//#[instrument(skip(state,data))]
pub async fn update_deleted_lists(sql: &mut DbConn, data: ListDeletedRequest, user: &Uuid) -> Result<ListDeletedResponse> {
    let t_now = Utc::now().naive_utc();

    let mut transaction = sql.begin().await?;
    
    let last_synced: Option<Timestamp> = last_synced(&mut transaction, user, &data.client, LastSyncedKind::ListsDeleted).await?;

    let time_cond = if last_synced.is_none() {
        ""
    } else {
        "AND time > ?"
    };
    let sql_fetch = format!("SELECT list,time FROM deleted_list
    WHERE user = ? {time}
    UNION
    SELECT list,time FROM deleted_list_shared
    WHERE user = ? {time}",time=time_cond);
    let sql_t = sqlx::query_as::<_,ListDeleteEntry>(sql_fetch.as_str());
    let stream = match last_synced {
        Some(time) => sql_t.bind(user).bind(time).bind(user).bind(time),
        None => sql_t.bind(user).bind(user)
    }.fetch(&mut transaction);

    let mut return_lists: HashMap<Uuid,ListDeleteEntry> = stream.map_ok(|v|(v.list,v))
        .try_collect().await.context("retrieving changes to return")?;
    
    // four loops to retain statement cache in the transaction connection

    // remove entries without permissions
    // FIXLATER no async in iterators, stream::iter().try_filter_map(|mut v| async move {}) lifetime issues
    let mut unknown = Vec::new();
    let mut unowned = Vec::new();
    let mut filtered = Vec::with_capacity(data.lists.len());
    for mut v in data.lists.into_iter() {
        if v.time > t_now {
            info!(%v.time,%t_now,"ignoring change date in future");
            v.time = t_now;
        }
        // don't process deletions we already know
        if return_lists.remove(&v.list).is_none() {
            let owner = sqlx::query_as::<_,(Uuid,)>("SELECT owner FROM lists WHERE uuid = ?")
                .bind(v.list).fetch_optional(&mut transaction).await.context("retrieving owner of lists")?;
            if let Some((owner,)) = owner {
                // only owners can delete lists
                if owner == *user {
                    filtered.push(v);
                } else {
                    trace!(list=%v.list,"Ignoring non-owned list deletion request");
                    unowned.push(v.list);
                }
            } else {
                trace!(list=%v.list,"Ignoring unknown list deletion request");
                unknown.push(v.list);
            }
        }
    }
    // add tombstone for owner
    for v in filtered.iter() {
        sqlx::query("INSERT IGNORE INTO deleted_list (user,list,time) VALUES(?,?,?)")
            .bind(user).bind(v.list).bind(v.time)
            .execute(&mut transaction).await.context("inserting deleted_list")?;
    }
    // add tombstone for shared users
    for v in filtered.iter() {
        sqlx::query("INSERT INTO deleted_list_shared (user,list,`time`) 
            SELECT user,list,? FROM list_permissions WHERE list = ?")
            .bind(v.time).bind(v.list)
            .execute(&mut transaction).await.context("inserting deleted_list_shared")?;
    }
    // delete list
    for v in filtered.iter() {
        sqlx::query("DELETE FROM lists WHERE owner = ? AND uuid = ?")
            .bind(user).bind(v.list)
            .execute(&mut transaction).await.context("deleting lists")?;
    }

    update_last_synced(&mut transaction,user, &data.client, LastSyncedKind::ListsDeleted, t_now).await?;

    transaction.commit().await?;

    Ok(ListDeletedResponse {
        delta: return_lists,
        unowned,
        unknown,
    })
}

//#[instrument(skip(state,data))]
pub async fn update_changed_lists(sql: &mut MySqlConnection, data: ListChangedRequest, user: &UserId) -> Result<ListChangedResponse> {
    let t_now = Utc::now().naive_utc();
    let mut transaction = sql.begin().await?;
    
    let last_synced = last_synced(&mut transaction, &user.0, &data.client, LastSyncedKind::ListsChanged).await?;
    trace!(?last_synced, "Last synced");

    // resolve all changed entries we should send back
    let time_cond_lists = if last_synced.is_none() {
        ""
    } else {
        "AND l.changed > ?"
    };
    let time_cond_shared = if last_synced.is_none() {
        ""
    } else {
        "AND ( l.changed > ? OR p.changed > ?)"
    };
    let sql_fetch_resp = format!("SELECT -1 as permissions,uuid,name,name_a,name_b,changed,created
    FROM lists l WHERE owner = ? {time_cond_lists}
    UNION
    SELECT p.write as permissions,uuid,name,name_a,name_b,l.changed,l.created
    FROM lists l
    JOIN list_permissions p ON p.list = l.uuid
    WHERE p.user = ? {time_cond_shared}
    ",time_cond_lists=time_cond_lists,time_cond_shared=time_cond_shared);
    let sql_t = sqlx::query_as::<_,ListChangedEntrySend>(sql_fetch_resp.as_str());
    let stream = match last_synced {
        Some(time) => sql_t.bind(&user.0).bind(time).bind(&user.0).bind(time).bind(time),
        None => sql_t.bind(&user.0).bind(&user.0)
    }.fetch(&mut transaction);
    let return_lists: HashMap<Uuid,ListChangedEntrySend> = stream.map_ok(|v|(v.uuid,v)).try_collect().await.context("requesting changes")?;
    trace!(amount=return_lists.len(),"fetched return data");
    
    let mut failure = Vec::new();
    let mut deleted: Vec<Uuid> = Vec::new();// TODO: use this actually in the return
    let sql_del_check = format!("SELECT 1 FROM deleted_list d WHERE d.list = ? AND d.user = ?
    UNION
    SELECT 1 FROM deleted_list_shared WHERE list = ? AND user = ?");
    let sql_owner_changed = "SELECT owner,changed FROM lists WHERE uuid = ? FOR UPDATE";
    let sql_foreign_perm = "SELECT `write` FROM list_permissions WHERE list = ? AND list = ?";
    let query_insert_list = "INSERT INTO lists (uuid,name,name_a,name_b,changed,created,owner)
                VALUES (?,?,?,?,?,?,?)";
    let query_update_list = "UPDATE lists SET name=?, name_a = ?, name_b = ?, changed = ? WHERE list = ?";
    let amount = data.lists.len();
    let mut updated = 0;
    let mut inserted = 0;
    let mut outdated = 0;
    for v in data.lists.into_iter() {
        if v.changed > t_now {
            info!(%v.changed,%t_now,"ignoring change date in future");
            failure.push(EntrySyncFailure {
                id: v.uuid,
                error: Cow::Owned(format!("Invalid changed date: {} current time: {}",v.changed, t_now)),
            });
            continue;
        }
        // remove entries for deleted lists ones
        let res: Option<bool> = sqlx::query_scalar(sql_del_check.as_str())
            .bind(v.uuid)
            .bind(&user.0)
            .bind(v.uuid)
            .bind(&user.0)
            .fetch_optional(&mut transaction).await.context("checking tombstones")?;
        if res.is_some() {
            deleted.push(v.uuid);
            continue;
        }
        let res = sqlx::query_as::<_,(Uuid,Timestamp)>(sql_owner_changed)
            .bind(v.uuid).fetch_optional(&mut transaction).await.context("fetching owner + changed")?;
        if let Some((owner,changed)) = res {
            // check permissions
            if owner != user.0 {
                let change_perm = sqlx::query_scalar(sql_foreign_perm)
                .bind(v.uuid).bind(&user.0).fetch_optional(&mut transaction)
                .await.context("fetching foreign perms")?;
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
                .execute(&mut transaction).await.context("updating list")?;
            
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
                .execute(&mut transaction).await.context("inserting list")?;
            inserted += 1;
        }
    }
    trace!(changed=updated,new=inserted,outdated=outdated,from=amount,"filtered deleted");
        
    update_last_synced(&mut transaction, &user.0, &data.client, LastSyncedKind::ListsChanged, t_now).await?;

    transaction.commit().await?;

    trace!("Found {} changes to send back, failures {}", return_lists.len(),failure.len());
    let response = ListChangedResponse {
        delta: return_lists,
        failures: failure
    };
    

    Ok(response)
}

//#[instrument(skip(state,data))]
pub async fn update_deleted_entries(sql: &mut MySqlConnection, mut data: EntryDeletedRequest, user: &UserId) -> Result<EntryDeletedResponse> {
    let t_now = Utc::now().naive_utc();

    let mut transaction = sql.begin().await?;

    let last_synced = last_synced(&mut transaction, &user.0, &data.client, LastSyncedKind::EntriesDeleted).await?;

    // first retrieve deleted entries to send back
    let time_addition = if last_synced.is_some() {
        "AND d.time > ?"
    } else {
        ""
    };
    let sql_fetch = format!(
        "SELECT d.list,d.time,d.entry FROM deleted_entry d
        JOIN lists l ON d.list = l.uuid
        WHERE l.owner = ? {time}
        UNION
        SELECT d.list,d.time,d.entry FROM deleted_entry d
        JOIN list_permissions p ON d.list = p.list
        WHERE p.user = ? {time}",
        time = time_addition);
    let q = sqlx::query_as::<_,EntryDeleteEntry>(sql_fetch.as_str());
    let stream = match last_synced { // Not for update
        Some(time) => q.bind(user.0).bind(time).bind(user.0).bind(time),
        None => q.bind(user.0).bind(user.0)
    }.fetch(&mut transaction);

    let mut return_delta: HashMap<Uuid,EntryDeleteEntry> = stream.map_ok(|v|(v.entry,v))
        .try_collect().await.context("fetching deleted_entry to send back")?;
    trace!(affected=return_delta.len(),"fetched send-back");

    let sqlt_deleted_list = "SELECT 1 from `deleted_list` WHERE list = ?";
    let sqlt_owner = "SELECT owner FROM lists WHERE uuid = ?";
    let sqlt_perms_shared = "SELECT `write` FROM list_permissions WHERE list = ? AND user = ?";
    let sqlt_delete_entry = "DELETE FROM entries WHERE uuid = ?";
    let sqlt_tombstone = "INSERT INTO deleted_entry (list,`entry`,`time`) VALUES (?,?,?)";
    let mut list_deleted = HashSet::new();
    // map of lists and whether we have change permissions
    let mut list_perm: HashMap<Uuid,bool> = HashMap::new();
    let mut invalid = Vec::new();
    let mut ignored = Vec::new();
    for mut e in data.entries.into_iter() {
        if e.time > t_now {
            info!(%e.time,%t_now,"ignoring change date in future");
            e.time = t_now;
        }
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
            .bind(e.list).fetch_optional(&mut transaction).await.context("fetching list deletion")?;
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
                    .bind(e.list).fetch_optional(&mut transaction).await.context("fetching list owner")?;
            if let Some(owner) = res {
                if owner != user.0 {
                    // not owner, check for write perm
                    let res: Option<bool> = sqlx::query_scalar(sqlt_perms_shared)
                        .bind(e.list).bind(user.0).fetch_optional(&mut transaction)
                            .await.context("fetching shared write perms")?;
                    let has_perms = res == Some(true);
                    // cache
                    list_perm.insert(e.list.clone(),res == Some(true));
                    if !has_perms {
                        invalid.push(e.entry);
                        continue;
                    }
                } else {
                    // cache
                    list_perm.insert(e.list.clone(),true);
                }
            }
        }

        // delete entry
        let res = sqlx::query(sqlt_delete_entry).bind(e.entry)
            .execute(&mut transaction).await.context("deleting entry")?;
        if res.rows_affected() != 0 {
            // tombstone only for existing entries
            sqlx::query(sqlt_tombstone).bind(e.list).bind(e.entry).bind(e.time)
                .execute(&mut transaction).await.context("inserting tombstone")?;
        } else {
            ignored.push(e.entry);
        }
    }

    update_last_synced(&mut transaction, &user.0, &data.client, LastSyncedKind::EntriesDeleted, t_now).await?;

    transaction.commit().await?;

    Ok(EntryDeletedResponse {
        delta: return_delta,
        ignored, 
        invalid,
    })
}

//#[instrument(skip(state,data))]
pub async fn update_changed_entries(sql: &mut MySqlConnection, mut data: EntryChangedRequest, user: &Uuid) -> Result<Vec<EntryChangedEntry>> {
    let t_now = Utc::now().naive_utc();
    
    let mut transaction = sql.begin().await?;
    
    // let last_synced: Option<Timestamp> = sqlx::query!("SELECT date FROM last_synced WHERE `type` = ? AND user_id = ? AND client = ? FOR UPDATE",
    //     LastSyncedKind::EntriesChanged as i32, user, &data.client)
    //     .fetch_optional(&mut transaction).await.context("selecting last_synced")?.map(|v|v.date);
    let last_synced = last_synced(&mut transaction,user,&data.client,LastSyncedKind::EntriesChanged).await?;

    // fetch data to return
    // don't request meanings already, we can do that after checking for newer data in the payload
    let time_addition = if last_synced.is_some() {
        "AND e.changed > ?"
    } else {
        ""
    };
    let sql_t = format!("SELECT e.list,e.uuid,e.changed,tip FROM entries e
    JOIN list_permissions p ON e.list = p.list
    WHERE p.user = ? {time}
    UNION
    SELECT e.list,e.uuid,e.changed,tip FROM entries e
    JOIN lists l ON e.list = l.uuid
    WHERE l.owner = ? {time}",time=time_addition);
    let q = sqlx::query_as::<_,EntryChangedEntryBlank>(sql_t.as_str());
    let stream = match last_synced {
        Some(time) => {
            q.bind(user).bind(time).bind(user).bind(time)
        },
        None => {
            q.bind(user).bind(user)
        }
    }.fetch(&mut transaction);
    let _raw_return_entries: Vec<EntryChangedEntryBlank> = stream.try_collect().await.context("requesting changes")?;
    let mut raw_return_entries: HashMap<Uuid, EntryChangedEntryBlank> = _raw_return_entries.into_iter().map(|v|(v.uuid,v)).collect();
    trace!(amount=raw_return_entries.len(),"retrieved changes to send back");

    let mut rng = rand::thread_rng();
    let table_name: String = format!("t_{}",repeat(())
        .map(|()| rng.sample(Alphanumeric))
        .map(char::from)
        .take(7)
        .collect::<String>());
    transaction.execute(format!("CREATE TEMPORARY TABLE {} (
        list BINARY(16) NOT NULL,
        uuid BINARY(16) NOT NULL,
        changed DATETIME NOT NULL,
        tip VARCHAR(127),
        PRIMARY KEY (list,uuid),
        INDEX (list),
        INDEX (uuid),
        INDEX (changed),
        INDEX (uuid,changed)
    )",table_name).as_str()).await.context("ceating temp table")?;
    trace!("created temp table");

    // insert received entries into temp table
    let sql_t = format!("INSERT INTO `{tbl}` (list,uuid,changed,tip) VALUES(?,?,?,?)
        ON DUPLICATE KEY UPDATE changed=VALUES(changed), tip=VALUES(tip)",tbl= table_name);
    for v in data.entries.iter_mut() {
        if v.changed > t_now {
            info!(%v.changed,%t_now,"ignoring change date in future");
            //v.changed = t_now;
            // TODO: track failures for sendback
            continue;
        }
        
        // verify we don't send back outdated stuff
        if let Some(ret_v) = raw_return_entries.get(&v.uuid) {
            if ret_v.changed > v.changed {
                raw_return_entries.remove(&v.uuid);
            }
        }
        sqlx::query(sql_t.as_str())
            .bind(v.list)
            .bind(v.uuid)
            .bind(v.changed)
            .bind(&v.tip)
            .execute(&mut transaction).await.context("inserting data to temp table")?;
    }
    // remove all non-existing/non-owner lists from temp table
    let sqlt_del_nonowned = format!("DELETE FROM `{tbl}` WHERE list NOT IN (
        SELECT l.uuid FROM lists l
        WHERE `{tbl}`.list = l.uuid AND l.owner = ?
        UNION
        SELECT p.list FROM list_permissions p
        WHERE p.list = `{tbl}`.list AND p.`write` = true AND p.user = ?)", tbl = table_name);
    let sql_res = sqlx::query(sqlt_del_nonowned.as_str())
        .bind(user)
        .bind(user)
        .execute(&mut transaction).await.context("removing entries without list permissions")?;
    trace!(affected=sql_res.rows_affected(),"removed entries without permission / existing list");

    // remove outdated
    let query_outdated = format!("DELETE FROM `{tbl}` WHERE uuid IN
        (SELECT uuid FROM entries e WHERE e.`uuid` = `{tbl}`.`uuid` AND e.`changed` >= `{tbl}`.`changed` FOR UPDATE);",tbl = table_name);
    let res = sqlx::query(query_outdated.as_str()).execute(&mut transaction).await.context("removing outdated")?;
    trace!(affected=res.rows_affected(),"removed outdated data");

    // insert entries back
    let query_upsert = format!("INSERT INTO entries (list,uuid,changed,tip)
        SELECT list,uuid,changed,tip FROM `{tbl}`
        ON DUPLICATE KEY UPDATE changed=VALUES(changed), tip=VALUES(tip)",tbl=table_name);
    let res = sqlx::query(query_upsert.as_str()).execute(&mut transaction).await.context("upserting entries")?;
    trace!(affected=res.rows_affected(),"inserted into entries");
    // TODO: insert meanings
    // retrieve entries for which we need to update their meanings
    let q_fetch = format!("SELECT uuid FROM `{tbl}`",tbl = table_name);
    let to_update: Vec<FetchUuid> = sqlx::query_as::<_,FetchUuid>(q_fetch.as_str())
        .fetch(&mut transaction).try_collect().await.context("fetching entries to meaning update")?;
    let to_update: HashSet<Uuid> = to_update.into_iter().map(|v|v.uuid).collect();
    trace!(amount=to_update.len(),"retrieved entries to update meanings");
    
    // remove all meanings for entries
    // let query_delete_meanings = format!("DELETE FROM entry_meaning WHERE entry IN (SELECT uuid FROM `{tbl}`)",tbl=table_name);
    // let res = sqlx::query(query_delete_meanings.as_str()).execute(&mut transaction).await.context("deleting meanings")?;
    // trace!(affected=res.rows_affected(),"deleted old meanings");
    let mut affected = 0;
    for e in data.entries.iter() {
        let res = sqlx::query("DELETE FROM entry_meaning WHERE entry = ?")
            .bind(e.uuid)
            .execute(&mut transaction).await.context("deleting old meanings")?;
        affected += res.rows_affected();
    }
    trace!(affected=affected,"deleted old meanings");


    // now insert the new meanings
    let mut meanings_ins = 0;
    for e in data.entries.into_iter() {
        if to_update.contains(&e.uuid) {
            for m in e.meanings.into_iter() {
                let res = sqlx::query("INSERT INTO entry_meaning (entry,value,is_a) VALUES(?,?,?)")
                .bind(e.uuid)
                .bind(m.value)
                .bind(m.is_a)
                .execute(&mut transaction).await.context("inserting meanings")?;
                meanings_ins += res.rows_affected();
            }
        }
    }
    trace!(affected=meanings_ins,"inserted meanings");

    // now fetch the meanings
    let mut return_entries = Vec::with_capacity(raw_return_entries.len());
    for (_,e) in raw_return_entries.into_iter() {
        let meanings: Vec<Meaning> = sqlx::query_as::<_,Meaning>("SELECT value,is_a FROM entry_meaning WHERE entry = ?")
            .bind(e.uuid).fetch(&mut transaction).try_collect().await.context("fetching meanings")?;
        return_entries.push(e.into_full(meanings));
    }

    update_last_synced(&mut transaction,user,&data.client,LastSyncedKind::EntriesChanged,t_now).await?;

    transaction.execute(format!("DROP TABLE {}",table_name).as_str()).await.context("dropping temp table")?;

    transaction.commit().await?;

    Ok(return_entries)
}

async fn last_synced(sql: &mut MySqlConnection, user: &Uuid, client: &Uuid, kind: LastSyncedKind) -> Result<Option<Timestamp>> {
    let last_synced: Option<Timestamp> = sqlx::query_scalar("SELECT date FROM last_synced WHERE `type` = ? AND user_id = ? AND client = ? FOR UPDATE")
        .bind(kind as i32).bind(user).bind(client)
        .fetch_optional(sql).await.context("selecting last_synced")?;
        // .map(|v|v.date);

    Ok(last_synced)
}

async fn update_last_synced(sql: &mut MySqlConnection, user: &Uuid, client: &Uuid, kind: LastSyncedKind,time: Timestamp) -> Result<()> {
    sqlx::query("INSERT INTO last_synced (user_id,client,type,date) VALUES(?,?,?,?) ON DUPLICATE KEY UPDATE date=VALUES(date)")
        .bind(user).bind(client).bind(kind as i32).bind(time)
        .execute(sql).await.context("updating last_synced time")?;
    Ok(())
}