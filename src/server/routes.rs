use crate::prelude::*;
use crate::server::ServerInfo;
use actix_web::{get, web, HttpResponse};
use chrono::Utc;

#[instrument]
#[get("/api/v1/server/info")]
fn server_info(state: AppState) -> HttpResponse {
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
