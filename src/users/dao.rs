use std::convert::TryInto;
use std::str::FromStr;

use ormx::Insert;
use crate::prelude::*;
use super::user::*;
use super::Result;

// no async traits and I'd like to avoid async_trait
#[instrument]
pub async fn register_user(state: &AppState, claims: RegisterClaims, auth_key: Vec<u8>, key_type: KeyType) -> Result<Uuid> {
    let mut transaction = state.sql.begin().await?;
    let user = InsertUser {
        name: claims.name,
        delete_after: claims.delete_after,
        uuid: claims.iss,
        locked: None,
    }.insert(&mut transaction).await?;
    
    let type_id: i32 = match sqlx::query!("SELECT id FROM key_type WHERE name = ?",key_type).fetch_optional(&mut transaction).await? {
        Some(r) => r.id,
        None => {
            let res = sqlx::query!("INSERT INTO key_type (name) VALUES(?)",key_type).execute(&mut transaction).await?;
            res.last_insert_id().try_into().unwrap()
        }
    };

    sqlx::query!("INSERT INTO user_key (user_id,auth_key,key_type) VALUES(?,?,?)",user.uuid,auth_key,type_id).execute(&mut transaction).await?;
    // let ins = InsertUserKey {
    //     user_id: user.uuid,
    //     auth_key,
    //     key_type: type_id,
    // }.insert(&mut transaction).await?;

    transaction.commit().await?;
    Ok(user.uuid)
}

#[instrument]
pub async fn user_key(state: &AppState, user: &Uuid) -> Result<Option<UserKeyParsed>> {
    if let Some(raw) = UserKey::by_user_uuid_opt(&state.sql, user).await? {
        let r = sqlx::query!("SELECT name FROM key_type WHERE id = ?",raw.key_type).fetch_one(&state.sql).await?;
        let p_type = KeyType::from_str(&r.name).map_err(color_eyre::eyre::Error::from)?;
        Ok(Some(
            UserKeyParsed {
                auth_key: raw.auth_key,
                key_type: p_type,
            }
        ))
    } else {
        Ok(None)
    }
}

// let type_id: KeyType = match sqlx::query!("SELECT name FROM key_type WHERE id = ?",key_type).fetch_optional(&mut transaction).await? {
//     Some(r) => KeyType::from_str(&r.name).map_err(color_eyre::eyre::Error::from)?,
//     None => {
//         let res = sqlx::query!("INSERT INTO key_type (name) VALUES(?)",key_type).execute(&mut transaction).await?;
//         res.last_insert_id()
//     }
// };