use actix_identity::Identity;
use actix_web::{HttpResponse, post, web};
use super::models::*;
use super::*;

pub fn init(cfg: &mut web::ServiceConfig) {
    cfg.service(list_sync_del)
        .service(list_sync_changed)
        .service(entry_sync_del)
        .service(entry_sync_changed);
}

#[instrument(skip(id,reg,state))]
#[post("/api/v1/sync/lists/deleted")]
async fn list_sync_del(reg: web::Json<ListDeletedRequest>, id: Identity, state: AppState) -> Result<HttpResponse> {
    let identity = id.identity();
    trace!(?identity,"list sync deleted request");
    let user = Uuid::parse_str(&identity.ok_or(ListError::NotAuthenticated)?)?;
    let data = reg.into_inner();

    let response = dao::update_deleted_lists(&mut *state.sql.acquire().await?, data, &user).await?;
    Ok(HttpResponse::Ok().json(response))
}

#[instrument(skip(id,reg,state))]
#[post("/api/v1/sync/lists/changed")]
async fn list_sync_changed(reg: web::Json<ListChangedRequest>, id: Identity, state: AppState) -> Result<HttpResponse> {
    let identity = id.identity();
    trace!(?identity,"list sync changed request");
    let user = Uuid::parse_str(&identity.ok_or(ListError::NotAuthenticated)?)?;
    let response = dao::update_changed_lists(&mut *state.sql.acquire().await?, reg.into_inner(), &UserId(user)).await?;
    Ok(HttpResponse::Ok().json(response))
}

#[instrument(skip(id,reg,state))]
#[post("/api/v1/sync/entries/deleted")]
async fn entry_sync_del(reg: web::Json<EntryDeletedRequest>, id: Identity, state: AppState) -> Result<HttpResponse> {
    let identity = id.identity();
    trace!(?identity,"entry sync deleted request");
    let user = UserId(Uuid::parse_str(&identity.ok_or(ListError::NotAuthenticated)?)?);
    let data = reg.into_inner();

    let response = dao::update_deleted_entries(&mut *state.sql.acquire().await?, data, &user).await?;
    Ok(HttpResponse::Ok().json(response))
}

#[instrument(skip(id,reg,state))]
#[post("/api/v1/sync/entries/changed")]
async fn entry_sync_changed(reg: web::Json<EntryChangedRequest>, id: Identity, state: AppState) -> Result<HttpResponse> {
    let identity = id.identity();
    trace!(?identity,"entry sync changed request");
    let user = Uuid::parse_str(&identity.ok_or(ListError::NotAuthenticated)?)?;
    let data = reg.into_inner();

    //let mut connection = state.sql.acquire().await?
    let response = dao::update_changed_entries(&mut *state.sql.acquire().await?, data, &user).await?;
    Ok(HttpResponse::Ok().json(response))
}