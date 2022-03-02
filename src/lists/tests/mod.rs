use chrono::Utc;
use futures::TryStreamExt;
use rand::prelude::ThreadRng;
use rand::Rng;
use sqlx::MySqlConnection;

use super::models::*;
use super::*;
use crate::prelude::tests::*;

mod list_basics;
mod sharing;

fn gen_list(rng: &mut ThreadRng) -> ListChange {
    ListChange {
        name: random_string(&mut *rng, 7),
        name_a: random_string(&mut *rng, 7),
        name_b: random_string(&mut *rng, 7),
    }
}

fn gen_list_create(rng: &mut ThreadRng) -> ListCreate {
    ListCreate {
        name: random_string(&mut *rng, 7),
        name_a: random_string(&mut *rng, 7),
        name_b: random_string(&mut *rng, 7),
    }
}

fn gen_entry(rng: &mut ThreadRng) -> EntryChange {
    EntryChange {
        tip: random_string(&mut *rng, 7),
        meanings: gen_meanings(&mut *rng, 2),
    }
}

fn gen_meanings(rng: &mut ThreadRng, amount: usize) -> Vec<EntryMeaning> {
    (0..amount)
        .into_iter()
        .map(|_| EntryMeaning {
            value: random_string(&mut *rng, 7),
            is_a: rng.gen(),
        })
        .collect()
}

/// Insert list permissions, test only
async fn insert_list_perm(sql: &mut DbConn, user: &Uuid, list: &Uuid, change: bool, reshare: bool) {
    let t_now = Utc::now().naive_utc();
    sqlx::query("INSERT INTO list_permissions (user,list,`write`,`reshare`,changed) VALUES(?,?,?,?,?)
        ON DUPLICATE KEY UPDATE changed=VALUES(changed), `write`=VALUES(`write`), reshare=VALUES(reshare)")
        .bind(user).bind(list).bind(change).bind(&reshare).bind(t_now)
        .execute(sql)
        .await.unwrap();
}

async fn get_deleted_lists(sql: &mut DbConn, user: &UserId) -> Vec<Uuid> {
    let ret: Vec<Uuid> =
        sqlx::query_scalar::<_, Uuid>("SELECT list FROM deleted_list WHERE user = ?")
            .bind(user.0)
            .fetch(sql)
            .try_collect()
            .await
            .unwrap();
    ret
}

async fn get_deleted_entries(sql: &mut DbConn, list: &ListId) -> Vec<Uuid> {
    let ret: Vec<Uuid> =
        sqlx::query_scalar::<_, Uuid>("SELECT `entry` FROM deleted_entry e WHERE e.list = ?")
            .bind(list.0)
            .fetch(sql)
            .try_collect()
            .await
            .unwrap();
    ret
}

async fn list_updated_date(sql: &mut MySqlConnection, list: &ListId) -> Timestamp {
    sqlx::query_scalar::<_, Timestamp>("SELECT updated FROM lists WHERE uuid = ?")
        .bind(list.0)
        .fetch_one(sql)
        .await
        .unwrap()
}

async fn entry_updated_date(sql: &mut MySqlConnection, entry: &EntryId) -> Timestamp {
    sqlx::query_scalar::<_, Timestamp>("SELECT updated FROM entries WHERE uuid = ?")
        .bind(entry.0)
        .fetch_one(sql)
        .await
        .unwrap()
}
