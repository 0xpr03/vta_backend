use std::collections::HashMap;

use base64ct::Base64Url;
use base64ct::Encoding;
use chrono::Utc;
use color_eyre::eyre::eyre;
use futures::TryStreamExt;
use rand_core::RngCore;
use sha2::Digest;
use sha2::Sha256;
use sqlx::MySql;
use sqlx::Transaction;
use sqlx::{Connection, MySqlConnection};
use subtle::ConstantTimeEq;

use super::models::*;
use super::*;

// #[instrument(skip(state,data))]
pub async fn all_lists(sql: &mut MySqlConnection, user: &UserId) -> Result<HashMap<Uuid, List>> {
    let sql_fetch =
        "SELECT uuid,name,name_a,name_b,0 as `foreign`,0 as `change` FROM lists l WHERE l.owner = ?
    UNION SELECT uuid,name,name_a,name_b,1,`write` FROM lists l
    JOIN list_permissions p ON p.list = l.uuid
    WHERE p.user = ?";
    // TODO: return is_shared
    let lists: HashMap<Uuid, List> = sqlx::query_as::<_, List>(sql_fetch)
        .bind(user.0)
        .bind(user.0)
        .fetch(sql)
        .map_ok(|v| (v.uuid, v))
        .try_collect()
        .await
        .context("fetching fetching lists")?;

    Ok(lists)
}

// #[instrument(skip(state,data))]
pub async fn single_list(sql: &mut MySqlConnection, user: &UserId, list: &ListId) -> Result<List> {
    if !has_list_perm(&mut *sql, &user, &list, Permission::READ).await? {
        return Err(ListError::ListPermission);
    }
    // FIXME: we're requesting the permission data indirectly in list-perm check already
    let sql_fetch = "SELECT uuid,name,name_a,name_b,0 as `foreign`,0 as `change`
    FROM lists l WHERE l.owner = ? AND l.uuid = ?
    UNION SELECT uuid,name,name_a,name_b,1,`write` FROM lists l
    JOIN list_permissions p ON p.list = l.uuid
    WHERE p.user = ? AND l.uuid = ?";
    // TODO: return is_shared
    let lists: List = sqlx::query_as::<_, List>(sql_fetch)
        .bind(user.0)
        .bind(list.0)
        .bind(user.0)
        .bind(list.0)
        .fetch_one(sql)
        .await
        .context("fetching fetching lists")?;

    Ok(lists)
}

// #[instrument(skip(state,data))]
pub async fn list_sharing(
    sql: &mut MySqlConnection,
    user: &UserId,
    list: &ListId,
) -> Result<HashMap<Uuid, SharedUser>> {
    if !has_list_perm(&mut *sql, &user, &list, Permission::OWNER).await? {
        return Err(ListError::ListPermission);
    }
    let sql_fetch = "SELECT u.uuid,name,`write`,`reshare` FROM list_permissions p
    JOIN users u ON p.user = u.uuid
    WHERE p.list = ?";
    let users = sqlx::query_as::<_, SharedUser>(sql_fetch)
        .bind(list.0)
        .fetch(sql)
        .map_ok(|v| (v.uuid, v))
        .try_collect()
        .await
        .context("fetching shared users")?;

    Ok(users)
}

// #[instrument(skip(state,data))]
pub async fn remove_sharing_user(
    sql: &mut MySqlConnection,
    user: &UserId,
    list: &ListId,
    shared_user: &UserId,
) -> Result<()> {
    if !has_list_perm(&mut *sql, &user, &list, Permission::OWNER).await? {
        return Err(ListError::ListPermission);
    }
    let sql_del = "DELETE FROM list_permissions p
    WHERE p.list = ? AND p.user = ?";
    let res = sqlx::query(sql_del)
        .bind(list.0)
        .bind(shared_user.0)
        .execute(sql)
        .await
        .context("fetching shared users")?;
    trace!(
        affected = res.rows_affected(),
        "removed user from shared access"
    );
    Ok(())
}

pub async fn set_share_permissions(
    sql: &mut MySqlConnection,
    user: &UserId,
    list: &ListId,
    shared_user: &UserId,
    perms: UserPermissions,
) -> Result<()> {
    if !has_list_perm(&mut *sql, &user, &list, Permission::OWNER).await? {
        return Err(ListError::ListPermission);
    }
    let sql_del = "UPDATE list_permissions SET `write` = ?, `reshare` = ?
    WHERE p.list = ? AND p.user = ?";
    let res = sqlx::query(sql_del)
        .bind(perms.write)
        .bind(perms.reshare)
        .bind(list.0)
        .bind(shared_user.0)
        .execute(sql)
        .await
        .context("fetching shared users")?;
    trace!(affected = res.rows_affected(), "changed shared user access");
    Ok(())
}

pub async fn generate_share_code(
    sql: &mut MySqlConnection,
    user: &UserId,
    list: &ListId,
    data: NewTokenData,
) -> Result<ShareTokenReturn> {
    if !has_list_perm(&mut *sql, &user, &list, Permission::OWNER).await? {
        return Err(ListError::ListPermission);
    }

    let mut rng = rand::thread_rng();

    let mut token_a = [0u8; 16];
    rng.fill_bytes(&mut token_a);
    let mut token_b = [0u8; 16];
    rng.fill_bytes(&mut token_b);

    let token_b_hash = {
        let mut hasher = Sha256::new();
        hasher.update(token_b);
        hasher.finalize()
    };
    debug_assert_eq!(token_b_hash.len(), 32);
    debug_assert_eq!(token_a.len(), 16);

    let sql_token = "INSERT INTO share_token (list,token_a,deadline,hash,`write`,reshare,reusable) VALUES(?,?,?,?,?,?,?)";
    sqlx::query(sql_token)
        .bind(list.0)
        .bind(token_a.as_slice())
        .bind(data.deadline)
        .bind(token_b_hash.as_slice())
        .bind(data.write)
        .bind(data.reshare)
        .bind(data.reusable)
        .execute(sql)
        .await
        .context("inserting sharing token")?;
    Ok(ShareTokenReturn {
        token_a: Base64Url::encode_string(token_a.as_slice()),
        token_b: Base64Url::encode_string(token_b.as_slice()),
    })
}

pub async fn use_share_code(
    sql: &mut MySqlConnection,
    user: &UserId,
    token_a: &str,
    token_b: &str,
) -> Result<ListId> {
    let mut transaction = sql.begin().await?;
    let res = _use_share_code(&mut transaction, user, token_a, token_b).await;
    if res.is_ok() {
        transaction.commit().await?;
    } else {
        transaction.rollback().await?;
    }
    res
}

async fn _use_share_code(
    sql: &mut Transaction<'_, MySql>,
    user: &UserId,
    token_a: &str,
    token_b: &str,
) -> Result<ListId> {
    let sql_sel =
        "SELECT list,deadline,hash,`write`,reshare,reusable FROM share_token WHERE token_a = ?";
    let time = Utc::now().naive_utc();

    let token_a_decoded = match Base64Url::decode_vec(token_a) {
        Ok(v) => v,
        Err(_) => return Err(ListError::ValidationError("token_a")),
    };
    let res: Option<ShareTokenEntry> = sqlx::query_as::<_, ShareTokenEntry>(sql_sel)
        .bind(&token_a_decoded)
        .fetch_optional(&mut *sql)
        .await
        .context("fetching share token")?;
    match res {
        None => Err(ListError::SharecodeInvalid),
        Some(entry) => {
            if time > entry.deadline {
                return Err(ListError::SharecodeOutdated);
            }

            let token_b_decoded = match Base64Url::decode_vec(token_b) {
                Ok(v) => v,
                Err(e) => {
                    debug!(?e, "base64 decode failed");
                    return Err(ListError::ValidationError("token_b"));
                }
            };
            debug_assert_eq!(token_b_decoded.len(), 16);
            let token_b_hash = {
                let mut hasher = Sha256::new();
                hasher.update(token_b_decoded);
                hasher.finalize()
            };
            debug_assert_eq!(token_b_hash.len(), 32);
            // We don't need more than constant time verification of the hash
            if entry
                .hash
                .as_slice()
                .ct_eq(token_b_hash.as_slice())
                .unwrap_u8()
                != 1u8
            {
                return Err(ListError::SharecodeInvalid);
            }

            // TODO: handle user is owner
            let sql_add = "INSERT INTO list_permissions (user,list,`write`,reshare,changed) VALUES (?,?,?,?,?)";
            let res = sqlx::query(sql_add)
                .bind(user.0)
                .bind(&entry.list)
                .bind(entry.write)
                .bind(entry.reshare)
                .bind(time)
                .execute(&mut *sql)
                .await;
            if check_duplicate(res)? {
                // TODO: handle already accessible list
                return Ok(ListId(entry.list));
            }

            if !entry.reusable {
                let sql_del_code = "DELETE FROM share_token WHERE token_a = ?";
                sqlx::query(sql_del_code)
                    .bind(&token_a_decoded)
                    .execute(&mut *sql)
                    .await
                    .context("removing share code")?;
            }
            Ok(ListId(entry.list))
        }
    }
}

pub async fn change_list(
    sql: &mut MySqlConnection,
    user: &UserId,
    list: ListId,
    data: ListChange,
) -> Result<()> {
    let t_now = Utc::now().naive_utc();
    // TODO: what happens if this change is behind the last-change date in the DB for this list?
    let mut transaction = sql.begin().await?;
    if !has_list_perm(&mut transaction, &user, &list, Permission::WRITE).await? {
        return Err(ListError::ListPermission);
    }

    let sql_change =
        "UPDATE lists SET name = ?, name_a = ?, name_b = ?, `changed` = ? WHERE uuid = ?";
    let res = sqlx::query(sql_change)
        .bind(data.name)
        .bind(data.name_a)
        .bind(data.name_b)
        .bind(t_now)
        .bind(list.0)
        .execute(&mut transaction)
        .await
        .context("updating list")?;
    trace!(list=%list,affected=res.rows_affected(),"updated list");

    transaction.commit().await?;

    Ok(())
}

pub async fn create_list(
    sql: &mut MySqlConnection,
    user: &UserId,
    data: ListCreate,
) -> Result<ListId> {
    let t_now = Utc::now().naive_utc();
    let list = Uuid::new_v4();
    let sql_create =
        "INSERT INTO lists (owner,uuid,name,name_a,name_b,changed,created) VALUES(?,?,?,?,?,?,?)";
    let res = sqlx::query(sql_create)
        .bind(user.0)
        .bind(list)
        .bind(data.name)
        .bind(data.name_a)
        .bind(data.name_b)
        .bind(t_now)
        .bind(t_now)
        .execute(sql)
        .await
        .context("updating list")?;
    trace!(list=%list,affected=res.rows_affected(),"updated list");

    Ok(ListId(list))
}

pub async fn delete_list(sql: &mut MySqlConnection, user: &UserId, list: ListId) -> Result<()> {
    let mut transaction = sql.begin().await?;
    let res = _delete_list(&mut transaction, user, list).await;
    if res.is_ok() {
        transaction.commit().await?;
    } else {
        transaction.rollback().await?;
    }
    res
}

async fn _delete_list(
    transaction: &mut Transaction<'_, MySql>,
    user: &UserId,
    list: ListId,
) -> Result<()> {
    if !has_list_perm(&mut *transaction, &user, &list, Permission::OWNER).await? {
        return Err(ListError::ListPermission);
    }

    let t_now = Utc::now().naive_utc();

    let sql_tombstone = "INSERT INTO deleted_list (user,list,created) VALUES (?,?,?)";
    sqlx::query(sql_tombstone)
        .bind(user.0)
        .bind(list.0)
        .bind(t_now)
        .execute(&mut *transaction)
        .await
        .context("inserting list tombstone")?;
    let sql_del_list = "DELETE FROM lists WHERE uuid = ?";
    let res = sqlx::query(sql_del_list)
        .bind(list.0)
        .execute(&mut *transaction)
        .await
        .context("deleting list")?;
    trace!(list=%list,affected=res.rows_affected(),"deleted list");
    if res.rows_affected() < 1 {
        return Err(ListError::Other(eyre!(
            "Expected > 0 affected rows on list delete, got {}",
            res.rows_affected()
        )));
    }
    Ok(())
}

// #[instrument(skip(state,data))]
pub async fn entries(
    sql: &mut MySqlConnection,
    user: &UserId,
    list: ListId,
) -> Result<HashMap<Uuid, Entry>> {
    if !has_list_perm(&mut *sql, &user, &list, Permission::READ).await? {
        return Err(ListError::ListPermission);
    }
    let sql_entry = "SELECT uuid,tip FROM entries e 
    WHERE e.list = ?";
    let raw_e: Vec<(Uuid, String)> = sqlx::query_as::<_, (Uuid, String)>(sql_entry)
        .bind(list.0)
        .fetch(&mut *sql)
        .try_collect()
        .await
        .context("fetching entries")?;

    let sql_meanings = "SELECT value,is_a FROM entry_meaning WHERE entry = ?";
    let mut entries = HashMap::with_capacity(raw_e.len());
    for (uuid, tip) in raw_e {
        let meanings = sqlx::query_as::<_, EntryMeaning>(sql_meanings)
            .bind(&uuid)
            .fetch(&mut *sql)
            .try_collect()
            .await
            .context("fetching meanings")?;
        entries.insert(
            uuid.clone(),
            Entry {
                tip,
                uuid,
                meanings,
            },
        );
    }

    Ok(entries)
}

pub async fn change_entry(
    sql: &mut MySqlConnection,
    user: &UserId,
    entry: EntryId,
    data: EntryChange,
) -> Result<()> {
    let mut transaction = sql.begin().await?;

    let res = _change_entry(&mut transaction, user, entry, data).await;
    if res.is_ok() {
        transaction.commit().await?;
    } else {
        transaction.rollback().await?;
    }
    res
}

async fn _change_entry(
    transaction: &mut Transaction<'_, MySql>,
    user: &UserId,
    entry: EntryId,
    data: EntryChange,
) -> Result<()> {
    let t_now = Utc::now().naive_utc();
    let list = list_of_entry(&mut *transaction, &entry).await?;
    if !has_list_perm(&mut *transaction, &user, &list, Permission::WRITE).await? {
        return Err(ListError::ListPermission);
    }

    let sql_change = "UPDATE entries SET tip = ?, `changed` = ? WHERE uuid = ?";
    let res = sqlx::query(sql_change)
        .bind(data.tip)
        .bind(t_now)
        .bind(entry.0)
        .execute(&mut *transaction)
        .await
        .context("updating entry")?;
    trace!(list=%list,affected=res.rows_affected(),"updated entry");

    let sql_del_meaning = "DELETE FROM entry_meaning WHERE entry = ?";
    let res = sqlx::query(sql_del_meaning)
        .bind(entry.0)
        .execute(&mut *transaction)
        .await
        .context("deleting meanings")?;
    trace!(list=%list,affected=res.rows_affected(),"deleted meanings");

    let sql_meaning = "INSERT INTO entry_meaning (entry,value,is_a) VALUES(?,?,?)";
    for m in data.meanings {
        sqlx::query(sql_meaning)
            .bind(entry.0)
            .bind(m.value)
            .bind(m.is_a)
            .execute(&mut *transaction)
            .await
            .context("inserting meanings")?;
    }
    Ok(())
}

pub async fn create_entry(
    sql: &mut MySqlConnection,
    user: UserId,
    list: ListId,
    data: EntryCreate,
) -> Result<EntryId> {
    let t_now = Utc::now().naive_utc();
    let mut transaction = sql.begin().await?;
    if !has_list_perm(&mut transaction, &user, &list, Permission::WRITE).await? {
        return Err(ListError::ListPermission);
    }

    let entry = Uuid::new_v4();

    let sql_change = "INSERT INTO entries (list,uuid,`changed`,updated,tip) VALUES(?,?,?,?,?)";
    let res = sqlx::query(sql_change)
        .bind(list.0)
        .bind(entry)
        .bind(t_now)
        .bind(t_now)
        .bind(data.tip)
        .execute(&mut transaction)
        .await
        .context("inserting entry")?;
    trace!(list=%list,affected=res.rows_affected(),"inserting entry");

    let sql_meaning = "INSERT INTO entry_meaning (entry,value,is_a) VALUES(?,?,?)";
    for m in data.meanings {
        sqlx::query(sql_meaning)
            .bind(entry)
            .bind(m.value)
            .bind(m.is_a)
            .execute(&mut transaction)
            .await
            .context("inserting meanings")?;
    }

    transaction.commit().await?;

    Ok(EntryId(entry))
}

pub async fn delete_entry(sql: &mut MySqlConnection, user: &UserId, entry: EntryId) -> Result<()> {
    let mut transaction = sql.begin().await?;
    let res = _delete_entry(&mut transaction, user, entry).await;
    if res.is_ok() {
        transaction.commit().await?;
    } else {
        transaction.rollback().await?;
    }
    res
}

async fn _delete_entry(
    transaction: &mut Transaction<'_, MySql>,
    user: &UserId,
    entry: EntryId,
) -> Result<()> {
    let t_now = Utc::now().naive_utc();
    let list = list_of_entry(&mut *transaction, &entry).await?;
    if !has_list_perm(&mut *transaction, &user, &list, Permission::WRITE).await? {
        return Err(ListError::ListPermission);
    }

    let sql_tombstone = "INSERT INTO deleted_entry (list,`entry`,created) VALUES (?,?,?)";
    sqlx::query(sql_tombstone)
        .bind(list.0)
        .bind(entry.0)
        .bind(t_now)
        .execute(&mut *transaction)
        .await
        .context("inserting list tombstone")?;

    let sql_del_entry = "DELETE FROM entries WHERE uuid = ?";
    let res = sqlx::query(sql_del_entry)
        .bind(entry.0)
        .execute(&mut *transaction)
        .await
        .context("deleting entry")?;
    trace!(entry=%entry,affected=res.rows_affected(),"deleted entry");
    Ok(())
}

#[allow(clippy::upper_case_acronyms)]
#[derive(Eq, PartialEq)]
pub enum Permission {
    READ,
    WRITE,
    OWNER,
    RESHARE,
}

async fn list_of_entry(sql: &mut MySqlConnection, entry: &EntryId) -> Result<ListId> {
    let sql_fetch = "SELECT list FROM entries WHERE uuid = ?";
    let list = sqlx::query_scalar::<_, Uuid>(sql_fetch)
        .bind(entry.0)
        .fetch_optional(&mut *sql)
        .await
        .context("selecting list of entry")?
        .ok_or(ListError::ListNotFound)?;
    Ok(ListId(list))
}

/// Check if user has list permission.
/// TODO: list existing ?
pub async fn has_list_perm(
    sql: &mut MySqlConnection,
    user: &UserId,
    list: &ListId,
    perm: Permission,
) -> Result<bool> {
    let sql_owner = "SELECT owner FROM lists WHERE uuid = ?";
    let owner = sqlx::query_scalar::<_, Uuid>(sql_owner)
        .bind(list.0)
        .fetch_optional(&mut *sql)
        .await
        .context("testing list owner")?
        .ok_or(ListError::ListNotFound)?;
    let is_owner = owner == user.0;
    if perm == Permission::OWNER {
        return Ok(is_owner);
    }
    if (perm == Permission::READ || perm == Permission::WRITE || perm == Permission::RESHARE)
        && is_owner
    {
        return Ok(true);
    }

    let sql_foreign = if let Permission::WRITE = perm {
        "SELECT `change` FROM list_permissions WHERE list = ? AND user = ?"
    } else if let Permission::READ = perm {
        "SELECT 1 FROM list_permissions WHERE list = ? AND user = ?"
    } else {
        "SELECT reshare FROM list_permissions WHERE list = ? AND user = ?"
    };
    let perm = sqlx::query_scalar::<_, bool>(sql_foreign)
        .bind(list.0)
        .bind(user.0)
        .fetch_optional(sql)
        .await
        .context("testing shared list perm")?
        .unwrap_or(false);

    Ok(perm)
}
