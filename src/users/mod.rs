use actix_web::{HttpResponse, ResponseError};
use thiserror::Error;
use crate::prelude::*;

pub mod routes;
pub mod user;
pub mod dao;
#[cfg(test)]
mod tests;

#[derive(Error, Debug)]
pub enum AuthError {
    #[error("unknown data store error")]
    Other(#[from] color_eyre::eyre::Error),
    #[error("invalid UUID")]
    Uuid(#[from] uuid::Error),
    #[error("invalid jwt data")]
    Serde(#[from] serde_json::error::Error),
    #[error("db error")]
    Sqlx(#[from] sqlx::Error),
    #[error("jwt error")]
    Jwt(#[from] jsonwebtoken::errors::Error),
    #[error("invalid or missing auth")]
    NotAuthenticated,
    #[error("invalid login")]
    InvalidCredentials,
    #[error("hashing error")]
    Argon2(#[from] argon2::password_hash::Error),
    #[error("user already existing")]
    ExistingUser,
    #[error("login exists already")]
    ExistingLogin,
    #[error("account locked")]
    LockedAccount,
    #[error("user unknown")]
    UnknownUser,
    #[error("user deleted")]
    DeletedUser,
}

fn jwt_err_into_response(error: &jsonwebtoken::errors::Error) -> HttpResponse {
    use jsonwebtoken::errors::ErrorKind::*;
    HttpResponse::BadRequest().content_type("text/plain").body(match error.kind() {
        InvalidToken => "invalid JWT token",
        InvalidSignature => "invalid JWT signature",
        InvalidEcdsaKey => "ecdsa key invalid",
        InvalidRsaKey => "rsa key invalid",
        ExpiredSignature => "rsa key invalid",
        InvalidAlgorithmName => "rsa key invalid",
        InvalidKeyFormat => "invalid key format",
        InvalidIssuer => "iss invalid",
        InvalidAlgorithm => "key/decode algorithm mismatch",
        InvalidAudience => "aud invalid",
        _ => "JWT invalid"
    })
}

impl ResponseError for AuthError {
    fn error_response(&self) -> HttpResponse {
        trace!(?self);
        match self {
            AuthError::Uuid(_) => HttpResponse::BadRequest().reason("invalid UUID format").finish(),
            AuthError::Serde(_) => HttpResponse::BadRequest().reason("invalid payload").finish(),
            AuthError::Jwt(e) => jwt_err_into_response(e),
            AuthError::NotAuthenticated => HttpResponse::Unauthorized().finish(),
            AuthError::InvalidCredentials => HttpResponse::Forbidden().finish(),
            AuthError::ExistingUser => HttpResponse::Conflict().reason("user already registered").finish(),
            AuthError::ExistingLogin => HttpResponse::Conflict().reason("login already existing").finish(),
            AuthError::LockedAccount => HttpResponse::Forbidden().reason("account locked").finish(),
            AuthError::DeletedUser => HttpResponse::Conflict().reason("account deleted").finish(),
            AuthError::UnknownUser => HttpResponse::BadRequest().reason("account unknown").finish(),
            e => {
                warn!("{}",e);
                HttpResponse::InternalServerError().finish()
            }
        }
    }
}

type Result<T> = std::result::Result<T,AuthError>;