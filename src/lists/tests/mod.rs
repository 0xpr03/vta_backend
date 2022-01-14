use chrono::Utc;
use rand::prelude::ThreadRng;

use crate::prelude::*;
use crate::prelude::tests::*;
use super::models::*;
use super::*;

mod list_basics;

fn gen_list(rng: &mut ThreadRng) -> ListChange {
    ListChange {
        name: random_string(&mut *rng, 7),
        name_a: random_string(&mut *rng, 7),
        name_b: random_string(&mut *rng, 7),
    }
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