use std::fmt;

use sqlx::MySqlPool;
use uuid::Uuid;

pub struct State {
    // pub config: Config,
    pub sql: MySqlPool,
    // pub kv: KvPool,
    pub id: Uuid,
}

// required for actix-tracing
impl fmt::Debug for State {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("AppState")
         .finish()
    }
}


pub type AppState = actix_web::web::Data<State>;