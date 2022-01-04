use chrono::Duration;

use crate::prelude::*;
use crate::prelude::tests::*;
use super::models::*;
use super::*;

#[actix_rt::test]
async fn test_deleted_lists() {
    let db = DatabaseGuard::new().await;
    let mut conn = &mut *db.conn().await;
    let mut rng = rand::thread_rng();

    let user = register_test_user(&mut conn, &mut rng).await;

    // generate some lists and insert them
    let lists = vec![gen_list(None),gen_list(None),gen_list(None)];

    for l in lists.iter() {
        insert_list(&mut conn, &user,l).await;
    }
    // construct delete requests
    let del_req = ListDeletedRequest {
        client: Uuid::new_v4(),
        lists: vec![
            // unknown list
            ListDeleteEntry {list: Uuid::new_v4(), time: timestamp("2015-05-01 02:03:04")},
            // known list
            ListDeleteEntry {list: lists[0].uuid.clone(), time: timestamp("2015-05-01 02:03:04")},
            // Known but future date
            ListDeleteEntry {list: lists[1].uuid.clone(), time: random_future_date(&mut rng)}
        ]
    };
    let t_now = chrono::Utc::now().naive_utc();
    let res = dao::update_deleted_lists(&mut conn, del_req.clone(), &user).await.unwrap();
    // no previous data, and sanity check
    assert_eq!(0,res.lists.len());
    assert_eq!(0,res.unowned.len());
    // one unknown list delete entry
    assert_eq!(1,res.unknown.len());
    assert!(res.unknown.contains(&del_req.lists[0].list));

    // retrieve all changes as new client
    let empty_data = ListDeletedRequest{client: Uuid::new_v4(), lists: Vec::new()};
    let res = dao::update_deleted_lists(&mut conn, empty_data, &&user).await.unwrap();
    assert_eq!(2,res.lists.len());
    // valid entry
    let first = res.lists.get(&del_req.lists[1].list).expect("expected valid list in delta not found");
    assert_eq!(first.time,del_req.lists[1].time);
    // second valid but time should be corrected
    let second = res.lists.get(&del_req.lists[2].list).expect("expected time-correct list in delta not found");
    assert_ne!(second.time,del_req.lists[2].time);
    assert!(second.time - t_now < Duration::seconds(1));

    // again with 1 old, 1 new entry testing delta + deduplication of return
    let del_eq_2 = ListDeletedRequest {
        client: Uuid::new_v4(),
        lists: vec![
            // old, existing entry but unsend to this client
            ListDeleteEntry {list: lists[1].uuid.clone(), time: random_naive_date(&mut rng, true)},
            // new valid entry
            ListDeleteEntry {list: lists[2].uuid.clone(), time: random_future_date(&mut rng)}
        ]
    };
    let res = dao::update_deleted_lists(&mut conn, del_eq_2.clone(), &user).await.unwrap();
    assert_eq!(1,res.lists.len());
    assert_eq!(0,res.unowned.len());
    assert_eq!(0,res.unknown.len());
    let item = res.lists.get(&lists[0].uuid).expect("expected list from delta not found");
    assert_eq!(item.time,del_req.lists[1].time);

    db.drop_async().await;
}
 
#[actix_rt::test]
async fn test_deleted_lists_shared() {
    // Test that deleted lists, which were shared
    // are also tracked for the users which whom the lists were shared.
    // Also the same should be true when the list owner deleted their account.

    let db = DatabaseGuard::new().await;
    let mut conn = &mut *db.conn().await;
    let mut rng = rand::thread_rng();

    let user_1 = register_test_user(&mut conn, &mut rng).await;
    let user_2 = register_test_user(&mut conn, &mut rng).await;
    let user_3 = register_test_user(&mut conn, &mut rng).await;

    // generate some lists and insert them
    let lists = vec![gen_list(None),gen_list(None),gen_list(None)];

    insert_list(&mut conn, &user_1,&lists[0]).await;
    insert_list(&mut conn, &user_2,&lists[1]).await;
    insert_list(&mut conn, &user_3,&lists[2]).await;

    // user2 gets access to user1 list, changeable
    insert_list_perm(&mut conn, &user_2,&lists[0].uuid,true,false).await;
    // user3 gets access to user1 list, non-changeable
    insert_list_perm(&mut conn, &user_3,&lists[0].uuid,false,false).await;
    
    // construct delete requests
    let del_req = ListDeletedRequest {
        client: Uuid::new_v4(),
        lists: vec![
            // known list
            ListDeleteEntry {list: lists[0].uuid.clone(), time: random_naive_date(&mut rng, true)},
        ]
    };
    // now try to delete that from user2
    let res = dao::update_deleted_lists(&mut conn, del_req.clone(), &user_2).await.unwrap();
    assert_eq!(0,res.lists.len());
    assert_eq!(0,res.unknown.len());
    // and this user shouldn't be allowed, no owner
    assert_eq!(1,res.unowned.len());
    assert!(res.unowned.contains(&del_req.lists[0].list));

    let res = dao::update_deleted_lists(&mut conn, del_req.clone(), &user_3).await.unwrap();
    assert_eq!(0,res.lists.len());
    assert_eq!(0,res.unknown.len());
    // same for user 3 with read-only
    assert_eq!(1,res.unowned.len());
    assert!(res.unowned.contains(&del_req.lists[0].list));

    // Now delete it from user1, owner
    let res = dao::update_deleted_lists(&mut conn, del_req.clone(), &user_1).await.unwrap();
    assert_eq!(0,res.lists.len());
    assert_eq!(0,res.unknown.len());
    assert_eq!(0,res.unowned.len());

    // which should be visible for user2
    let new_req = ListDeletedRequest {client: Uuid::new_v4(),lists: Vec::new()};
    let res = dao::update_deleted_lists(&mut conn, new_req.clone(), &user_2).await.unwrap();
    assert_eq!(1,res.lists.len());
    assert_eq!(0,res.unknown.len());
    assert_eq!(0,res.unowned.len());
    assert_ne!(None,res.lists.get(&del_req.lists[0].list));
    // and user 3
    let res = dao::update_deleted_lists(&mut conn, new_req, &user_3).await.unwrap();
    assert_eq!(1,res.lists.len());
    assert_eq!(0,res.unknown.len());
    assert_eq!(0,res.unowned.len());
    assert_ne!(None,res.lists.get(&del_req.lists[0].list));

    db.drop_async().await;
}

fn timestamp(ts: &str) -> Timestamp {
    Timestamp::parse_from_str(ts, "%Y-%m-%d %H:%M:%S").unwrap()
}

async fn insert_list(sql: &mut DbConn, user: &Uuid, list: &ListChangedEntry) {
    sqlx::query("INSERT INTO lists (owner,uuid,name,name_a,name_b,changed,created) VALUES(?,?,?,?,?,?,?)")
        .bind(user).bind(list.uuid).bind(&list.name).bind(&list.name_a).bind(&list.name_b).bind(list.changed).bind(list.created)
        .execute(sql)
        .await.unwrap();
}

async fn insert_list_perm(sql: &mut DbConn, user: &Uuid, list: &Uuid,change: bool, reshare: bool) {
    sqlx::query("INSERT INTO list_permissions (user,list,`change`,`reshare`) VALUES(?,?,?,?)")
        .bind(user).bind(list).bind(change).bind(&reshare)
        .execute(sql)
        .await.unwrap();
}

fn gen_list(date: Option<&str>) -> ListChangedEntry {
    let mut rng = rand::thread_rng();
    let created = if let Some(date) = date {
        timestamp(date)
    } else {
        random_naive_date(&mut rng,true)
    };
    ListChangedEntry {
        uuid: Uuid::new_v4(),
        name: random_string(&mut rng,7),
        name_a: random_string(&mut rng,7),
        name_b: random_string(&mut rng,7),
        changed: created.clone(),
        created: created,
    }
}