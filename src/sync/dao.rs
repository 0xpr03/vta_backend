use std::collections::HashSet;
use std::iter::repeat;

use chrono::Utc;
use ormx::exports::futures::TryStreamExt;
use rand::Rng;
use rand::distributions::Alphanumeric;
use sqlx::Executor;

use crate::prelude::*;
use super::*;
use super::models::*;

#[instrument(skip(state,data))]
pub async fn update_deleted_lists(state: &AppState, mut data: ListDeletedRequest, user: &Uuid) -> Result<HashSet<ListDeleteEntry>> {
    let t_now = Utc::now();

    let mut transaction = state.sql.begin().await?;
    
    let last_synced: Option<Timestamp> = sqlx::query!("SELECT date FROM last_synced WHERE `type` = ? AND user_id = ? AND client = ? FOR UPDATE",
        LastSyncedKind::ListsDeleted as i32, user, &data.client)
        .fetch_optional(&mut transaction).await?.map(|v|v.date);

    let stream = match last_synced {
        Some(time) => sqlx::query_as::<_,ListDeleteEntry>("SELECT list,time FROM deleted_list WHERE user = ? AND time > ?").bind(user).bind(time),
        None => sqlx::query_as::<_,ListDeleteEntry>("SELECT list,time FROM deleted_list WHERE user = ?").bind(user)
    }.fetch(&mut transaction);

    let mut return_lists: HashSet<ListDeleteEntry> = stream.try_collect().await?;
    
    for v in data.lists.iter_mut() {
        if v.time > t_now {
            info!(%v.time,%t_now,"ignoring change date in future");
            v.time = t_now;
        }
        if !return_lists.remove(v) {
            sqlx::query!("INSERT IGNORE INTO deleted_list (user,list,time) VALUES(?,?,?)",user,v.list,v.time).execute(&mut transaction).await?;
            sqlx::query!("DELETE FROM lists WHERE owner = ? AND uuid = ?",user,v.list).execute(&mut transaction).await?;
        }
    }

    sqlx::query!("INSERT INTO last_synced (user_id,client,type,date) VALUES(?,?,?,?) ON DUPLICATE KEY UPDATE date=VALUES(date)",
        user,&data.client,LastSyncedKind::ListsDeleted as i32,t_now)
        .execute(&mut transaction).await.context("updating sync time")?;

    Ok(return_lists)
}

#[instrument(skip(state,data))]
pub async fn update_changed_lists(state: &AppState, mut data: ListChangedRequest, user: &Uuid) -> Result<Vec<ListChangedEntry>> {
    let t_now = Utc::now();
    let mut transaction = state.sql.begin().await?;
    
    let mut rng = rand::thread_rng();
    let table_name: String = repeat(())
        .map(|()| rng.sample(Alphanumeric))
        .map(char::from)
        .take(7)
        .collect();

    let last_synced: Option<Timestamp> = sqlx::query!("SELECT date FROM last_synced WHERE `type` = ? AND user_id = ? AND client = ? FOR UPDATE",
        LastSyncedKind::ListsChanged as i32, user, &data.client)
        .fetch_optional(&mut transaction).await?.map(|v|v.date);

    transaction.execute(format!("CREATE TEMPORARY TABLE {} (
        uuid BINARY(16) NOT NULL PRIMARY KEY,
        name VARCHAR(127) NOT NULL,
        name_a VARCHAR(127) NOT NULL,
        name_b VARCHAR(127) NOT NULL,
        changed BIGINT UNSIGNED NOT NULL,
        created BIGINT UNSIGNED NOT NULL,
        INDEX (uuid,changed)
    )",table_name).as_str()).await?;
    
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

    // remove deleted ones
    let query_deleted = format!("DELETE FROM `{}` WHERE uuid IN (SELECT uuid FROM deleted_list l)",table_name);
    sqlx::query(query_deleted.as_str()).execute(&mut transaction).await.context("removing deleted")?;

    // remove outdated
    let query_outdated = format!("DELETE FROM `{tbl}` WHERE uuid IN (SELECT uuid FROM lists li WHERE li.`uuid` = `{tbl}`.`uuid` AND li.`changed` >= `{tbl}`.`changed`);",tbl = table_name);
    sqlx::query(query_outdated.as_str()).execute(&mut transaction).await.context("removing outdated")?;

    // resolve all changed entries we should send back
    let stream = match last_synced {
        Some(time) => sqlx::query_as::<_,ListChangedEntry>("SELECT uuid,name,name_a,name_b,changed,created FROM lists WHERE owner = ? AND time > ?").bind(user).bind(time),
        None => sqlx::query_as::<_,ListChangedEntry>("SELECT uuid,name,name_a,name_b,changed,created FROM lists WHERE owner = ?").bind(user)
    }.fetch(&mut transaction);
    let return_lists: Vec<ListChangedEntry> = stream.try_collect().await.context("requesting changes")?;

    // now insert changes from client
    let query_upsert = format!("INSERT INTO lists (uuid,name,name_a,name_b,changed,created,owner)
        SELECT uuid,name,name_a,name_b,changed,created,? FROM `{tbl}`
        ON DUPLICATE KEY UPDATE name=VALUES(name),name_a=VALUES(name_a),name_b=VALUES(name_b),changed=VALUES(changed)",tbl = table_name);
    sqlx::query(query_upsert.as_str()).bind(user).execute(&mut transaction).await.context("inserting changes")?;

    sqlx::query!("INSERT INTO last_synced (user_id,client,type,date) VALUES(?,?,?,?) ON DUPLICATE KEY UPDATE date=VALUES(date)",
        user,&data.client,LastSyncedKind::ListsChanged as i32,t_now)
        .execute(&mut transaction).await.context("updating sync time")?;
    
    transaction.commit().await?;

    Ok(return_lists)
}