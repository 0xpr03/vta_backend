use std::fmt;

use crate::prelude::*;

pub struct ListId(pub Uuid);
pub struct EntryId(pub Uuid);
pub struct UserId(pub Uuid);

impl fmt::Display for ListId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}
impl fmt::Display for EntryId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}
impl fmt::Display for UserId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}


#[derive(Debug, Serialize, Deserialize, sqlx::FromRow)]
pub struct List {
    pub id: Uuid,
    pub uuid: Uuid,
    pub name: String,
    pub name_a: String,
    pub name_b: String,
    pub foreign: bool,
    pub change: bool,
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

#[derive(Debug, Deserialize)]
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