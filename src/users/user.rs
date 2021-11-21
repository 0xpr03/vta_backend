use serde::{Deserialize, Serialize};
use sqlx::types::{chrono::{DateTime, Utc}};
use strum::EnumString;
use uuid::Uuid;

#[derive(Debug, ormx::Table, Serialize)]
#[ormx(table = "users", id = uuid, insertable)]
pub struct User {
    // generate `User::get_by_user_id(u32) -> Result<Self>`
    #[ormx(get_one = by_user_uuid(&Uuid))]
    #[ormx(get_optional = by_user_uuid_opt(&Uuid))]
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
    #[ormx(get_optional = by_user_uuid_opt(&Uuid))]
    #[ormx(get_one = by_user_uuid(Uuid))]
    #[ormx(custom_type)]
    pub user_id: Uuid,
    pub auth_key: Vec<u8>,
    pub key_type: i32,
}

#[derive(Debug)]
pub struct UserKeyParsed {
    pub auth_key: Vec<u8>,
    pub key_type: KeyType,
}

#[derive(Debug, Deserialize)]
pub struct AccRegister {
    pub key: String,
    pub keytype: KeyType,
    pub proof: String,
}

#[derive(Debug, EnumString, sqlx::Type, Serialize, Deserialize)]
#[allow(non_camel_case_types)]
pub enum KeyType {
    RSA_PEM,
    EC_PEM
}

#[derive(Debug,sqlx::FromRow)]
pub struct KeyTypeRecord {
    pub name: KeyType,
}

#[derive(Deserialize)]
pub struct RegisterClaims {
    pub iss: Uuid,
    pub name: String,
    pub delete_after: Option<u32>,
}

#[derive(Debug, Deserialize)]
pub struct AccLogin {
    pub iss: Uuid,
    pub proof: String,
}

#[derive(Deserialize)]
pub struct LoginClaims {
    pub iss: Uuid
}