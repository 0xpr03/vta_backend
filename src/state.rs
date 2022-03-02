use std::fmt;

use crate::prelude::*;
use serde::{Deserialize, Serialize};
use sqlx::MySqlPool;
use uuid::Uuid;

/// Login cookie data
#[derive(Debug, Serialize, Deserialize)]
pub struct LoginCookie {
    pub id: Uuid,
    pub valid_till: Timestamp,
    pub key_login: bool,
}

pub struct State {
    // pub config: Config,
    pub sql: MySqlPool,
    // pub kv: KvPool,
    pub id: Uuid,
}

// required for actix-tracing
impl fmt::Debug for State {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("AppState").finish()
    }
}

pub type AppState = actix_web::web::Data<State>;
