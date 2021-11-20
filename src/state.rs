use sqlx::MySqlPool;
use uuid::Uuid;

#[derive(Debug)]
pub struct State {
    // pub config: Config,
    pub sql: MySqlPool,
    // pub kv: KvPool,
    pub id: Uuid,
}

pub type AppState = actix_web::web::Data<State>;