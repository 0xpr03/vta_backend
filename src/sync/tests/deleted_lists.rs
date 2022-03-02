use actix_rt::time::sleep;
use chrono::Duration;

use super::*;
use crate::prelude::tests::*;
use crate::prelude::*;

#[actix_rt::test]
async fn test_deleted_lists() {
    let db = DatabaseGuard::new().await;
    let mut conn = &mut *db.conn().await;
    let mut rng = rand::thread_rng();

    let user = register_test_user(&mut conn, &mut rng).await;

    // generate some lists and insert them
    let lists = vec![gen_list(None), gen_list(None), gen_list(None)];

    for l in lists.iter() {
        insert_list(&mut conn, &user, l).await;
    }

    // construct delete requests
    let del_req = ListDeletedRequest {
        since: None,
        lists: vec![
            // unknown list
            Uuid::new_v4(),
            // known list
            lists[0].uuid.clone(),
            // Known but future date
            lists[1].uuid.clone(),
        ],
    };
    let t_now = chrono::Utc::now().naive_utc();
    let res = dao::update_deleted_lists(&mut conn, del_req.clone(), &user)
        .await
        .unwrap();
    // no previous data, and sanity check
    assert_eq!(0, res.delta.len());
    assert_eq!(0, res.unowned.len());
    // one unknown list delete entry
    assert_eq!(1, res.unknown.len());
    assert!(res.unknown.contains(&del_req.lists[0]));

    dbg!(Utc::now().naive_utc());
    sleep(std::time::Duration::from_secs(2)).await;
    let time1 = Utc::now().naive_utc();
    dbg!(time1);

    // sanity check revisiting later should give us a delta of 0
    let empty_data_d = ListDeletedRequest {
        since: Some(time1),
        lists: Vec::new(),
    };
    let res = dao::update_deleted_lists(&mut conn, empty_data_d, &user)
        .await
        .unwrap();
    dbg!(&res.delta);
    assert_eq!(0, res.delta.len());
    assert_eq!(0, res.unowned.len());
    assert_eq!(0, res.unknown.len());

    // retrieve all changes
    let empty_data = ListDeletedRequest {
        since: None,
        lists: Vec::new(),
    };
    let res = dao::update_deleted_lists(&mut conn, empty_data, &user)
        .await
        .unwrap();
    assert_eq!(2, res.delta.len());
    // valid entry
    let first = res
        .delta
        .get(&del_req.lists[1])
        .expect("expected valid list in delta not found");
    // valid entry
    let second = res
        .delta
        .get(&del_req.lists[2])
        .expect("expected valid list in delta not found");

    // again with 1 old, 1 new entry testing delta + deduplication of return
    let del_eq_2 = ListDeletedRequest {
        since: None,
        lists: vec![
            // old, existing entry but unsend to this client
            lists[1].uuid.clone(),
            // new valid entry
            lists[2].uuid.clone(),
        ],
    };
    let res = dao::update_deleted_lists(&mut conn, del_eq_2.clone(), &user)
        .await
        .unwrap();
    assert_eq!(1, res.delta.len());
    assert_eq!(0, res.unowned.len());
    assert_eq!(0, res.unknown.len());
    let item = res
        .delta
        .get(&lists[0].uuid)
        .expect("expected list from delta not found");

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
    let lists = vec![gen_list(None), gen_list(None), gen_list(None)];

    insert_list(&mut conn, &user_1, &lists[0]).await;
    insert_list(&mut conn, &user_2, &lists[1]).await;
    insert_list(&mut conn, &user_3, &lists[2]).await;

    // user2 gets access to user1 list, changeable
    insert_list_perm(&mut conn, &user_2, &lists[0].uuid, true, false).await;
    // user3 gets access to user1 list, non-changeable
    insert_list_perm(&mut conn, &user_3, &lists[0].uuid, false, false).await;

    // construct delete requests
    let del_req = ListDeletedRequest {
        since: None,
        lists: vec![
            // known list
            lists[0].uuid.clone(),
        ],
    };
    // now try to delete that from user2
    let res = dao::update_deleted_lists(&mut conn, del_req.clone(), &user_2)
        .await
        .unwrap();
    assert_eq!(0, res.delta.len());
    assert_eq!(0, res.unknown.len());
    // and this user shouldn't be allowed, no owner
    assert_eq!(1, res.unowned.len());
    assert!(res.unowned.contains(&del_req.lists[0]));

    let res = dao::update_deleted_lists(&mut conn, del_req.clone(), &user_3)
        .await
        .unwrap();
    assert_eq!(0, res.delta.len());
    assert_eq!(0, res.unknown.len());
    // same for user 3 with read-only
    assert_eq!(1, res.unowned.len());
    assert!(res.unowned.contains(&del_req.lists[0]));

    // Now delete it from user1, owner
    let res = dao::update_deleted_lists(&mut conn, del_req.clone(), &user_1)
        .await
        .unwrap();
    assert_eq!(0, res.delta.len());
    assert_eq!(0, res.unknown.len());
    assert_eq!(0, res.unowned.len());

    // which should be visible for user2
    let new_req = ListDeletedRequest {
        since: None,
        lists: Vec::new(),
    };
    let res = dao::update_deleted_lists(&mut conn, new_req.clone(), &user_2)
        .await
        .unwrap();
    assert_eq!(1, res.delta.len());
    assert_eq!(0, res.unknown.len());
    assert_eq!(0, res.unowned.len());
    assert_ne!(None, res.delta.get(&del_req.lists[0]));
    // and user 3
    let res = dao::update_deleted_lists(&mut conn, new_req, &user_3)
        .await
        .unwrap();
    assert_eq!(1, res.delta.len());
    assert_eq!(0, res.unknown.len());
    assert_eq!(0, res.unowned.len());
    assert_ne!(None, res.delta.get(&del_req.lists[0]));

    // now delete user1, the tombstones for user 2&3 should remain
    crate::users::dao::delete_user(&mut conn, &user_1)
        .await
        .unwrap();
    let new_req = ListDeletedRequest {
        since: None,
        lists: Vec::new(),
    };
    let res = dao::update_deleted_lists(&mut conn, new_req.clone(), &user_2)
        .await
        .unwrap();
    assert_ne!(None, res.delta.get(&del_req.lists[0]));
    // and user 3
    let res = dao::update_deleted_lists(&mut conn, new_req, &user_3)
        .await
        .unwrap();
    assert_ne!(None, res.delta.get(&del_req.lists[0]));

    db.drop_async().await;
}
