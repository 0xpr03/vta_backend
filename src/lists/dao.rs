use std::collections::HashMap;

use chrono::Utc;
use futures::TryStreamExt;
use sqlx::{MySqlConnection, Connection};

use crate::prelude::*;
use super::*;
use super::models::*;

// #[instrument(skip(state,data))]
pub async fn all_lists(sql: &mut MySqlConnection, user: &UserId) -> Result<HashMap<Uuid,List>> {
    let sql_fetch = "SELECT uuid,name,name_a,name_b,0 as `foreign`,0 as `change` FROM lists l WHERE l.owner = ?
    UNION SELECT uuid,name,name_a,name_b,1,`write` FROM lists l
    JOIN list_permissions p ON p.list = l.uuid
    WHERE p.user = ?";
    // TODO: return is_shared
    let lists: HashMap<Uuid,List> = sqlx::query_as::<_,List>(sql_fetch)
        .bind(user.0).bind(user.0).fetch(sql)
        .map_ok(|v|(v.uuid,v)).try_collect().await.context("fetching fetching lists")?;

    Ok(lists)
}

// #[instrument(skip(state,data))]
pub async fn single_list(sql: &mut MySqlConnection, user: &UserId, list: &ListId) -> Result<List> {
    if !has_list_perm(&mut *sql,&user,&list,Permission::READ).await? {
        return Err(ListError::ListPermission);
    }
    // FIXME: we're requesting the permission data indirectly in list-perm check already
    let sql_fetch = "SELECT uuid,name,name_a,name_b,0 as `foreign`,0 as `change`
    FROM lists l WHERE l.owner = ? AND l.uuid = ?
    UNION SELECT uuid,name,name_a,name_b,1,`write` FROM lists l
    JOIN list_permissions p ON p.list = l.uuid
    WHERE p.user = ? AND l.uuid = ?";
    // TODO: return is_shared
    let lists: List = sqlx::query_as::<_,List>(sql_fetch)
        .bind(user.0).bind(list.0).bind(user.0).bind(list.0).fetch_one(sql)
        .await.context("fetching fetching lists")?;

    Ok(lists)
}

// #[instrument(skip(state,data))]
pub async fn list_sharing(sql: &mut MySqlConnection, user: &UserId, list: &ListId) -> Result<HashMap<Uuid,SharedUser>> {
    if !has_list_perm(&mut *sql,&user,&list,Permission::OWNER).await? {
        return Err(ListError::ListPermission);
    }
    let sql_fetch = "SELECT u.uuid,name,`write`,`reshare` FROM list_permissions p
    JOIN users u ON p.user = u.uuid
    WHERE p.list = ?";
    let users = sqlx::query_as::<_,SharedUser>(sql_fetch)
        .bind(list.0).fetch(sql)
        .map_ok(|v|(v.uuid,v)).try_collect()
        .await.context("fetching shared users")?;

    Ok(users)
}

pub async fn change_list(sql: &mut MySqlConnection, user: UserId, list: ListId, data: ListChange) -> Result<()> {
    let t_now = Utc::now().naive_utc();
    // TODO: what happens if this change is behind the last-change date in the DB for this list?
    let mut transaction = sql.begin().await?;
    if !has_list_perm(&mut transaction,&user,&list,Permission::WRITE).await? {
        return Err(ListError::ListPermission);
    }

    let sql_change = "UPDATE lists SET name = ?, name_a = ?, name_b = ?, `changed` = ? WHERE uuid = ?";
    let res = sqlx::query(sql_change)
        .bind(data.name).bind(data.name_a).bind(data.name_b).bind(t_now).bind(list.0)
        .execute(&mut transaction).await.context("updating list")?;
    trace!(list=%list,affected=res.rows_affected(),"updated list");

    transaction.commit().await?;
    
    Ok(())
}

pub async fn create_list(sql: &mut MySqlConnection, user: &UserId, data: ListCreate) -> Result<ListId> {
    let t_now = Utc::now().naive_utc();
    let list = Uuid::new_v4();
    let sql_create = "INSERT INTO lists (owner,uuid,name,name_a,name_b,changed,created) VALUES(?,?,?,?,?,?,?)";
    let res = sqlx::query(sql_create)
        .bind(user.0).bind(list)
        .bind(data.name).bind(data.name_a).bind(data.name_b)
        .bind(t_now).bind(t_now)
        .execute(sql).await.context("updating list")?;
    trace!(list=%list,affected=res.rows_affected(),"updated list");
    
    Ok(ListId(list))
}

pub async fn delete_list(sql: &mut MySqlConnection, user: UserId, list: ListId) -> Result<()> {
    let mut transaction = sql.begin().await?;
    if !has_list_perm(&mut transaction, &user,&list,Permission::OWNER).await? {
        return Err(ListError::ListPermission);
    }

    let t_now = Utc::now().naive_utc();

    let sql_tombstone = "INSERT INTO lists_deleted (user,list,`time`) VALUES (?,?,?)";
    sqlx::query(sql_tombstone).bind(user.0).bind(list.0).bind(t_now)
        .execute(&mut transaction)
        .await.context("inserting list tombstone")?;
    let sql_del_list = "DELETE FROM lists WHERE uuid = ?";
    let res = sqlx::query(sql_del_list).bind(user.0)
        .execute(&mut transaction).await.context("deleting list")?;
    trace!(list=%list,affected=res.rows_affected(),"deleted list");

    transaction.commit().await?;
    
    Ok(())
}

// #[instrument(skip(state,data))]
pub async fn entries(sql: &mut MySqlConnection, user: UserId, list: ListId) -> Result<List> {
    if !has_list_perm(&mut *sql, &user,&list,Permission::READ).await? {
        return Err(ListError::ListPermission);
    }
    let sql_entry = "SELECT uuid,tip FROM entries e 
    WHERE e.list = ?";
    Ok(sqlx::query_as::<_,List>(sql_entry)
        .bind(user.0).bind(user.0).fetch_one(sql).await.context("fetching single list")?)
}

pub async fn change_entry(sql: &mut MySqlConnection, user: UserId, entry: EntryId, data: EntryChange) -> Result<()> {
    let t_now = Utc::now().naive_utc();
    let mut transaction = sql.begin().await?;
    let list = list_of_entry(&mut transaction, &entry).await?;
    if !has_list_perm(&mut transaction,&user,&list,Permission::WRITE).await? {
        return Err(ListError::ListPermission);
    }

    let sql_change = "UPDATE entry SET tip = ?, `changed` = ? WHERE uuid = ?";
    let res = sqlx::query(sql_change)
        .bind(data.tip).bind(t_now).bind(entry.0)
        .execute(&mut transaction).await.context("updating entry")?;
    trace!(list=%list,affected=res.rows_affected(),"updated entry");

    let sql_del_meaning = "DELETE FROM entry_meaning WHERE entry = ?";
    let res = sqlx::query(sql_del_meaning)
        .bind(entry.0)
        .execute(&mut transaction).await.context("deleting meanings")?;
    trace!(list=%list,affected=res.rows_affected(),"deleted meanings");

    let sql_meaning = "INSERT INTO entry_meaning (entry,value,is_a) VALUES(?,?,?)";
    for m in data.meanings {
        sqlx::query(sql_meaning)
        .bind(entry.0).bind(m.value).bind(m.is_a)
        .execute(&mut transaction).await.context("inserting meanings")?;
    }

    transaction.commit().await?;
    
    Ok(())
}

pub async fn create_entry(sql: &mut MySqlConnection, user: UserId, list: ListId, data: EntryCreate) -> Result<EntryId> {
    let t_now = Utc::now().naive_utc();
    let mut transaction = sql.begin().await?;
    if !has_list_perm(&mut transaction,&user,&list,Permission::WRITE).await? {
        return Err(ListError::ListPermission);
    }

    let entry = Uuid::new_v4();

    let sql_change = "INSERT INTO entry (list,uuid,`changed`,tip) VALUES(?,?,?)";
    let res = sqlx::query(sql_change)
        .bind(list.0).bind(entry).bind(t_now).bind(data.tip)
        .execute(&mut transaction).await.context("inserting entry")?;
    trace!(list=%list,affected=res.rows_affected(),"inserting entry");

    let sql_meaning = "INSERT INTO entry_meaning (entry,value,is_a) VALUES(?,?,?)";
    for m in data.meanings {
        sqlx::query(sql_meaning)
        .bind(entry).bind(m.value).bind(m.is_a)
        .execute(&mut transaction).await.context("inserting meanings")?;
    }

    transaction.commit().await?;

    Ok(EntryId(entry))
}

pub async fn delete_entry(sql: &mut MySqlConnection, user: UserId, entry: EntryId) -> Result<()> {
    let t_now = Utc::now().naive_utc();
    let mut transaction = sql.begin().await?;
    let list = list_of_entry(&mut transaction, &entry).await?;
    if !has_list_perm(&mut transaction,&user,&list,Permission::WRITE).await? {
        return Err(ListError::ListPermission);
    }

    let sql_tombstone = "INSERT INTO lists_deleted (user,list,`time`) VALUES (?,?,?)";
    sqlx::query(sql_tombstone).bind(user.0).bind(list.0).bind(t_now)
        .execute(&mut transaction)
        .await.context("inserting list tombstone")?;

    let sql_del_entry = "DELETE FROM entry WHERE uuid = ?";
    let res = sqlx::query(sql_del_entry)
        .bind(entry.0).execute(&mut transaction).await.context("deleting entry")?;
    trace!(entry=%entry,affected=res.rows_affected(),"deleted entry");
    transaction.commit().await?;

    Ok(())
}

#[allow(clippy::upper_case_acronyms)]
#[derive(Eq, PartialEq)]
pub enum Permission {
    READ,WRITE,OWNER,RESHARE
}

async fn list_of_entry(sql: &mut MySqlConnection, entry: &EntryId) -> Result<ListId> {
    let sql_fetch = "SELECT list FROM entry WHERE id = ?";
    let list = match sqlx::query_as::<_,(Uuid,)>(sql_fetch)
        .bind(entry.0).fetch_optional(&mut *sql)
        .await.context("selecting list of entry")? {
        Some((u,)) => u,
        None => return Err(ListError::ListNotFound)
    };
    Ok(ListId(list))
}

/// Check if user has list permission. 
/// TODO: list existing ?
pub async fn has_list_perm(sql: &mut MySqlConnection, user: &UserId, list: &ListId, perm: Permission) -> Result<bool> {
    let sql_owner = "SELECT owner FROM lists WHERE uuid = ?";
    let owner = match sqlx::query_as::<_,(Uuid,)>(sql_owner)
        .bind(list.0).fetch_optional(&mut *sql)
        .await.context("testing list owner")? {
        Some((u,)) => u,
        None => return Err(ListError::ListNotFound)
    };
    let is_owner = owner == user.0;
    if perm == Permission::OWNER {
        return Ok(is_owner);
    }
    if (perm == Permission::WRITE || perm == Permission::RESHARE) && is_owner {
        return Ok(true);
    }

    let sql_foreign = if let Permission::WRITE = perm {
        "SELECT `change` FROM list_permissions WHERE list = ? AND user = ?"
    } else if let Permission::READ = perm {
        "SELECT 1 FROM list_permissions WHERE list = ? AND user = ?"
    } else {
        "SELECT reshare FROM list_permissions WHERE list = ? AND user = ?"
    };
    let perm = sqlx::query_as::<_,(bool,)>(sql_foreign)
        .bind(list.0).bind(user.0).fetch_optional(sql)
        .await.context("testing shared list perm")?
        .map_or(false,|(v,)|v);

    Ok(perm)
}