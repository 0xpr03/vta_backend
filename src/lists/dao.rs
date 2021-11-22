use std::collections::HashSet;

use chrono::Utc;
use ormx::exports::futures::TryStreamExt;

use crate::prelude::*;
use super::*;
use super::list::*;

// async fn last_sync(sqlx: impl MySqlExecutor<'static>, user: &Uuid, client: &Uuid, kind: LastSyncedKind) -> Result<Option<Timestamp>> {
//     let res = sqlx::query!("SELECT date FROM last_synced WHERE `type` = ? AND user_id = ? AND client = ?", kind as i32, user, client)
//         .fetch_optional(sqlx).await?;
    
//     Ok(res.map(|v|v.date))
// }

#[instrument(skip(state,data))]
pub async fn update_deleted_lists(state: &AppState, mut data: ListDeletedRequest, user: &Uuid) -> Result<HashSet<ListDeleteEntry>> {
    let t_now = Utc::now();

    let mut transaction = state.sql.begin().await?;
    
    // let synced = dao::last_sync(&mut transaction, user,&data.client, LastSyncedKind::ListsDeleted).await?;
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

    Ok(return_lists)
}