use std::fmt;

use strum::EnumString;
use crate::prelude::*;

#[derive(Debug, sqlx::FromRow, Serialize)]
pub struct User {
    pub uuid: Uuid,
    pub name: String,
    pub locked: Option<String>,
    pub last_seen: Timestamp,
    pub delete_after: Option<u32>,
}

// Patches can be used to update multiple fields at once (in diesel, they're called "ChangeSets").
// #[derive(ormx::Patch)]
// #[ormx(table_name = "users", table = User, id = "uuid")]
// pub struct UpdateName {
//     pub name: String,
//     pub delete_after: Option<u32>,
//     pub locked: Option<String>,
// }

#[derive(sqlx::FromRow, Serialize)]
pub struct UserKey{
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

#[derive(Debug, Clone, PartialEq, EnumString, sqlx::Type, Serialize, Deserialize)]
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
        f.debug_struct("PasswordBindRequest")
         .field("password length ", &self.password.len())
         .field("email", &self.email)
         .finish()
    }
}

#[derive(Debug, sqlx::FromRow, Serialize)]
pub struct UserLogin {
    pub user_id: Uuid,
    pub email: String,
    pub password: String,
    pub verified: bool,
}