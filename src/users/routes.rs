use actix_identity::Identity;
use actix_rt::task;
use actix_web::{HttpResponse, Responder, get, post, web};
use color_eyre::eyre::Context;
use jsonwebtoken::Algorithm;
use jsonwebtoken::DecodingKey;
use jsonwebtoken::TokenData;
use jsonwebtoken::Validation;
use jsonwebtoken::decode;
use ormx::Table;
use serde::de::DeserializeOwned;
use tracing::*;
use tracing_actix_web::RootSpan;
use uuid::Uuid;

use crate::state::AppState;

use super::dao;
use super::AuthError;
use std::collections::HashSet;
use std::result::Result as StdResult;
use super::user::*;
use super::Result;

pub fn init(cfg: &mut web::ServiceConfig) {
    cfg.service(app_register)
        .service(app_info)
        .service(debug_find_all)
        .service(app_login);
}

/// App user register route.
#[instrument]
#[post("/api/v1/account/register")]
async fn app_register(reg: web::Json<AccRegister>,state: AppState) -> Result<HttpResponse> {
    trace!("acc register request");
    let reg = reg.into_inner();

    let server_id = state.id.to_string();

    // FIX ME: loosing context here for tracing span
    let (reg_claims,auth_key, keytype) = task::spawn_blocking(move || -> Result<_>  {
        let td: TokenData<RegisterClaims> = verify_claims_auth(server_id,&reg.proof,reg.key.as_bytes(),&reg.keytype)?;
        Ok((td.claims,reg.key,reg.keytype))
    }).await.context("failed joining verifier thread")??;

    let uid = dao::register_user(&state,reg_claims,auth_key.into_bytes(),keytype).await?;
    trace!(?uid,"registered account with key");
    Ok(HttpResponse::Accepted().finish())
}

fn verify_claims_auth<T: DeserializeOwned>(server_id: String,input: &str,key: &[u8],k_type: &KeyType) -> Result<TokenData<T>>{
    let aud = HashSet::from([server_id]);
    let algo_ec = vec![Algorithm::ES256,Algorithm::ES384];
    let mut validation = Validation {
        aud: Some(aud),
        sub: Some(String::from("register")),
        leeway: 5,
        algorithms: algo_ec,
        ..Validation::default()
    };
    let key = match k_type {
        KeyType::EC_PEM => {
            DecodingKey::from_ec_pem(key)?
        },
        KeyType::RSA_PEM => {
            // can't mix RS and EC untill fixes
            // https://github.com/Keats/jsonwebtoken/issues/219
            validation.algorithms = vec![Algorithm::RS256,Algorithm::RS384,Algorithm::RS512];
            DecodingKey::from_rsa_pem(key)?
        },
    };
    let td = decode::<T>(input, &key, &validation)?;
    Ok(td)
}

/// App user login
#[post("/api/v1/account/login/key")]
async fn app_login(id: Identity, reg: web::Json<AccLogin>, state: AppState, root_span: RootSpan) -> Result<HttpResponse> {
    let reg = reg.into_inner();
    let user = reg.iss;
    let key_data = dao::user_key(&state,&user).await?
        .ok_or(AuthError::InvalidCredentials)?;
    
    let server_id = state.id.to_string();
    let claims = task::spawn_blocking(move || -> Result<_>  {
        let td: TokenData<LoginClaims> = verify_claims_auth(server_id,&reg.proof,&key_data.auth_key,&key_data.key_type)?;
        Ok(td.claims)
    }).await.context("failed joining verifier thread")??;
    if claims.iss != user {
        debug!(%claims.iss,%user,"claim iss != user");
        return Err(AuthError::InvalidCredentials);
    }

    id.remember(user.to_string());
    Ok(HttpResponse::NoContent().finish())
}

/// App user info
#[get("/api/v1/account/info")]
async fn app_info(id: Identity, state: AppState) -> Result<HttpResponse> {
    info!("acc register request");
    let uuid = Uuid::parse_str(&id.identity().ok_or(AuthError::NotAuthenticated)?)?;
    Ok(match User::by_user_uuid_opt(&state.sql, &uuid).await? {
        Some(v) => HttpResponse::Ok().json(v),
        None => {
            id.forget();
            HttpResponse::Gone().body("deleted - user invalid")
        }
    })
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