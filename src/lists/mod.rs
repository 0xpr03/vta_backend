use actix_web::{HttpResponse, ResponseError};
use thiserror::Error;
use crate::prelude::*;

pub mod routes;
mod models;
mod dao;
#[cfg(test)]
mod tests;

#[derive(Error, Debug)]
pub enum ListError {
    #[error("unknown data store error")]
    Other(#[from] color_eyre::eyre::Error),
    #[error("invalid UUID")]
    Uuid(#[from] uuid::Error),
    #[error("invalid jwt data")]
    Serde(#[from] serde_json::error::Error),
    #[error("db error")]
    Sqlx(#[from] sqlx::Error),
    #[error("invalid or missing auth")]
    NotAuthenticated,
    #[error("missing permission for list")]
    ListPermission,
    #[error("list not existing")]
    ListNotFound,
}

impl ResponseError for ListError {
    fn error_response(&self) -> HttpResponse {
        trace!(?self);
        match self {
            ListError::Serde(_) => HttpResponse::BadRequest().reason("invalid payload").finish(),
            ListError::NotAuthenticated => HttpResponse::Unauthorized().finish(),
            e => {
                error!("{}",e);
                HttpResponse::InternalServerError().finish()
            }
        }
    }
}

type Result<T> = std::result::Result<T,ListError>;