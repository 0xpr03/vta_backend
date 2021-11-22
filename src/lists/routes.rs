use actix_identity::Identity;
use actix_web::{HttpRequest, HttpResponse, get, post, web};
use crate::prelude::*;
use super::list::*;
use super::*;

pub fn init(cfg: &mut web::ServiceConfig) {
    cfg.service(list_sync_del)
        .service(list_sync_changed);
}

#[instrument(skip(id))]
#[post("/api/v1/sync/lists/deleted")]
async fn list_sync_del(reg: web::Json<ListDeletedRequest>, id: Identity, state: AppState) -> Result<HttpResponse> {
    let identity = id.identity();
    trace!(?identity,"acc info request");
    let user = Uuid::parse_str(&identity.ok_or(ListError::NotAuthenticated)?)?;
    let data = reg.into_inner();

    let response = dao::update_deleted_lists(&state, data, &user).await?;
    Ok(HttpResponse::Ok().json(response))
}

#[instrument(skip(id))]
#[post("/api/v1/sync/lists/changed")]
async fn list_sync_changed(id: Identity, state: AppState, req: HttpRequest) -> Result<HttpResponse> {
    let identity = id.identity();
    trace!(?identity,"acc info request");
    let uuid = Uuid::parse_str(&identity.ok_or(ListError::NotAuthenticated)?)?;
    // Ok(match User::by_user_uuid_opt(&state.sql, &uuid).await? {
    //     Some(v) => HttpResponse::Ok().json(v),
    //     None => {
    //         id.forget();
    //         HttpResponse::Gone().body("deleted - user invalid")
    //     }
    // })
    unimplemented!()
}