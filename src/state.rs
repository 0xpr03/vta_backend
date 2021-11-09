use sqlx::MySqlPool;

#[derive(Debug)]
pub struct State {
    // pub config: Config,
    pub sql: MySqlPool,
    // pub kv: KvPool,
}
pub type AppState = actix_web::web::Data<State>;