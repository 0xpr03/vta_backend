use actix_web::{HttpResponse, post, web};
use time::OffsetDateTime;
use tracing::{info, instrument};
use crate::{server::ServerInfo, state::AppState};

#[instrument]
#[post("/api/v1/server/info")]
async fn server_info(state: AppState) -> HttpResponse {
    info!("acc register request");
    let info = ServerInfo {
        id: state.id,
        time: OffsetDateTime::now_utc().unix_timestamp(),
    };
    HttpResponse::Ok().json(info)
}

pub fn init(cfg: &mut web::ServiceConfig) {
    cfg.service(server_info);
}