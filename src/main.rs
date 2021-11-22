use std::str::FromStr;

use actix_identity::{CookieIdentityPolicy, IdentityService};
use color_eyre::eyre::Result;
use tracing::{debug, error, info, instrument};
use tracing_actix_web::TracingLogger;
use tracing_subscriber::prelude::__tracing_subscriber_SubscriberExt;
use sqlx::{Executor, MySqlPool, mysql::{MySqlConnectOptions, MySqlPoolOptions}};
use actix_web::{App, HttpServer, cookie::SameSite, web};
use uuid::Uuid;

mod config;
mod users;
mod state;
mod server;
mod sync;
mod prelude;

pub type Pool = MySqlPool;

const SERVER_ID: &str = "server_id";
const SESSION_KEY: &str = "session_key";
#[cfg(debug_assertions)]
const SECURE_COOKIE: bool = false;
#[cfg(not(debug_assertions))]
const SECURE_COOKIE: bool = true;

fn init_telemetry() {
    let app_name = env!("CARGO_BIN_NAME");

    // Start a new Jaeger trace pipeline.
    // Spans are exported in batch - recommended setup for a production application.
    opentelemetry::global::set_text_map_propagator(opentelemetry::sdk::propagation::TraceContextPropagator::new());
    let tracer = opentelemetry_jaeger::new_pipeline()
        .with_service_name(app_name)
        .install_batch(opentelemetry::runtime::TokioCurrentThread)
        .expect("Failed to install OpenTelemetry tracer.");

    // Filter based on level - trace, debug, info, warn, error
    // Tunable via `RUST_LOG` env variable
    let env_filter = tracing_subscriber::EnvFilter::try_from_default_env().unwrap_or(tracing_subscriber::EnvFilter::new("trace"));
    // Create a `tracing` layer using the Jaeger tracer
    let telemetry = tracing_opentelemetry::layer().with_tracer(tracer);
    // Create a `tracing` layer to emit spans as structured logs to stdout
    let formatting_layer = tracing_bunyan_formatter::BunyanFormattingLayer::new(app_name.into(), std::io::stdout);
    // Combined them all together in a `tracing` subscriber
    let subscriber = tracing_subscriber::Registry::default()
        .with(env_filter)
        .with(telemetry)
        .with(tracing_bunyan_formatter::JsonStorageLayer)
        .with(formatting_layer);
    tracing::subscriber::set_global_default(subscriber)
        .expect("Failed to install `tracing` subscriber.")
}

#[actix_web::main]
async fn main() -> Result<()>{
    color_eyre::install()?;

    // let subscriber = FmtSubscriber::builder()
    //     // all spans/events with a level higher than TRACE (e.g, debug, info, warn, etc.)
    //     // will be written to stdout.
    //     .with_max_level(LevelFilter::TRACE)
    //     // completes the builder.
    //     .finish();

    // tracing::subscriber::set_global_default(subscriber)
    //     .expect("setting default subscriber failed");
    init_telemetry();
    main_().await
}

#[instrument]
async fn main_() -> Result<()> {
    info!("Starting {} {}",env!("CARGO_BIN_NAME"),env!("CARGO_PKG_VERSION"));
    if !SECURE_COOKIE {
        eprintln!("Secure (httpS) cookies disabled in debug mode!");
        error!("Secure (httpS) cookies disabled in debug mode!")
    }
    let config = config::Settings::new()?;

    let mut options = MySqlConnectOptions::new()
        .host(&config.database.host)
        .port(config.database.port)
        .username(&config.database.user)
        .database(&config.database.db);
    if let Some(pw) = config.database.password {
        options = options.password(&pw);
    }

    let db_pool = MySqlPoolOptions::new().max_connections(config.database.max_conn).after_connect(|conn| Box::pin(async move {
        conn.execute("SET SESSION sql_mode=STRICT_ALL_TABLES; SET SESSION innodb_strict_mode=ON;").await?;
        Ok(())
     })).connect_with(options).await?;
    
    debug!("Migrating DB");
    let mut stm = db_pool.begin().await?;
    match sqlx::migrate!()
    .run(&mut stm)
    .await {
        Ok(_) => {stm.commit().await?;},
        Err(e) => {
            stm.rollback().await?;
            error!(?e,"Migration failed");
            return Err(e.into());
        },
    }
    debug!("Migration finished");


    let server_id = match server::load_setting(&db_pool,SERVER_ID).await? {
        Some(v) => Uuid::from_str(&v)?,
        None => {
            let id = Uuid::new_v4();
            server::set_setting(&db_pool, SERVER_ID,&id.to_hyphenated_ref().to_string(),false).await?;
            id
        }
    };
    debug!("Server id {}",server_id);

    let session_key = match server::load_setting(&db_pool,SESSION_KEY).await? {
        Some(v) => base64::decode(&v)?,
        None => {
            let random_bytes: Vec<u8> = (0..32).map(|_| { rand::random::<u8>() }).collect();
            server::set_setting(&db_pool, SESSION_KEY,&base64::encode(&random_bytes),false).await?;
            random_bytes
        }
    };

    let state = web::Data::new(state::State {
        sql: db_pool,
        id: server_id
    });

    let server = HttpServer::new(move || {
        App::new()
            // pass database pool to application so we can access it inside handlers
            .app_data(state.clone())
            .wrap(IdentityService::new(
                CookieIdentityPolicy::new(&session_key)
                    .name("auth")
                    .http_only(true)
                    .same_site(SameSite::Strict)
                    .secure(SECURE_COOKIE),
            ))
            .wrap(TracingLogger::default())
            .configure(users::routes::init) // init user routes
            .configure(server::routes::init) // init app api routes
            .configure(sync::routes::init) // init lists api routes
    })
    .bind((config.listen_ip.as_ref(), config.listen_port))?;

    info!("Starting server, listening on {}:{}",config.listen_ip,config.listen_port);
    server.run().await?;
    info!("Shutting down");
    opentelemetry::global::shutdown_tracer_provider();

    Ok(())
}