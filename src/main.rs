use std::convert::TryInto;

use color_eyre::eyre::Result;
use tracing::{info, metadata::LevelFilter};
use tracing_actix_web::TracingLogger;
use tracing_subscriber::FmtSubscriber;
use sqlx::{MySqlPool, mysql::MySqlConnectOptions};
use actix_web::{web, App, HttpServer};
use uuid::Uuid;

mod config;
mod users;
mod state;
mod server;

pub type Pool = MySqlPool;

const SERVER_ID: &str = "server_id";

#[actix_web::main]
async fn main() -> Result<()>{
    color_eyre::install()?;

    let subscriber = FmtSubscriber::builder()
        // all spans/events with a level higher than TRACE (e.g, debug, info, warn, etc.)
        // will be written to stdout.
        .with_max_level(LevelFilter::TRACE)
        // completes the builder.
        .finish();

    tracing::subscriber::set_global_default(subscriber)
        .expect("setting default subscriber failed");
    info!("Starting {} {}",env!("CARGO_BIN_NAME"),env!("CARGO_PKG_VERSION"));

    let config = config::Settings::new()?;

    let mut options = MySqlConnectOptions::new()
        .host(&config.database.host)
        .port(config.database.port)
        .username(&config.database.user)
        .database(&config.database.db);
    if let Some(pw) = config.database.password {
        options = options.password(&pw);
    }
    let db_pool = MySqlPool::connect_with(options).await?;
    let server_id = match server::load_setting(&db_pool,SERVER_ID).await? {
        Some(v) => v.try_into(),
        None => {
            let id = Uuid::new_v4();
            server::set_setting(SERVER_ID,id).await?;
            id
        }
    };
    sqlx::migrate!()
    .run(&db_pool)
    .await?;

    let state = web::Data::new(state::State {
        sql: db_pool,
        id: server_id
    });

    let server = HttpServer::new(move || {
        App::new()
            // pass database pool to application so we can access it inside handlers
            .app_data(state.clone())
            .wrap(TracingLogger::default())
            .configure(users::routes::init) // init user routes
            //.configure(api::app::init) // init app api routes
    })
    .bind((config.listen_ip.as_ref(), config.listen_port))?;

    info!("Starting server, listening on {}:{}",config.listen_ip,config.listen_port);
    server.run().await?;
    info!("Shutting down");

    Ok(())
}
