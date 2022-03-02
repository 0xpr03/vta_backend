use chrono::{Duration, NaiveDateTime, Utc};
use rand::Rng;

use super::models::*;
use super::*;
use crate::prelude::tests::*;
use crate::prelude::*;

mod changed_entries;
mod changed_lists;
mod deleted_entries;
mod deleted_lists;

fn timestamp(ts: &str) -> Timestamp {
    Timestamp::parse_from_str(ts, "%Y-%m-%d %H:%M:%S").unwrap()
}

/// Insert entries, test only
async fn insert_entries(sql: &mut DbConn, entries: &[EntryChangedEntry]) {
    let t_now = Utc::now().naive_utc();
    for e in entries {
        sqlx::query("INSERT INTO entries (list,uuid,changed,updated,tip) VALUES (?,?,?,?,?)")
            .bind(e.list)
            .bind(e.uuid)
            .bind(e.changed)
            .bind(t_now)
            .bind(&e.tip)
            .execute(&mut *sql)
            .await
            .unwrap();

        for m in e.meanings.iter() {
            sqlx::query("INSERT INTO entry_meaning (entry,value,is_a) VALUES (?,?,?)")
                .bind(e.uuid)
                .bind(&m.value)
                .bind(m.is_a)
                .execute(&mut *sql)
                .await
                .unwrap();
        }
    }
}

/// Insert list, test only
async fn insert_list(sql: &mut DbConn, user: &UserId, list: &ListChangedEntryRecv) {
    sqlx::query(
        "INSERT INTO lists (owner,uuid,name,name_a,name_b,changed,created) VALUES(?,?,?,?,?,?,?)",
    )
    .bind(user.0)
    .bind(list.uuid)
    .bind(&list.name)
    .bind(&list.name_a)
    .bind(&list.name_b)
    .bind(list.changed)
    .bind(list.created)
    .execute(sql)
    .await
    .unwrap();
}

/// Insert list permissions, test only
async fn insert_list_perm(
    sql: &mut DbConn,
    user: &UserId,
    list: &Uuid,
    change: bool,
    reshare: bool,
) {
    let t_now = Utc::now().naive_utc();
    sqlx::query("INSERT INTO list_permissions (user,list,`write`,`reshare`,changed) VALUES(?,?,?,?,?)
        ON DUPLICATE KEY UPDATE changed=VALUES(changed), `write`=VALUES(`write`), reshare=VALUES(reshare)")
        .bind(user.0).bind(list).bind(change).bind(&reshare).bind(t_now)
        .execute(sql)
        .await.unwrap();
}

fn gen_list(date: Option<&str>) -> ListChangedEntryRecv {
    let mut rng = rand::thread_rng();
    let created = if let Some(date) = date {
        timestamp(date)
    } else {
        random_naive_date(&mut rng, true)
    };
    ListChangedEntryRecv {
        uuid: Uuid::new_v4(),
        name: random_string(&mut rng, 7),
        name_a: random_string(&mut rng, 7),
        name_b: random_string(&mut rng, 7),
        changed: created.clone(),
        created: created,
    }
}

fn gen_entry(list: &Uuid, date: Option<NaiveDateTime>) -> EntryChangedEntry {
    let mut rng = rand::thread_rng();
    let created = if let Some(date) = date {
        date
    } else {
        random_naive_date(&mut rng, true)
    };

    let mut v = Vec::new();
    for _ in 0..rng.gen_range(0..10) {
        v.push(Meaning {
            value: random_string(&mut rng, 7),
            is_a: rng.gen(),
        });
    }
    EntryChangedEntry {
        uuid: Uuid::new_v4(),
        changed: created,
        list: list.clone(),
        tip: random_string(&mut rng, 7),
        meanings: v,
    }
}
