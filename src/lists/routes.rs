use actix_identity::Identity;
use actix_web::{HttpResponse, get, post, delete, web};
use super::models::*;
use super::*;

pub fn init(cfg: &mut web::ServiceConfig) {
    cfg.service(all_lists)
        .service(single_list)
        .service(change_list)
        .service(delete_list)
        .service(create_list)
        .service(delete_entry)
        .service(list_entries)
        .service(change_entry)
        .service(create_entry);
        // .service(list_sync_changed)
        // .service(entry_sync_del)
        // .service(entry_sync_changed);
}

// #[instrument(skip(id,reg,state))]
#[get("/api/v1/lists")]
async fn all_lists(id: Identity, state: AppState) -> Result<HttpResponse> {
    let user = get_user(id)?;

    let response = dao::all_lists(&mut *state.sql.acquire().await?, user).await?;
    Ok(HttpResponse::Ok().json(response))
}

#[get("/api/v1/lists/{list}")]
async fn single_list(id: Identity, state: AppState, path: web::Path<(Uuid,)>) -> Result<HttpResponse> {
    let user = get_user(id)?;
    let (list,) = path.into_inner();

    let response = dao::single_list(&mut *state.sql.acquire().await?, user, ListId(list)).await?;
    Ok(HttpResponse::Ok().json(response))
}

#[delete("/api/v1/lists/{list}")]
async fn delete_list(id: Identity, state: AppState, path: web::Path<(Uuid,)>) -> Result<HttpResponse> {
    let user = get_user(id)?;
    let (list,) = path.into_inner();

    let response = dao::delete_list(&mut *state.sql.acquire().await?, user, ListId(list)).await?;
    Ok(HttpResponse::Ok().json(response))
}

#[post("/api/v1/lists/{list}")]
async fn change_list(id: Identity, state: AppState, path: web::Path<(Uuid,)>, reg: web::Json<ListChange>) -> Result<HttpResponse> {
    let user = get_user(id)?;
    let (list,) = path.into_inner();
    let data = reg.into_inner();

    let response = dao::change_list(&mut *state.sql.acquire().await?, user, ListId(list), data).await?;
    Ok(HttpResponse::Ok().json(response))
}

#[post("/api/v1/lists")]
async fn create_list(id: Identity, state: AppState, reg: web::Json<ListCreate>) -> Result<HttpResponse> {
    let user = get_user(id)?;
    let data = reg.into_inner();

    let response = dao::create_list(&mut *state.sql.acquire().await?, user, data).await?;
    Ok(HttpResponse::Ok().json(response.0))
}

#[get("/api/v1/lists/{list}/entries")]
async fn list_entries(id: Identity, state: AppState, path: web::Path<(Uuid,)>) -> Result<HttpResponse> {
    let user = get_user(id)?;
    let (list,) = path.into_inner();

    let response = dao::entries(&mut *state.sql.acquire().await?, user, ListId(list)).await?;
    Ok(HttpResponse::Ok().json(response))
}

#[delete("/api/v1/lists/{list}/entry/{entry}")]
async fn delete_entry(id: Identity, state: AppState, path: web::Path<(Uuid,Uuid)>) -> Result<HttpResponse> {
    let user = get_user(id)?;
    // TODO: we don't need the list, we have to resolve the entry-list by ourself anyway
    // but its logical to have this API path
    let (_list,entry) = path.into_inner();

    let response = dao::delete_entry(&mut *state.sql.acquire().await?, user, EntryId(entry)).await?;
    Ok(HttpResponse::Ok().json(response))
}

#[post("/api/v1/lists/{list}/entry/{entry}")]
async fn change_entry(id: Identity, state: AppState, path: web::Path<(Uuid,Uuid)>, data: web::Path<EntryChange>) -> Result<HttpResponse> {
    let user = get_user(id)?;
    // TODO: we don't need the list, we have to resolve the entry-list by ourself anyway
    // but its logical to have this API path
    let (_list,entry) = path.into_inner();
    let data = data.into_inner();

    let response = dao::change_entry(&mut *state.sql.acquire().await?, user, EntryId(entry), data).await?;
    Ok(HttpResponse::Ok().json(response))
}

#[post("/api/v1/lists/{list}/entry")]
async fn create_entry(id: Identity, state: AppState, path: web::Path<(Uuid,)>, data: web::Path<EntryCreate>) -> Result<HttpResponse> {
    let user = get_user(id)?;
    // TODO: we don't need the list, we have to resolve the entry-list by ourself anyway
    // but its logical to have this API path
    let (list,) = path.into_inner();
    let data = data.into_inner();

    let response = dao::create_entry(&mut *state.sql.acquire().await?, user, ListId(list), data).await?;
    Ok(HttpResponse::Ok().json(response.0))
}

fn get_user(id: Identity) -> Result<UserId> {
    Ok(UserId(Uuid::parse_str(&id.identity().ok_or(ListError::NotAuthenticated)?)?))
}