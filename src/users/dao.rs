use ormx::Insert;
use uuid::Uuid;
use super::user::*;
use crate::state::AppState;

use super::user::RegisterClaims;

// no async traits and I'd like to avoid async_trait

pub async fn register_user(state: &AppState, claims: RegisterClaims, auth_key: Vec<u8>) -> sqlx::Result<Uuid> {
    let mut transaction = state.sql.begin().await?;
    let user = InsertUser {
        name: claims.name,
        delete_after: claims.delete_after,
        uuid: claims.iss,
        locked: None,
    }.insert(&mut transaction).await?;
    let ins = InsertUserKey {
        user_id: user.uuid,
        auth_key,
    }.insert(&mut transaction).await?;
    transaction.commit().await?;
    Ok(ins.user_id)
}