use std::convert::TryFrom;
use std::hash::{Hash, Hasher};

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

#[derive(Debug, Deserialize)]
pub struct ListDeletedRequest {
    pub client: Uuid,
    pub lists: Vec<ListDeleteEntry>,
}

#[derive(Debug, Serialize, Deserialize, sqlx::FromRow)]
pub struct ListDeleteEntry {
    pub list: Uuid,
    pub time: Timestamp,
}

impl Hash for ListDeleteEntry {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.list.hash(state);
    }
}
impl PartialEq for ListDeleteEntry {
    fn eq(&self, other: &Self) -> bool {
        self.list == other.list
    }
}
impl Eq for ListDeleteEntry {}

#[derive(Debug, Deserialize)]
pub struct ListChangedRequest {
    pub client: Uuid,
    pub lists: Vec<ListChangedEntry>,
}

#[derive(Debug, Serialize, Deserialize, sqlx::FromRow)]
pub struct ListChangedEntry {
    pub uuid: Uuid,
    pub name: String,
    pub name_a: String,
    pub name_b: String,
    pub changed: Timestamp,
    pub created: Timestamp,
}

#[derive(Debug, Serialize)]
pub struct ListChangedResponse {
    pub lists: Vec<ListChangedEntry>,
    pub failures: Vec<EntrySyncFailure>,
}

#[derive(Debug, Serialize)]
pub struct EntrySyncFailure {
    pub id: Uuid,
    pub error: String,
}

#[derive(Debug, Deserialize)]
pub struct EntryDeletedRequest {
    pub client: Uuid,
    pub entries: Vec<EntryDeleteEntry>,
}

#[derive(Debug, Serialize, Deserialize, sqlx::FromRow)]
pub struct EntryDeleteEntry {
    pub list: Uuid,
    pub entry: Uuid,
    pub time: Timestamp
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