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

#[derive(Debug, Serialize, Deserialize, sqlx::FromRow)]
pub struct ListEntry {
    pub id: Uuid,
    pub tip: String,
    pub meanings: Vec<EntryMeaning>,
}

#[derive(Debug, Serialize, Deserialize, sqlx::FromRow)]
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
pub struct EntryChange {
    pub tip: String,
    pub meanings: Vec<EntryMeaning>,
}

pub type EntryCreate = EntryChange;