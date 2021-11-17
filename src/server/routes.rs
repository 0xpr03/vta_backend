use actix_web::{HttpResponse, post, web};
use tracing::{info, instrument};
use color_eyre::eyre::Result;
use crate::state::AppState;

#[instrument]
#[post("/api/v1/server/info")]
async fn server_info(state: AppState) -> HttpResponse {
    info!("acc register request");
    
    HttpResponse::Accepted().finish()
}

pub fn init(cfg: &mut web::ServiceConfig) {
    cfg.service(server_info);
}