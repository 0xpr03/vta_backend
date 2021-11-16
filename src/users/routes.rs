use actix_rt::task;
use actix_web::{HttpResponse, Responder, get, post, web};
use color_eyre::eyre::Context;
use jsonwebtoken::Algorithm;
use jsonwebtoken::DecodingKey;
use jsonwebtoken::Validation;
use jsonwebtoken::decode;
use ormx::Table;
use tracing::*;

use crate::state::AppState;

use super::AuthError;
use std::result::Result as StdResult;
use super::user::*;
use super::dao::*;

pub fn init(cfg: &mut web::ServiceConfig) {
    cfg.service(app_register)
        .service(app_info)
        .service(debug_find_all)
        .service(app_login);
}


/// App user register route.
#[instrument]
#[post("/api/v1/account/register")]
async fn app_register(reg: web::Json<AccRegister>,state: AppState) -> StdResult<HttpResponse,AuthError> {
    trace!("acc register request");
    let reg = reg.into_inner();

    // FIX ME: loosing context here for tracing span
    let (reg_claims,auth_key) = task::spawn_blocking(move || -> StdResult<_,AuthError>  {
        // can't allow Algorithm::RS256,Algorithm::RS384,Algorithm::RS512 untill fixes
        // https://github.com/Keats/jsonwebtoken/issues/219
        let algorithms = vec![Algorithm::ES256,Algorithm::ES384];
        let validation = Validation { sub: Some(String::from("register")), leeway: 5, algorithms, ..Validation::default() };
        let key = match reg.keytype {
            KeyType::EC_PEM => DecodingKey::from_ec_pem(reg.key.as_bytes())?,
            KeyType::RSA_PEM => DecodingKey::from_rsa_pem(reg.key.as_bytes())?,
        };
        let td = decode::<RegisterClaims>(&reg.proof, &key, &validation)?;
        Ok((td.claims,reg.key))
    }).await.context("failed joining verifier thread")??;

    let uid = register_user(&state,reg_claims,auth_key.into_bytes()).await?;
    trace!(?uid,"registered account with key");
    Ok(HttpResponse::Accepted().finish())
}

/// App user login
#[instrument]
#[post("/api/v1/account/login")]
async fn app_login(reg: web::Json<AccLogin>, state: AppState) -> StdResult<HttpResponse,AuthError> {
    info!("acc register request");


    
    Ok(HttpResponse::Accepted().finish())
}

/// App user info
#[instrument]
#[post("/api/v1/account/info")]
async fn app_info(state: AppState) -> StdResult<HttpResponse,AuthError> {
    info!("acc register request");
    
    Ok(HttpResponse::Accepted().finish())
}

/// Debug route
#[get("/users")]
async fn debug_find_all(state: AppState) -> impl Responder {
    let result = User::all(&state.get_ref().sql).await;
    match result {
        Ok(users) => HttpResponse::Ok().json(users),
        Err(err) => {
            error!("error fetching todos: {}", err);
            HttpResponse::InternalServerError()
                .body("Error trying to read all todos from database")
        }
    }
}