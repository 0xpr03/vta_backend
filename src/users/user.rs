use serde::{Deserialize, Serialize};
use sqlx::types::{chrono::{DateTime, Utc}};
use uuid::Uuid;

#[derive(Debug, ormx::Table, Serialize)]
#[ormx(table = "users", id = uuid, insertable)]
pub struct User {
    // generate `User::get_by_user_id(u32) -> Result<Self>`
    #[ormx(get_one = get_by_user_uuid(Uuid))]
    #[ormx(custom_type)]
    pub uuid: Uuid,
    pub name: String,
    pub locked: Option<String>,
    // don't include this field into `InsertUser` since it has a default value
    // generate `User::set_last_login(Option<NaiveDateTime>) -> Result<()>`
    #[ormx(default, set)]
    pub last_seen: DateTime<Utc>,
    pub delete_after: Option<u32>,
}

// Patches can be used to update multiple fields at once (in diesel, they're called "ChangeSets").
#[derive(ormx::Patch)]
#[ormx(table_name = "users", table = User, id = "uuid")]
pub struct UpdateName {
    pub name: String,
    pub delete_after: Option<u32>,
    pub locked: Option<String>,
}

#[derive(Debug, ormx::Table, Serialize)]
#[ormx(table = "user_key", id = user_id, insertable)]
pub struct UserKey{
    #[ormx(get_one = get_by_user_uuid(Uuid))]
    #[ormx(custom_type)]
    pub user_id: Uuid,
    pub auth_key: Vec<u8>,
}

#[derive(Debug, Deserialize)]
pub struct AccRegister {
    pub key: String,
    pub proof: String,
}

#[derive(Deserialize)]
pub struct RegisterClaims {
    pub iss: Uuid,
    pub name: String,
    pub delete_after: Option<u32>,
}