use actix_web::{HttpResponse, ResponseError};
use josekit::JoseError;
use thiserror::Error;
use tracing::warn;

pub mod routes;
pub mod user;
pub mod dao;

#[derive(Error, Debug)]
enum AuthError {
    #[error("JWT error")]
    JWT(#[from] JoseError),
    #[error("unknown data store error")]
    Other(#[from] color_eyre::eyre::Error),
    #[error("invalid UUID")]
    UUID(#[from] uuid::Error),
    #[error("invalid jwt data")]
    Serde(#[from] serde_json::error::Error),
    #[error("db error")]
    SQLX(#[from] sqlx::Error),
}

impl ResponseError for AuthError {
    fn error_response(&self) -> HttpResponse {
        match self {
            AuthError::JWT(e) => {
                match e {
                    JoseError::InvalidKeyFormat(_) => HttpResponse::BadRequest().reason("invalid ecdsa key").finish(),
                    JoseError::InvalidJson(_) => HttpResponse::BadRequest().reason("invalid json").finish(),
                    JoseError::InvalidClaim(_) => HttpResponse::BadRequest().reason("invalid claim").finish(),
                    JoseError::InvalidSignature(_) => HttpResponse::BadRequest().reason("invalid proof").finish(),
                    JoseError::InvalidJwtFormat(_) | JoseError::InvalidJwsFormat(_) => HttpResponse::BadRequest().reason("invalid JWS format").finish(),
                    e => {
                        warn!(error = ?e,"josekit error");
                        HttpResponse::InternalServerError().finish()
                    },
                }
            },
            AuthError::UUID(_) => HttpResponse::BadRequest().reason("invalid UUID format").finish(),
            AuthError::Serde(_) => HttpResponse::BadRequest().reason("invalid payload").finish(),            
            e => {
                warn!("{}",e);
                HttpResponse::InternalServerError().finish()
            }
        }
    }
}