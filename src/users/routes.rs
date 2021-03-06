use actix_identity::Identity;
use actix_rt::task;
use actix_web::HttpRequest;
use actix_web::{get, post, web, HttpResponse};
use argon2::{self, PasswordHasher, PasswordVerifier};
use jsonwebtoken::decode;
use jsonwebtoken::Algorithm;
use jsonwebtoken::DecodingKey;
use jsonwebtoken::TokenData;
use jsonwebtoken::Validation;
use rand_core::OsRng;
use serde::de::DeserializeOwned;
use std::borrow::Borrow;
use std::collections::HashSet;

use super::user::*;
use super::*;

pub fn init(cfg: &mut web::ServiceConfig) {
    cfg.service(app_register)
        .service(account_info)
        .service(app_login)
        .service(app_password_register)
        .service(form_login)
        .service(account_delete);
}

/// App user register route.
#[instrument]
#[post("/api/v1/account/register/new")]
async fn app_register(reg: web::Json<AccRegister>, state: AppState) -> Result<HttpResponse> {
    trace!("acc register request");
    let reg = reg.into_inner();

    let server_id = state.id.to_string();
    // FIX ME: loosing context here for tracing span
    let (reg_claims, auth_key, keytype) = task::spawn_blocking(move || -> Result<_> {
        let td: TokenData<RegisterClaims> = verify_claims_auth(
            "register",
            server_id,
            &reg.proof,
            reg.key.as_bytes(),
            &reg.keytype,
        )?;
        Ok((td.claims, reg.key, reg.keytype))
    })
    .await
    .context("failed joining verifier thread")??;

    let uid = dao::register_user(
        &mut *state.sql.begin().await?,
        &reg_claims,
        auth_key.as_bytes(),
        keytype,
    )
    .await?;
    trace!(?uid, "registered account with key");
    Ok(HttpResponse::Ok().finish())
}

fn verify_claims_auth<T: DeserializeOwned>(
    sub: &str,
    server_id: String,
    input: &str,
    key: &[u8],
    k_type: &KeyType,
) -> Result<TokenData<T>> {
    debug!("verifying with {:?}", k_type);
    let aud = HashSet::from([server_id]);
    let algo_ec = vec![Algorithm::ES256, Algorithm::ES384];
    let mut validation = Validation {
        aud: Some(aud),
        // FIXME: allow zero copy in validator
        sub: Some(sub.to_owned()),
        leeway: 5,
        algorithms: algo_ec,
        ..Validation::default()
    };
    let key = match k_type {
        KeyType::EC_PEM => DecodingKey::from_ec_pem(key)?,
        KeyType::RSA_PEM => {
            // can't mix RS and EC untill fixes
            // https://github.com/Keats/jsonwebtoken/issues/219
            validation.algorithms = vec![Algorithm::RS256, Algorithm::RS384, Algorithm::RS512];
            DecodingKey::from_rsa_pem(key)?
        }
    };
    let td = decode::<T>(input, &key, &validation)?;
    Ok(td)
}

/// App user login
#[instrument(skip(id))]
#[post("/api/v1/account/login/key")]
async fn app_login(
    id: Identity,
    reg: web::Json<AccLoginKey>,
    state: AppState,
) -> Result<HttpResponse> {
    trace!("acc login via key");
    let reg = reg.into_inner();
    let mut conn = state.sql.acquire().await?;
    let user = UserId(reg.iss);
    let key_data = match dao::user_key(&mut conn, &user).await? {
        None => {
            return Err(match dao::user_deleted(&mut conn, &user).await? {
                true => AuthError::DeletedUser,
                false => AuthError::InvalidCredentials,
            })
        }
        Some(k) => k,
    };
    trace!(?key_data, "user key");
    let server_id = state.id.to_string();
    let claims = task::spawn_blocking(move || -> Result<_> {
        let td: TokenData<LoginClaims> = verify_claims_auth(
            "login",
            server_id,
            &reg.proof,
            &key_data.auth_key,
            &key_data.key_type,
        )?;
        Ok(td.claims)
    })
    .await
    .context("failed joining verifier thread")??;
    if claims.iss != user.0 {
        debug!(%claims.iss,%user,"claim iss != user");
        return Err(AuthError::InvalidCredentials);
    }
    // TODO: update last seen
    id.remember(user.to_string());
    Ok(HttpResponse::Ok().finish())
}

#[instrument(skip(id))]
#[post("/api/v1/account/delete")]
async fn account_delete(
    id: Identity,
    reg: web::Json<AccLoginKey>,
    state: AppState,
) -> Result<HttpResponse> {
    let user_id = get_user(&id)?;
    trace!(?user_id, "account delete request");

    dao::delete_user(&mut *state.sql.begin().await?, &user_id).await?;
    id.forget();

    Ok(HttpResponse::Ok().finish())
}

#[instrument(skip(id))]
#[post("/api/v1/account/login/password")]
async fn form_login(
    id: Identity,
    reg: web::Json<AccLoginPassword>,
    state: AppState,
    req: HttpRequest,
) -> Result<HttpResponse> {
    trace!("acc login via key");
    let reg: AccLoginPassword = reg.into_inner();

    let mut conn = state.sql.acquire().await?;
    let login_data = dao::user_by_email(&mut conn, &reg.email)
        .await?
        .ok_or(AuthError::InvalidCredentials)?;
    let hash_move = login_data.password;
    task::spawn_blocking(move || -> Result<_> { verify_pw(reg.password, hash_move) })
        .await
        .context("failed joining verifier thread")??;
    // TODO: update last seen
    id.remember(login_data.user_id.to_string());
    Ok(HttpResponse::Ok().finish())
}

/// add email + password to account as login
#[instrument(skip(id))]
#[post("/api/v1/account/register/password")]
async fn app_password_register(
    reg: web::Json<PasswordBindRequest>,
    id: Identity,
    state: AppState,
) -> Result<HttpResponse> {
    let user_id = get_user(id)?;
    trace!(?user_id, "acc info request");
    let reg = reg.into_inner();

    let pw_move = reg.password;
    let hashed_password = task::spawn_blocking(move || -> Result<_> { hash_pw(pw_move) })
        .await
        .context("failed joining verifier thread")??;

    dao::create_password_login(
        &mut *state.sql.begin().await?,
        &user_id,
        &reg.email,
        &hashed_password,
    )
    .await?;
    // TODO: handle duplicate entries instead of erroring

    Ok(HttpResponse::Ok().finish())
}

pub(super) fn hash_pw(pw: String) -> Result<String> {
    let salt = argon2::password_hash::SaltString::generate(&mut OsRng);

    // Argon2 with default params (Argon2id v19)
    let argon2 = argon2::Argon2::default();

    // Hash password to PHC string ($argon2id$v=19$...)
    Ok(argon2.hash_password(pw.as_bytes(), &salt)?.to_string())
}

pub(super) fn verify_pw(pw: String, hash: String) -> Result<()> {
    let parsed_hash = argon2::PasswordHash::new(&hash)?;

    let argon2 = argon2::Argon2::default();

    argon2.verify_password(pw.as_bytes(), &parsed_hash)?;
    Ok(())
}

/// App user info
#[instrument(skip(id))]
#[get("/api/v1/account/info")]
async fn account_info(id: Identity, state: AppState, req: HttpRequest) -> Result<HttpResponse> {
    let user_id = get_user(&id)?;
    Ok(
        match dao::user_by_uuid(&mut *state.sql.acquire().await?, &user_id).await? {
            Some(v) => HttpResponse::Ok().json(v),
            None => {
                id.forget();
                HttpResponse::Gone().body("deleted - user invalid")
            }
        },
    )
}

/// Retrieve user from IDentity or error out
fn get_user<T: Borrow<actix_identity::Identity>>(id: T) -> Result<UserId> {
    Ok(UserId(Uuid::parse_str(
        &id.borrow().identity().ok_or(AuthError::NotAuthenticated)?,
    )?))
}
