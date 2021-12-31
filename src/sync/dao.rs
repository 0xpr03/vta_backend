use std::collections::{HashSet, HashMap};
use std::iter::repeat;

use chrono::Utc;
use ormx::exports::futures::TryStreamExt;
use rand::Rng;
use rand::distributions::Alphanumeric;
use sqlx::{Executor, MySqlConnection, Connection};

use crate::prelude::*;
use super::*;
use super::models::*;

//#[instrument(skip(state,data))]
pub async fn update_deleted_lists(sql: &mut MySqlConnection, mut data: ListDeletedRequest, user: &Uuid) -> Result<HashSet<ListDeleteEntry>> {
    let t_now = Utc::now().naive_utc();

    let mut transaction = sql.begin().await?;
    
    let last_synced: Option<Timestamp> = sqlx::query!("SELECT date FROM last_synced WHERE `type` = ? AND user_id = ? AND client = ? FOR UPDATE",
        LastSyncedKind::ListsDeleted as i32, user, &data.client)
        .fetch_optional(&mut transaction).await?.map(|v|v.date);

    let stream = match last_synced {
        Some(time) => sqlx::query_as::<_,ListDeleteEntry>("SELECT list,time FROM deleted_list WHERE user = ? AND time > ?").bind(user).bind(time),
        None => sqlx::query_as::<_,ListDeleteEntry>("SELECT list,time FROM deleted_list WHERE user = ?").bind(user)
    }.fetch(&mut transaction);

    let mut return_lists: HashSet<ListDeleteEntry> = stream.try_collect().await?;
    
    // two loops to retain statement cache
    // FIXLATER no async in iterators
    let mut lists = Vec::with_capacity(data.lists.len());
    for v in data.lists.iter_mut() {
        if v.time > t_now {
            info!(%v.time,%t_now,"ignoring change date in future");
            v.time = t_now;
        }
        // don't process deletions we already know
        if !return_lists.remove(v) {
            // TODO: check owner, not change permissions
            sqlx::query!("INSERT IGNORE INTO deleted_list (user,list,time) VALUES(?,?,?)",user,v.list,v.time).execute(&mut transaction).await?;
            lists.push(v);
        }
    }
    for v in lists.iter() {
        sqlx::query!("DELETE FROM lists WHERE owner = ? AND uuid = ?",user,v.list).execute(&mut transaction).await?;
    }

    sqlx::query!("INSERT INTO last_synced (user_id,client,type,date) VALUES(?,?,?,?) ON DUPLICATE KEY UPDATE date=VALUES(date)",
        user,&data.client,LastSyncedKind::ListsDeleted as i32,t_now)
        .execute(&mut transaction).await.context("updating sync time")?;

    transaction.commit().await?;

    Ok(return_lists)
}

//#[instrument(skip(state,data))]
pub async fn update_changed_lists(sql: &mut MySqlConnection, mut data: ListChangedRequest, user: &Uuid) -> Result<ListChangedResponse> {
    let t_now = Utc::now().naive_utc();
    let mut transaction = sql.begin().await?;
    
    let mut rng = rand::thread_rng();
    let table_name: String = format!("t_{}",repeat(())
        .map(|()| rng.sample(Alphanumeric))
        .map(char::from)
        .take(7)
        .collect::<String>());
    let last_synced: Option<Timestamp> = sqlx::query!("SELECT date FROM last_synced WHERE `type` = ? AND user_id = ? AND client = ? FOR UPDATE",
        LastSyncedKind::ListsChanged as i32, user, &data.client)
        .fetch_optional(&mut transaction).await?.map(|v|v.date);
    trace!(?last_synced, "Last synced");
    
    transaction.execute(format!("CREATE TEMPORARY TABLE {} (
        uuid BINARY(16) NOT NULL PRIMARY KEY,
        name VARCHAR(127) NOT NULL,
        name_a VARCHAR(127) NOT NULL,
        name_b VARCHAR(127) NOT NULL,
        changed BIGINT UNSIGNED NOT NULL,
        created BIGINT UNSIGNED NOT NULL,
        INDEX (uuid,changed)
    )",table_name).as_str()).await?;
    trace!("created temp table");
    
    let mut failure = Vec::new();
    let ins_sql = format!("INSERT IGNORE {} (uuid,name,name_a,name_b,changed,created) VALUES(?,?,?,?,?,?)",table_name);
    for v in data.lists.iter_mut() {
        if v.changed > t_now {
            info!(%v.changed,%t_now,"ignoring change date in future");
            failure.push(EntrySyncFailure {
                id: v.uuid,
                error: format!("Invalid changed date: {} current time: {}",v.changed, t_now),
            });
            continue;
        }
        sqlx::query(ins_sql.as_str())
            .bind(v.uuid)
            .bind(&v.name)
            .bind(&v.name_a)
            .bind(&v.name_b)
            .bind(v.changed)
            .bind(v.created)
            .execute(&mut transaction).await?;
    }
    trace!("inserted {} received changes",data.lists.len());
    
    // remove deleted ones
    let query_deleted = format!("DELETE FROM `{tbl}` WHERE uuid IN
        (SELECT uuid FROM deleted_list d WHERE d.list = `{tbl}`.uuid AND d.user = ? )",tbl=table_name);
    let res = sqlx::query(query_deleted.as_str()).bind(user).execute(&mut transaction).await.context("removing deleted")?;
    trace!(affected=res.rows_affected(),"removed deleted data");
    
    // remove outdated
    let query_outdated = format!("DELETE FROM `{tbl}` WHERE uuid IN
        (SELECT uuid FROM lists l WHERE l.`uuid` = `{tbl}`.`uuid` AND l.`changed` >= `{tbl}`.`changed` FOR UPDATE);",tbl = table_name);
    let res = sqlx::query(query_outdated.as_str()).execute(&mut transaction).await.context("removing outdated")?;
    trace!(affected=res.rows_affected(),"removed outdated data");
    trace!("Retrieving changes to send back");
    // resolve all changed entries we should send back
    let sql_t;
    let stream = match last_synced {
        Some(time) => {
            sql_t = format!("SELECT uuid,name,name_a,name_b,changed,created FROM lists WHERE owner = ? AND changed > ? AND uuid NOT IN (SELECT uuid FROM `{}`)",table_name);
            sqlx::query_as::<_,ListChangedEntry>(sql_t.as_str()).bind(user).bind(time)
        },
        None => {
            sql_t = format!("SELECT uuid,name,name_a,name_b,changed,created FROM lists WHERE owner = ? AND uuid NOT IN (SELECT uuid FROM `{}`)",table_name);
            sqlx::query_as::<_,ListChangedEntry>(sql_t.as_str()).bind(user)
        }
    }.fetch(&mut transaction);
    let return_lists: Vec<ListChangedEntry> = stream.try_collect().await.context("requesting changes")?;
    trace!("Found {} changes to send back, failures {}", return_lists.len(),failure.len());
    let response = ListChangedResponse {
        lists: return_lists,
        failures: failure
    };

    // TODO: check owner/change permissions

    trace!("Inserting new data to db");
    // now insert changes from client
    let query_upsert = format!("INSERT INTO lists (uuid,name,name_a,name_b,changed,created,owner)
        SELECT uuid,name,name_a,name_b,changed,created,? FROM `{tbl}`
        ON DUPLICATE KEY UPDATE name=VALUES(name),name_a=VALUES(name_a),name_b=VALUES(name_b),changed=VALUES(changed)",tbl = table_name);
    let res = sqlx::query(query_upsert.as_str()).bind(user).execute(&mut transaction).await.context("inserting changes")?;
    trace!(affected=res.rows_affected(),"updating last-seen for client");
    sqlx::query!("INSERT INTO last_synced (user_id,client,type,date) VALUES(?,?,?,?) ON DUPLICATE KEY UPDATE date=VALUES(date)",
        user,&data.client,LastSyncedKind::ListsChanged as i32,t_now)
        .execute(&mut transaction).await.context("updating sync time")?;

    transaction.execute(format!("DROP TABLE {}",table_name).as_str()).await.context("dropping temp table")?;
    
    transaction.commit().await?;

    Ok(response)
}

//#[instrument(skip(state,data))]
pub async fn update_deleted_entries(sql: &mut MySqlConnection, mut data: EntryDeletedRequest, user: &Uuid) -> Result<HashSet<EntryDeleteEntry>> {
    let t_now = Utc::now().naive_utc();

    let mut transaction = sql.begin().await?;
    
    let last_synced: Option<Timestamp> = sqlx::query!("SELECT date FROM last_synced WHERE `type` = ? AND user_id = ? AND client = ? FOR UPDATE",
        LastSyncedKind::EntriesDeleted as i32, user, &data.client)
        .fetch_optional(&mut transaction).await.context("selecting last_synced")?.map(|v|v.date);

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
        Some(time) => q.bind(user).bind(time).bind(user).bind(time),
        None => q.bind(user).bind(user)
    }.fetch(&mut transaction);

    let mut return_lists: HashSet<EntryDeleteEntry> = stream.try_collect().await.context("fetching deleted_entry to send back")?;
    trace!(affected=return_lists.len(),"fetched send-back");
  
    let mut rng = rand::thread_rng();
    let table_name: String = format!("t_{}",repeat(())
        .map(|()| rng.sample(Alphanumeric))
        .map(char::from)
        .take(7)
        .collect::<String>());
    
    transaction.execute(format!("CREATE TEMPORARY TABLE {} (
        uuid BINARY(16) NOT NULL PRIMARY KEY,
        list BINARY(16) NOT NULL,
        time BIGINT UNSIGNED NOT NULL,
        INDEX (uuid,list)
    )",table_name).as_str()).await.context("creating temp table")?;
    // insert received delete entries into temp table
    let sql_t = format!("INSERT INTO {tbl} (uuid,time,list) VALUES(?,?,?) ON DUPLICATE KEY UPDATE time=VALUES(time)",tbl= table_name);
    for v in data.entries.iter_mut() {
        if v.time > t_now {
            info!(%v.time,%t_now,"ignoring change date in future");
            v.time = t_now;
        }
        
        // remove entries from return data that we got already send
        // if not in return set, insert to temp table, otherwise known
        if !return_lists.remove(v) {
            sqlx::query(sql_t.as_str())
            .bind(v.entry)
            .bind(v.time)
            .bind(v.list)
            .execute(&mut transaction).await.context("inserting data to temp table")?;
        }
    }

    // remove entries for lists that are already deleted
    let sqlt_tdel = format!("DELETE FROM {tbl} WHERE list IN (SELECT list from `deleted_list`)",tbl = table_name);
    let sql_res = sqlx::query(sqlt_tdel.as_str())
        .execute(&mut transaction).await.context("removing entries for deleted lists")?;
    trace!(affected=sql_res.rows_affected(),"removed entries from already deleted lists");

    // remove all non-existing/non-owner lists from temp table
    let sqlt_del_nonowned = format!("DELETE FROM {tbl} WHERE list NOT IN (
        SELECT l.uuid FROM lists l
        WHERE `{tbl}`.list = l.uuid AND l.owner = ?
        UNION
        SELECT p.list FROM list_permissions p
        WHERE p.list = `{tbl}`.list AND p.`change` = true AND p.user = ?)", tbl = table_name);
    let sql_res = sqlx::query(sqlt_del_nonowned.as_str())
        .bind(user)
        .bind(user)
        .execute(&mut transaction).await.context("removing entries without list permissions")?;
    trace!(affected=sql_res.rows_affected(),"removed entries without permission / existing list");

    // insert remaining new data from temp table into deleted_entry
    let query_upsert = format!("INSERT INTO deleted_entry (entry,list,time)
        SELECT uuid,list,time FROM `{tbl}`",tbl = table_name);
    let sql_res = sqlx::query(query_upsert.as_str()).execute(&mut transaction).await.context("inserting back deleted entries")?;
    trace!(affected=sql_res.rows_affected(),"inserted into deleted_entry");

    // delete from entries
    // retrieve entries to delete via faster method
    let del_ids: Vec<FetchUuid> = sqlx::query_as::<_,FetchUuid>(format!("SELECT uuid FROM `{tbl}`",tbl=table_name).as_str())
            .fetch(&mut transaction).try_collect().await.context("fetching delete ids")?;
    let mut affected = 0;
    for id in del_ids.into_iter() {
        let sql_res = sqlx::query("DELETE FROM entries WHERE uuid = ?")
            .bind(id.uuid)
            .execute(&mut transaction).await.context("inserting back deleted entries")?;
        affected += sql_res.rows_affected();
    }
    trace!(affected=affected,"deleted entries");

    // let query_upsert = format!("DELETE FROM entries WHERE uuid IN
    //     (SELECT uuid FROM `{tbl}`)",tbl = table_name);
    // let sql_res = sqlx::query(query_upsert.as_str()).execute(&mut transaction).await.context("inserting back deleted entries")?;
    // trace!(affected=sql_res.rows_affected(),"deleted entries");

    sqlx::query!("INSERT INTO last_synced (user_id,client,type,date) VALUES(?,?,?,?) ON DUPLICATE KEY UPDATE date=VALUES(date)",
        user,&data.client,LastSyncedKind::EntriesDeleted as i32,t_now)
        .execute(&mut transaction).await.context("updating last_synced time")?;

    transaction.execute(format!("DROP TABLE {}",table_name).as_str()).await.context("dropping temp table")?;

    transaction.commit().await?;

    Ok(return_lists)
}

//#[instrument(skip(state,data))]
pub async fn update_changed_entries(sql: &mut MySqlConnection, mut data: EntryChangedRequest, user: &Uuid) -> Result<Vec<EntryChangedEntry>> {
    let t_now = Utc::now().naive_utc();

    let mut transaction = sql.begin().await?;
    
    let last_synced: Option<Timestamp> = sqlx::query!("SELECT date FROM last_synced WHERE `type` = ? AND user_id = ? AND client = ? FOR UPDATE",
        LastSyncedKind::EntriesChanged as i32, user, &data.client)
        .fetch_optional(&mut transaction).await.context("selecting last_synced")?.map(|v|v.date);

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
        WHERE p.list = `{tbl}`.list AND p.`change` = true AND p.user = ?)", tbl = table_name);
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

    sqlx::query!("INSERT INTO last_synced (user_id,client,type,date) VALUES(?,?,?,?) ON DUPLICATE KEY UPDATE date=VALUES(date)",
    user,&data.client,LastSyncedKind::EntriesChanged as i32,t_now)
    .execute(&mut transaction).await.context("updating last_synced time")?;

    transaction.execute(format!("DROP TABLE {}",table_name).as_str()).await.context("dropping temp table")?;

    transaction.commit().await?;

    Ok(return_entries)
}
