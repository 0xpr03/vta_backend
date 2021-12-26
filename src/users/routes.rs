use actix_identity::Identity;
use actix_rt::task;
use actix_web::HttpRequest;
use actix_web::{HttpResponse, get, post, web};
use argon2::{self, PasswordHasher, PasswordVerifier};
use jsonwebtoken::Algorithm;
use jsonwebtoken::DecodingKey;
use jsonwebtoken::TokenData;
use jsonwebtoken::Validation;
use jsonwebtoken::decode;
use ormx::Insert;
use rand_core::OsRng;
use serde::de::DeserializeOwned;
use std::collections::HashSet;

use super::*;
use super::user::*;

pub fn init(cfg: &mut web::ServiceConfig) {
    cfg.service(app_register)
        .service(account_info)
        .service(app_login)
        .service(app_password_register)
        .service(form_login);
}

/// App user register route.
#[instrument]
#[post("/api/v1/account/register/new")]
async fn app_register(reg: web::Json<AccRegister>,state: AppState) -> Result<HttpResponse> {
    trace!("acc register request");
    let reg = reg.into_inner();

    let server_id = state.id.to_string();
    // FIX ME: loosing context here for tracing span
    let (reg_claims,auth_key, keytype) = task::spawn_blocking(move || -> Result<_>  {
        let td: TokenData<RegisterClaims> = verify_claims_auth("register",server_id,&reg.proof,reg.key.as_bytes(),&reg.keytype)?;
        Ok((td.claims,reg.key,reg.keytype))
    }).await.context("failed joining verifier thread")??;

    let uid = dao::register_user(&state,reg_claims,auth_key.into_bytes(),keytype).await?;
    trace!(?uid,"registered account with key");
    Ok(HttpResponse::Ok().finish())
}

fn verify_claims_auth<T: DeserializeOwned>(sub: &str, server_id: String,input: &str,key: &[u8],k_type: &KeyType) -> Result<TokenData<T>>{
    debug!("verifying with {:?}", k_type);
    let aud = HashSet::from([server_id]);
    let algo_ec = vec![Algorithm::ES256,Algorithm::ES384];
    let mut validation = Validation {
        aud: Some(aud),
        // FIXME: allow zero copy in validator
        sub: Some(sub.to_owned()),
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
#[instrument(skip(id))]
#[post("/api/v1/account/login/key")]
async fn app_login(id: Identity, reg: web::Json<AccLoginKey>, state: AppState) -> Result<HttpResponse> {
    trace!("acc login via key");
    let reg = reg.into_inner();
    let user = reg.iss;
    let key_data = dao::user_key(&state,&user).await?
        .ok_or(AuthError::InvalidCredentials)?;
    trace!(?key_data,"user key");
    let server_id = state.id.to_string();
    let claims = task::spawn_blocking(move || -> Result<_>  {
        let td: TokenData<LoginClaims> = verify_claims_auth("login", server_id,&reg.proof,&key_data.auth_key,&key_data.key_type)?;
        Ok(td.claims)
    }).await.context("failed joining verifier thread")??;
    if claims.iss != user {
        debug!(%claims.iss,%user,"claim iss != user");
        return Err(AuthError::InvalidCredentials);
    }
    // TODO: update last seen
    id.remember(user.to_string());
    Ok(HttpResponse::Ok().finish())
}

#[instrument(skip(id))]
#[post("/api/v1/account/login/password")]
async fn form_login(id: Identity, reg: web::Json<AccLoginPassword>, state: AppState, req: HttpRequest) -> Result<HttpResponse> {
    trace!("acc login via key");
    let reg: AccLoginPassword = reg.into_inner();

    let mut conn = state.sql.acquire().await?;
    let login_data = UserLogin::by_email_opt(&mut conn, &reg.email).await?
        .ok_or(AuthError::InvalidCredentials)?;
    let hash_move = login_data.password;
    task::spawn_blocking(move || -> Result<_>  {
        verify_pw(reg.password,hash_move)
    }).await.context("failed joining verifier thread")??;
    // TODO: update last seen
    id.remember(login_data.user_id.to_string());
    Ok(HttpResponse::Ok().finish())
}

/// add email + password to account as login
#[instrument(skip(id))]
#[post("/api/v1/account/register/password")]
async fn app_password_register(reg: web::Json<PasswordBindRequest>, id: Identity, state: AppState) -> Result<HttpResponse> {
    let identity = id.identity();
    trace!(?identity,"acc info request");
    let uuid = Uuid::parse_str(&identity.ok_or(AuthError::NotAuthenticated)?)?;
    let reg = reg.into_inner();

    let pw_move = reg.password;
    let hashed_password = task::spawn_blocking(move || -> Result<_>  {
        hash_pw(pw_move)
    }).await.context("failed joining verifier thread")??;

    let mut conn = state.sql.acquire().await?;
    InsertUserLogin {
        user_id: uuid,
        email: reg.email,
        password: hashed_password,
        verified: false,
    }.insert(&mut conn).await?;
    // TODO: handle duplicate entries instead of erroring

    Ok(HttpResponse::Ok().finish())
}

fn hash_pw(pw: String) -> Result<String>{
    let salt = argon2::password_hash::SaltString::generate(&mut OsRng);

    // Argon2 with default params (Argon2id v19)
    let argon2 = argon2::Argon2::default();

    // Hash password to PHC string ($argon2id$v=19$...)
    Ok(argon2.hash_password(pw.as_bytes(), &salt)?.to_string())
}

fn verify_pw(pw: String, hash: String) -> Result<()> {
    let parsed_hash = argon2::PasswordHash::new(&hash)?;

    let argon2 = argon2::Argon2::default();

    argon2.verify_password(pw.as_bytes(), &parsed_hash)?;
    Ok(())
}

/// App user info
#[instrument(skip(id))]
#[get("/api/v1/account/info")]
async fn account_info(id: Identity, state: AppState, req: HttpRequest) -> Result<HttpResponse> {
    let identity = id.identity();
    trace!(?identity,"acc info request");
    let uuid = Uuid::parse_str(&identity.ok_or(AuthError::NotAuthenticated)?)?;
    Ok(match User::by_user_uuid_opt(&state.sql, &uuid).await? {
        Some(v) => HttpResponse::Ok().json(v),
        None => {
            id.forget();
            HttpResponse::Gone().body("deleted - user invalid")
        }
    })
}