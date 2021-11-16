use actix_web::{HttpResponse, ResponseError};
use thiserror::Error;
use tracing::{trace, warn};

pub mod routes;
pub mod user;
pub mod dao;

#[derive(Error, Debug)]
enum AuthError {
    #[error("unknown data store error")]
    Other(#[from] color_eyre::eyre::Error),
    #[error("invalid UUID")]
    UUID(#[from] uuid::Error),
    #[error("invalid jwt data")]
    Serde(#[from] serde_json::error::Error),
    #[error("db error")]
    SQLX(#[from] sqlx::Error),
    #[error("jwt error")]
    JWT(#[from] jsonwebtoken::errors::Error)
}

fn jwt_err_into_response(error: &jsonwebtoken::errors::Error) -> HttpResponse {
    use jsonwebtoken::errors::ErrorKind::*;
    HttpResponse::BadRequest().reason(match error.kind() {
        InvalidToken => "invalid JWT token",
        InvalidSignature => "invalid JWT signature",
        InvalidEcdsaKey => "ecdsa key invalid",
        InvalidRsaKey => "rsa key invalid",
        ExpiredSignature => "rsa key invalid",
        InvalidAlgorithmName => "rsa key invalid",
        InvalidKeyFormat => "invalid key format",
        InvalidIssuer => "iss invalid",
        InvalidAlgorithm => "key/decode algorithm mismatch",
        _ => "JWT invalid"
    }).finish()
}

impl ResponseError for AuthError {
    fn error_response(&self) -> HttpResponse {
        trace!(?self);
        match self {
            AuthError::UUID(_) => HttpResponse::BadRequest().reason("invalid UUID format").finish(),
            AuthError::Serde(_) => HttpResponse::BadRequest().reason("invalid payload").finish(),
            AuthError::JWT(e) => jwt_err_into_response(e),
            e => {
                warn!("{}",e);
                HttpResponse::InternalServerError().finish()
            }
        }
    }
}