use chrono::{Duration, Utc};

use crate::prelude::*;
use crate::prelude::tests::*;
use super::models::*;
use super::*;

mod deleted_lists;
mod changed_lists;

fn timestamp(ts: &str) -> Timestamp {
    Timestamp::parse_from_str(ts, "%Y-%m-%d %H:%M:%S").unwrap()
}

/// Insert list, test only
async fn insert_list(sql: &mut DbConn, user: &Uuid, list: &ListChangedEntryRecv) {
    sqlx::query("INSERT INTO lists (owner,uuid,name,name_a,name_b,changed,created) VALUES(?,?,?,?,?,?,?)")
        .bind(user).bind(list.uuid).bind(&list.name).bind(&list.name_a).bind(&list.name_b).bind(list.changed).bind(list.created)
        .execute(sql)
        .await.unwrap();
}

/// Insert list permissions, test only
async fn insert_list_perm(sql: &mut DbConn, user: &Uuid, list: &Uuid,change: bool, reshare: bool) {
    let t_now = Utc::now().naive_utc();
    sqlx::query("INSERT INTO list_permissions (user,list,`write`,`reshare`,changed) VALUES(?,?,?,?,?)
        ON DUPLICATE KEY UPDATE changed=VALUES(changed), `write`=VALUES(`write`), reshare=VALUES(reshare)")
        .bind(user).bind(list).bind(change).bind(&reshare).bind(t_now)
        .execute(sql)
        .await.unwrap();
}

fn gen_list(date: Option<&str>) -> ListChangedEntryRecv {
    let mut rng = rand::thread_rng();
    let created = if let Some(date) = date {
        timestamp(date)
    } else {
        random_naive_date(&mut rng,true)
    };
    ListChangedEntryRecv {
        uuid: Uuid::new_v4(),
        name: random_string(&mut rng,7),
        name_a: random_string(&mut rng,7),
        name_b: random_string(&mut rng,7),
        changed: created.clone(),
        created: created,
    }
}