use crate::prelude::*;
use actix_web::{HttpResponse, ResponseError};
use thiserror::Error;

mod dao;
mod models;
pub mod routes;
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
    #[error("sharecode invalid")]
    SharecodeInvalid,
    #[error("sharecode outdated")]
    SharecodeOutdated,
    #[error("Failed to validate field {}", 0)]
    ValidationError(&'static str),
}

impl ResponseError for ListError {
    fn error_response(&self) -> HttpResponse {
        trace!(?self);
        match self {
            ListError::Serde(_) => HttpResponse::BadRequest()
                .reason("invalid payload")
                .finish(),
            ListError::ListNotFound => HttpResponse::NotFound().reason("invalid list").finish(),
            ListError::SharecodeInvalid => HttpResponse::NotFound().reason("invalid").finish(),
            ListError::ListPermission => HttpResponse::Forbidden()
                .reason("missing permissions for list")
                .finish(),
            ListError::NotAuthenticated => HttpResponse::Unauthorized().finish(),
            e => {
                error!("{}", e);
                HttpResponse::InternalServerError().finish()
            }
        }
    }
}

type Result<T> = std::result::Result<T, ListError>;
