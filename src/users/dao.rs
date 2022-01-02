use std::convert::TryInto;
use std::str::FromStr;

use chrono::Utc;
use sqlx::Connection;
use sqlx::MySqlConnection;

use crate::prelude::*;
use super::AuthError;
use super::user::*;
use super::Result;

// no async traits and I'd like to avoid async_trait
#[instrument]
pub async fn register_user(sql: &mut MySqlConnection, claims: &RegisterClaims, auth_key: &[u8], key_type: KeyType) -> Result<Uuid> {
    let mut transaction = sql.begin().await?;
    let t_now = Utc::now().naive_utc();

    let locked: Option<String> = None;
    let sql_user = "INSERT INTO users (uuid,name,locked,last_seen,delete_after) VALUES(?,?,?,?,?)";
    let res = sqlx::query(sql_user).bind(&claims.iss).bind(&claims.name).bind(locked).bind(t_now).bind(claims.delete_after)
        .execute(&mut transaction).await;
    if check_duplicate(res)? {
        return Err(AuthError::ExistingUser);
    }
    trace!("user created");
    let type_id = key_type_by_name(&mut transaction, &key_type).await?;

    sqlx::query("INSERT INTO user_key (user_id,auth_key,key_type) VALUES(?,?,?)")
        .bind(claims.iss).bind(auth_key).bind(type_id).execute(&mut transaction).await?;

    transaction.commit().await?;
    Ok(claims.iss)
}

#[instrument]
pub async fn user_key(sql: &mut MySqlConnection, user: &Uuid) -> Result<Option<UserKeyParsed>> {
    if let Some(raw) = sqlx::query_as::<_,UserKey>("SELECT user_id,auth_key,key_type FROM user_key WHERE user_id = ?")
        .bind(user).fetch_optional(&mut *sql).await.context("retrieving user key")? {
        let (name,) = sqlx::query_as::<_,(String,)>("SELECT name FROM key_type WHERE id = ?")
            .bind(raw.key_type).fetch_one(&mut *sql).await.context("retrieving key type")?;
        let p_type = KeyType::from_str(&name).map_err(color_eyre::eyre::Error::from)?;
        Ok(Some(
            UserKeyParsed {
                auth_key: raw.auth_key,
                key_type: p_type,
            }
        ))
    } else {
        Ok(None)
    }
}

/// Fetch user_login by email
pub async fn user_by_email(sql: &mut MySqlConnection, email: &str) -> Result<Option<UserLogin>> {
    let sql_fetch = "SELECT user_id,email,password,verified FROM user_login WHERE email = ?";
    let login: Option<UserLogin> = sqlx::query_as(sql_fetch).bind(email).fetch_optional(sql).await?;
    Ok(login)
}

/// Insert password login for user in user_login
pub async fn create_password_login(sql: &mut MySqlConnection,user: &Uuid,email: &str,password: &str) -> Result<()> {
    
    let sql_pw_bind = "INSERT INTO user_login (user_id,email,password,verified) VALUES(?,?,?,?)";
    let res = sqlx::query(sql_pw_bind).bind(user).bind(email).bind(password).bind(false).execute(sql).await;
    if check_duplicate(res)? {
        return Err(AuthError::ExistingLogin);
    }

    Ok(())
}

/// Retrieve User by uuid
pub async fn user_by_uuid(sql: &mut MySqlConnection, user: &Uuid) -> Result<Option<User>> {
    let sql_fetch = "SELECT uuid,name,locked,last_seen,delete_after FROM users WHERE uuid = ?";
    let user: Option<User> = sqlx::query_as(sql_fetch).bind(user).fetch_optional(sql).await?;
    Ok(user)
}

async fn key_type_by_name(sql: &mut MySqlConnection, ktype: &KeyType) -> Result<i32> {
    Ok(match sqlx::query_as::<_,(i32,)>("SELECT id FROM key_type WHERE name = ?")
        .bind(ktype).fetch_optional(&mut *sql).await?.map(|(i,)|i) {
        Some(id) => id,
        None => {
            let res = sqlx::query("INSERT INTO key_type (name) VALUES(?)").bind(ktype).execute(&mut *sql).await?;
            res.last_insert_id().try_into().unwrap()
        }
    })
}

/// Check query result for duplicate-entry error. Returns true if found.
fn check_duplicate(res: std::result::Result<sqlx::mysql::MySqlQueryResult, sqlx::Error>) -> Result<bool> {
    if let Err(e) = res {
        if let sqlx::Error::Database(ref e) = e {
            if e.code() == Some(std::borrow::Cow::Borrowed("23000")) {
                return Ok(true);
            }
        }
        return Err(e.into());
    } else {
        return Ok(false)
    }
}