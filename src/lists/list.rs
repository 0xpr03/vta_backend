use std::convert::TryFrom;
use std::hash::{Hash, Hasher};

use crate::prelude::*;

pub enum LastSyncedKind {
    ListsDeleted = 1,
    Lists = 2,
    EntriesDeleted = 3,
    Entries = 4,
}

impl TryFrom<i32> for LastSyncedKind {
    type Error = ();

    fn try_from(v: i32) -> Result<Self, Self::Error> {
        use LastSyncedKind::*;
        match v {
            x if x == ListsDeleted as i32 => Ok(ListsDeleted),
            x if x == Lists as i32 => Ok(Lists),
            x if x == EntriesDeleted as i32 => Ok(EntriesDeleted),
            x if x == Entries as i32 => Ok(Entries),
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