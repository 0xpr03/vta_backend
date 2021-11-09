use actix_rt::task;
use actix_web::{HttpResponse, Responder, get, post, web};
use color_eyre::eyre::Context;
use josekit::jwt;
use ormx::Table;
use sqlx::{MySqlPool};
use tracing::*;
use josekit::jws::alg::ecdsa::EcdsaJwsAlgorithm::Es256;

use crate::state::AppState;

use super::AuthError;
use std::result::Result as StdResult;
use super::user::*;
use super::dao::*;

pub fn init(cfg: &mut web::ServiceConfig) {
    cfg.service(register)
        .service(info)
        .service(find_all)
        .service(login);
}


#[instrument]
#[post("/api/v1/account/register")]
async fn register(reg: web::Json<AccRegister>,state: AppState) -> StdResult<HttpResponse,AuthError> {
    trace!("acc register request");
    let reg = reg.into_inner();

    // FIX ME: loosing context here for tracing span
    let ((payload, header),auth_key) = task::spawn_blocking(move || -> StdResult<_,AuthError>  {
        let verifier = Es256.verifier_from_pem(reg.key.as_bytes())?;
        Ok((jwt::decode_with_verifier(reg.proof, &verifier)?,reg.key))
    }).await.context("failed joining verifier thread")??;

    debug!(?header, ?payload,"JWT verified");
    let claims: josekit::Map<String,josekit::Value>  = payload.into();
    let val = serde_json::Value::Object(claims);
    let reg_claims: RegisterClaims = serde_json::from_value(val)?;

    let uid = register_user(&state,reg_claims,auth_key.into_bytes()).await?;
    trace!(?uid,"registered account with key");
    Ok(HttpResponse::Accepted().finish())
}

#[instrument]
#[post("/api/v1/account/login")]
async fn login(db_pool: web::Data<MySqlPool>) -> StdResult<HttpResponse,AuthError> {
    info!("acc register request");
    
    Ok(HttpResponse::Accepted().finish())
}

#[instrument]
#[post("/api/v1/account/info")]
async fn info(db_pool: web::Data<MySqlPool>) -> StdResult<HttpResponse,AuthError> {
    info!("acc register request");
    
    Ok(HttpResponse::Accepted().finish())
}

#[get("/users")]
async fn find_all(state: AppState) -> impl Responder {
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