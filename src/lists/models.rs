use std::fmt;

use crate::prelude::*;

#[derive(Debug, Serialize, Deserialize, sqlx::FromRow)]
pub struct List {
    pub uuid: Uuid,
    pub name: String,
    pub name_a: String,
    pub name_b: String,
    pub foreign: bool,
    pub change: bool,
}

/// A user with which a list is shared
#[derive(Debug, Serialize, Deserialize, sqlx::FromRow)]
pub struct SharedUser {
    pub uuid: Uuid,
    pub name: String,
    pub write: bool,
    pub reshare: bool,
}

#[derive(Debug, Deserialize)]
pub struct UserPermissions {
    pub write: bool,
    pub reshare: bool,
}

#[derive(Debug, Deserialize)]
pub struct NewTokenData {
    pub write: bool,
    pub reshare: bool,
    pub reusable: bool,
    pub deadline: Timestamp,
}

#[derive(Debug, Serialize)]
pub struct ShareTokenReturn {
    pub token_a: String,
    pub token_b: String,
}

#[derive(Debug, sqlx::FromRow)]
pub struct ShareTokenEntry {
    pub list: Uuid,
    pub deadline: Timestamp,
    pub hash: Vec<u8>,
    pub write: bool,
    pub reshare: bool,
    pub reusable: bool,
}

#[derive(Debug, Serialize, Deserialize, sqlx::FromRow)]
#[cfg_attr(test, derive(PartialEq,Clone))]
pub struct EntryMeaning {
    pub value: String,
    pub is_a: bool,
}

#[derive(Debug, Deserialize, Clone)]
pub struct ListChange {
    pub name: String,
    pub name_a: String,
    pub name_b: String
}

pub type ListCreate = ListChange;

#[derive(Debug, Serialize, Deserialize)]
#[cfg_attr(test, derive(Clone))]
pub struct EntryChange {
    pub tip: String,
    pub meanings: Vec<EntryMeaning>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Entry {
    pub tip: String,
    pub uuid: Uuid,
    pub meanings: Vec<EntryMeaning>,
}

pub type EntryCreate = EntryChange;