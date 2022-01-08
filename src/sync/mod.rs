use std::fmt;

use actix_web::{HttpResponse, ResponseError};
use thiserror::Error;
use crate::prelude::*;

pub mod routes;
pub mod models;
pub mod dao;
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
    NotAuthenticated
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

/// Internal decoder for FromRow of ListPermission
#[derive(Debug)]
pub struct InvalidPermissionError{
    pub found: i32
}

impl fmt::Display for InvalidPermissionError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Invalid permission value {}!",self.found)
    }
}

impl std::error::Error for InvalidPermissionError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        None
    }
}
