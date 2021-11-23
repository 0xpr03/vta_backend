use actix_web::{HttpResponse, get, web};
use chrono::Utc;
use crate::server::ServerInfo;
use crate::prelude::*;

#[instrument]
#[get("/api/v1/server/info")]
async fn server_info(state: AppState) -> HttpResponse {
    info!("acc register request");
    let info = ServerInfo {
        id: state.id,
        time: Utc::now().naive_utc(),
    };
    HttpResponse::Ok().json(info)
}

pub fn init(cfg: &mut web::ServiceConfig) {
    cfg.service(server_info);
}