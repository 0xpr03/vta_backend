use chrono::NaiveDateTime;

pub type Timestamp = NaiveDateTime;
pub use tracing::*;
pub use uuid::Uuid;
pub use color_eyre::eyre::Context;
pub use serde::{Deserialize, Serialize};
pub use crate::state::AppState;