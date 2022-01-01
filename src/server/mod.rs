use actix_web::{HttpResponse, ResponseError};
use sqlx::{MySqlPool, Row};
use thiserror::Error;
use crate::prelude::*;

pub mod routes;

#[derive(Error, Debug)]
enum CError {
    #[error("invalid jwt data")]
    Serde(#[from] serde_json::error::Error),
    #[error("db error")]
    Sqlx(#[from] sqlx::Error)
}

impl ResponseError for CError {
    fn error_response(&self) -> HttpResponse {
        trace!(?self);
        HttpResponse::InternalServerError().finish()
    }
}

pub async fn load_setting(pool: &MySqlPool, key: &str) -> std::result::Result<Option<String>,sqlx::Error> {
    if let Some(row) = sqlx::query("SELECT `value` FROM settings WHERE `key` = ?")
        .bind(key)
        .fetch_optional(pool).await? {
        let value: String = row.try_get("value")?;
        Ok(Some(value))
    } else {
        Ok(None)
    }
}

pub async fn set_setting(pool: &MySqlPool, key: &str, value: &str, update: bool) -> std::result::Result<(),sqlx::Error> {
    let query = if update {
        sqlx::query("INSERT INTO settings (`key`,`value`) VALUES(?,?) ON DUPLICATE KEY `value`=VALUES(`value`)")
    } else {
        sqlx::query("INSERT INTO settings (`key`,`value`) VALUES(?,?)")
    };
    query
        .bind(key)
        .bind(value)
        .execute(pool).await?;
    Ok(())
}

#[derive(Debug,Serialize)]
struct ServerInfo {
    time: Timestamp,
    id: Uuid,
}