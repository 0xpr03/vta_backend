use std::borrow::Cow;
use std::collections::{HashMap, HashSet};
use std::convert::TryFrom;
use std::hash::{Hash, Hasher};
use sqlx::Row;

use super::InvalidPermissionError;
use crate::prelude::*;

pub enum LastSyncedKind {
    ListsDeleted = 1,
    ListsChanged = 2,
    EntriesDeleted = 3,
    EntriesChanged = 4,
}

impl TryFrom<i32> for LastSyncedKind {
    type Error = ();

    fn try_from(v: i32) -> Result<Self, Self::Error> {
        use LastSyncedKind::*;
        match v {
            x if x == ListsDeleted as i32 => Ok(ListsDeleted),
            x if x == ListsChanged as i32 => Ok(ListsChanged),
            x if x == EntriesDeleted as i32 => Ok(EntriesDeleted),
            x if x == EntriesChanged as i32 => Ok(EntriesChanged),
            _ => Err(()),
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
pub struct ListDeletedRequest {
    pub since: Option<Timestamp>,
    pub lists: Vec<Uuid>,
}

/// Server response to client for list delete sync
#[derive(Debug, Serialize)]
pub struct ListDeletedResponse {
    /// Delta of deleted lists for the client to store
    pub delta: HashSet<Uuid>,
    /// Lists that the server didn't know, thus no tombstone stored
    pub unknown: Vec<Uuid>,
    /// Lists for which the client doesn't have owner rights to delete them
    pub unowned: Vec<Uuid>,
}

#[derive(Debug, Deserialize, Clone)]
pub struct ListChangedRequest {
    pub since: Option<Timestamp>,
    pub lists: Vec<ListChangedEntryRecv>,
}

#[derive(Debug, Serialize, PartialEq)]
pub enum ListPermissions {
    Owner,
    Read,
    Write,
}

/// List change entry, received from clients
#[derive(Debug, Deserialize, Clone)]
pub struct ListChangedEntryRecv {
    pub uuid: Uuid,
    pub name: String,
    pub name_a: String,
    pub name_b: String,
    pub changed: Timestamp,
    pub created: Timestamp,
}

/// List change entry, send to clients
#[derive(Debug, Serialize)]
pub struct ListChangedEntrySend {
    pub permissions: ListPermissions,
    pub uuid: Uuid,
    pub name: String,
    pub name_a: String,
    pub name_b: String,
    pub changed: Timestamp,
    pub created: Timestamp,
}

impl sqlx::FromRow<'_, sqlx::mysql::MySqlRow> for ListChangedEntrySend {
    fn from_row(row: &sqlx::mysql::MySqlRow) -> sqlx::Result<Self> {
        let p_t: i32 = row.try_get("permissions")?;
        let permissions = match p_t {
            -1 => ListPermissions::Owner,
            0 => ListPermissions::Read,
            1 => ListPermissions::Write,
            x => return Err(sqlx::Error::Decode(Box::new(InvalidPermissionError{found: x}))),
        };
        Ok(ListChangedEntrySend {
            permissions,
            uuid: row.try_get("uuid")?,
            name: row.try_get("name")?,
            name_a: row.try_get("name_a")?,
            name_b: row.try_get("name_b")?,
            changed: row.try_get("changed")?,
            created: row.try_get("created")?,
        })
    }
}

// // derived via macro expansion for sqlx::FromRow without the unsupported type
// impl<'a, R: ::sqlx::Row> ::sqlx::FromRow<'a, R> for ListChangedEntry
//     where
//         &'a ::std::primitive::str: ::sqlx::ColumnIndex<R>,
//         Uuid: ::sqlx::decode::Decode<'a, R::Database>,
//         Uuid: ::sqlx::types::Type<R::Database>,
//         String: ::sqlx::decode::Decode<'a, R::Database>,
//         String: ::sqlx::types::Type<R::Database>,
//         Timestamp: ::sqlx::decode::Decode<'a, R::Database>,
//         Timestamp: ::sqlx::types::Type<R::Database>,
//     {
//         fn from_row(row: &'a R) -> ::sqlx::Result<Self> {
//             let uuid: Uuid = row.try_get("uuid")?;
//             let name: String = row.try_get("name")?;
//             let name_a: String = row.try_get("name_a")?;
//             let name_b: String = row.try_get("name_b")?;
//             let changed: Timestamp = row.try_get("changed")?;
//             let created: Timestamp = row.try_get("created")?;
//             ::std::result::Result::Ok(ListChangedEntry {
//                 uuid,
//                 name,
//                 name_a,
//                 name_b,
//                 changed,
//                 created,
//             })
//         }
//     }

// impl sqlx::FromRow for ListChangedEntry {
//     fn from_row(row: &Row) -> Result<ListChangedEntry> {

//     }
// }

#[derive(Debug, Serialize)]
pub struct ListChangedResponse {
    pub delta: HashMap<Uuid,ListChangedEntrySend>,
    pub failures: Vec<EntrySyncFailure>,
}

#[derive(Debug, Serialize)]
pub struct EntrySyncFailure {
    pub id: Uuid,
    pub error: Cow<'static,str>,
}

#[derive(Debug, Deserialize, Clone)]
pub struct EntryDeletedRequest {
    pub since: Option<Timestamp>,
    pub entries: Vec<EntryDeleteEntry>,
}

#[derive(Debug, Serialize, Deserialize, Clone, sqlx::FromRow)]
pub struct EntryDeleteEntry {
    pub list: Uuid,
    pub entry: Uuid
}

impl Hash for EntryDeleteEntry {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.entry.hash(state);
    }
}
impl PartialEq for EntryDeleteEntry {
    fn eq(&self, other: &Self) -> bool {
        self.entry == other.entry
    }
}
impl Eq for EntryDeleteEntry {}

#[derive(Debug, Serialize)]
pub struct EntryDeletedResponse {
    pub delta: HashMap<Uuid, EntryDeleteEntry>,
    pub ignored: Vec<Uuid>,
    pub invalid: Vec<Uuid>,
}

#[derive(Debug, Deserialize)]
pub struct EntryChangedRequest {
    pub since: Option<Timestamp>,
    pub entries: Vec<EntryChangedEntry>,
}

#[derive(Debug, Serialize, Deserialize)]
#[cfg_attr(test, derive(Clone))]
pub struct EntryChangedEntry {
    pub list: Uuid,
    pub uuid: Uuid,
    pub tip: String,
    pub changed: Timestamp,
    pub meanings: Vec<Meaning>,
}

impl Hash for EntryChangedEntry {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.uuid.hash(state);
    }
}
impl PartialEq for EntryChangedEntry {
    fn eq(&self, other: &Self) -> bool {
        self.uuid == other.uuid
    }
}
impl Eq for EntryChangedEntry {}

// raw entry from DB without meanings
#[derive(Debug, sqlx::FromRow)]
pub struct EntryChangedEntryBlank {
    pub list: Uuid,
    pub uuid: Uuid,
    pub changed: Timestamp,
    pub tip: String,
}

impl EntryChangedEntryBlank {
    pub fn into_full(self, meanings: Vec<Meaning>) -> EntryChangedEntry {
        EntryChangedEntry {
            list: self.list,
            uuid: self.uuid,
            tip: self.tip,
            changed: self.changed,
            meanings,
        }
    }
}

#[derive(Debug, Serialize, Deserialize, sqlx::FromRow)]
#[cfg_attr(test, derive(Clone, PartialEq))]
pub struct Meaning {
    pub value: String,
    pub is_a: bool,
}

#[derive(Debug, Serialize)]
pub struct EntryChangedResponse {
    pub delta: HashMap<Uuid,EntryChangedEntry>,
    pub ignored: Vec<Uuid>,
    pub invalid: Vec<Uuid>,
}