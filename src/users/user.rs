use std::fmt;

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

#[derive(ormx::Table, Serialize)]
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

#[derive(Debug, Deserialize)]
pub struct RegisterClaims {
    pub iss: Uuid,
    pub name: String,
    pub delete_after: Option<u32>,
}

#[derive(Debug, Deserialize)]
pub struct AccLoginKey {
    pub iss: Uuid,
    pub proof: String,
}

#[derive(Debug, Deserialize)]
pub struct AccLoginPassword {
    pub email: String,
    pub password: String,
}

#[derive(Debug, Deserialize)]
pub struct AccBindPassword {
    pub iss: Uuid,
    pub data: String,
}

#[derive(Deserialize)]
pub struct LoginClaims {
    pub iss: Uuid
}

#[derive(Deserialize)]
pub struct PasswordBindRequest {
    pub password: String,
    pub email: String,
}

// don't print passwords into the log
impl fmt::Debug for PasswordBindRequest {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Point")
         .field("password", &self.password.len())
         .field("email", &self.email)
         .finish()
    }
}

#[derive(Debug, ormx::Table, Serialize)]
#[ormx(table = "user_login", id = user_id, insertable)]
pub struct UserLogin {
    #[ormx(get_one = by_user_uuid(&Uuid))]
    #[ormx(get_optional = by_user_uuid_opt(&Uuid))]
    #[ormx(custom_type)]
    pub user_id: Uuid,
    #[ormx(get_optional = by_email_opt(&str))]
    pub email: String,
    pub password: String,
    #[ormx(custom_type)]
    pub verified: bool,
}