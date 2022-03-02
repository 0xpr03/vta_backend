use std::convert::TryInto;
use std::str::FromStr;

use chrono::Utc;
use sqlx::Connection;
use sqlx::MySqlConnection;

use super::user::*;
use super::AuthError;
use super::Result;
use crate::prelude::*;

// no async traits and I'd like to avoid async_trait
#[instrument]
pub async fn register_user(
    sql: &mut MySqlConnection,
    claims: &RegisterClaims,
    auth_key: &[u8],
    key_type: KeyType,
) -> Result<Uuid> {
    let mut transaction = sql.begin().await?;
    let t_now = Utc::now().naive_utc();

    let locked: Option<String> = None;
    let sql_user = "INSERT INTO users (uuid,name,locked,last_seen,delete_after) VALUES(?,?,?,?,?)";
    let res = sqlx::query(sql_user)
        .bind(&claims.iss)
        .bind(&claims.name)
        .bind(locked)
        .bind(t_now)
        .bind(claims.delete_after)
        .execute(&mut transaction)
        .await;
    if check_duplicate(res)? {
        return Err(AuthError::ExistingUser);
    }
    trace!("user created");
    let type_id = key_type_by_name(&mut transaction, &key_type).await?;

    sqlx::query("INSERT INTO user_key (user_id,auth_key,key_type) VALUES(?,?,?)")
        .bind(claims.iss)
        .bind(auth_key)
        .bind(type_id)
        .execute(&mut transaction)
        .await?;

    transaction.commit().await?;
    Ok(claims.iss)
}

#[instrument]
pub async fn user_key(sql: &mut MySqlConnection, user: &UserId) -> Result<Option<UserKeyParsed>> {
    if let Some(raw) = sqlx::query_as::<_, UserKey>(
        "SELECT user_id,auth_key,key_type FROM user_key WHERE user_id = ?",
    )
    .bind(user.0)
    .fetch_optional(&mut *sql)
    .await
    .context("retrieving user key")?
    {
        let (name,) = sqlx::query_as::<_, (String,)>("SELECT name FROM key_type WHERE id = ?")
            .bind(raw.key_type)
            .fetch_one(&mut *sql)
            .await
            .context("retrieving key type")?;
        let p_type = KeyType::from_str(&name).map_err(color_eyre::eyre::Error::from)?;
        Ok(Some(UserKeyParsed {
            auth_key: raw.auth_key,
            key_type: p_type,
        }))
    } else {
        Ok(None)
    }
}

/// Returns true if user got deleted
pub async fn user_deleted(sql: &mut MySqlConnection, user: &UserId) -> Result<bool> {
    let res = sqlx::query_as::<_, (bool,)>("SELECT 1 FROM deleted_user WHERE user = ?")
        .bind(user.0)
        .fetch_optional(sql)
        .await?;
    Ok(res.is_some())
}

/// Fetch user_login by email
pub async fn user_by_email(sql: &mut MySqlConnection, email: &str) -> Result<Option<UserLogin>> {
    let sql_fetch = "SELECT user_id,email,password,verified FROM user_login WHERE email = ?";
    let login: Option<UserLogin> = sqlx::query_as(sql_fetch)
        .bind(email)
        .fetch_optional(sql)
        .await?;
    Ok(login)
}

/// Insert password login for user in user_login
pub async fn create_password_login(
    sql: &mut MySqlConnection,
    user: &UserId,
    email: &str,
    password: &str,
) -> Result<()> {
    let sql_pw_bind = "INSERT INTO user_login (user_id,email,password,verified) VALUES(?,?,?,?)";
    let res = sqlx::query(sql_pw_bind)
        .bind(user.0)
        .bind(email)
        .bind(password)
        .bind(false)
        .execute(sql)
        .await;
    if check_duplicate(res)? {
        return Err(AuthError::ExistingLogin);
    }

    Ok(())
}

/// Retrieve User by uuid
pub async fn user_by_uuid(sql: &mut MySqlConnection, user: &UserId) -> Result<Option<User>> {
    let sql_fetch = "SELECT uuid,name,locked,last_seen,delete_after FROM users WHERE uuid = ?";
    let user: Option<User> = sqlx::query_as(sql_fetch)
        .bind(user.0)
        .fetch_optional(sql)
        .await?;
    Ok(user)
}

/// Delete user account
pub async fn delete_user(sql: &mut MySqlConnection, user: &UserId) -> Result<()> {
    let t_now = Utc::now().naive_utc();
    let mut transaction = sql.begin().await?;

    // check if user exists
    if sqlx::query_as::<_, (bool,)>("SELECT 1 from users WHERE uuid = ?")
        .bind(user.0)
        .fetch_optional(&mut transaction)
        .await?
        .is_none()
    {
        return Err(AuthError::UnknownUser);
    }

    // user tombstone, fails when already done
    let sql_tombstone = "INSERT INTO deleted_user (`user`,created) VALUES (?,?)";
    let res = sqlx::query(sql_tombstone)
        .bind(&user.0)
        .bind(t_now)
        .execute(&mut transaction)
        .await?;
    trace!(
        affected = res.rows_affected(),
        "creating deleted_user entry"
    );
    // shared lists tombstones
    let sql_lists_shared = "INSERT INTO deleted_list_shared (user,list,created)
    SELECT user,list,? FROM list_permissions lp
    JOIN lists l ON lp.list = l.uuid
    WHERE l.owner = ?";
    let res = sqlx::query(sql_lists_shared)
        .bind(t_now)
        .bind(&user.0)
        .execute(&mut transaction)
        .await?;
    trace!(
        affected = res.rows_affected(),
        "creating shared list tombstones"
    );
    // delete user
    let sql_del = "DELETE FROM users WHERE uuid = ?";
    let res = sqlx::query(sql_del)
        .bind(&user.0)
        .execute(&mut transaction)
        .await?;
    trace!(user=%user,affected=res.rows_affected(),"deleting user");

    transaction.commit().await?;

    Ok(())
}

async fn key_type_by_name(sql: &mut MySqlConnection, ktype: &KeyType) -> Result<i32> {
    Ok(
        match sqlx::query_as::<_, (i32,)>("SELECT id FROM key_type WHERE name = ?")
            .bind(ktype)
            .fetch_optional(&mut *sql)
            .await?
            .map(|(i,)| i)
        {
            Some(id) => id,
            None => {
                let res = sqlx::query("INSERT INTO key_type (name) VALUES(?)")
                    .bind(ktype)
                    .execute(&mut *sql)
                    .await?;
                res.last_insert_id().try_into().unwrap()
            }
        },
    )
}

/// Update last seen since date
pub async fn update_last_seen(
    sql: &mut MySqlConnection,
    user: &UserId,
    time: Timestamp,
) -> color_eyre::eyre::Result<()> {
    sqlx::query("UPDATE users SET last_seen = ? WHERE uuid = ?")
        .bind(time)
        .bind(user.0)
        .execute(sql)
        .await
        .context("updating last seen time")?;
    Ok(())
}
