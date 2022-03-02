use chrono::Duration;

use super::models::*;
use super::*;
use crate::prelude::tests::*;
use crate::prelude::*;

#[actix_rt::test]
async fn test_basic_deleted_lists() {
    let db = DatabaseGuard::new().await;
    let mut conn = &mut *db.conn().await;
    let mut rng = rand::thread_rng();

    let user = register_test_user(&mut conn, &mut rng).await;
    let second_user = register_test_user(&mut conn, &mut rng).await;

    // prepare lists
    let list1 = gen_list(None);
    let list2 = gen_list(None);
    let list3 = gen_list(None);
    insert_list(&mut conn, &user, &list1).await;
    insert_list(&mut conn, &second_user, &list2).await;
    insert_list(&mut conn, &second_user, &list3).await;
    // generate entries for both
    let entries1 = vec![
        gen_entry(&list1.uuid, None),
        gen_entry(&list1.uuid, None),
        gen_entry(&list1.uuid, None),
    ];
    insert_entries(&mut conn, &entries1).await;
    let entries2 = vec![
        gen_entry(&list2.uuid, None),
        gen_entry(&list2.uuid, None),
        gen_entry(&list2.uuid, None),
    ];
    insert_entries(&mut conn, &entries2).await;
    let entries3 = vec![
        gen_entry(&list3.uuid, None),
        gen_entry(&list3.uuid, None),
        gen_entry(&list3.uuid, None),
    ];
    insert_entries(&mut conn, &entries3).await;
    // allow read on last list
    insert_list_perm(&mut conn, &user, &list3.uuid, false, true).await;
    // delete request
    let v = EntryDeletedRequest {
        since: None,
        entries: vec![
            EntryDeleteEntry {
                list: list1.uuid.clone(),
                entry: entries1[0].uuid,
            }, // ok
            EntryDeleteEntry {
                list: list2.uuid.clone(),
                entry: entries2[0].uuid.clone(),
            }, // no perms
            EntryDeleteEntry {
                list: list3.uuid.clone(),
                entry: entries3[0].uuid.clone(),
            }, // read only
            EntryDeleteEntry {
                list: list1.uuid.clone(),
                entry: Uuid::new_v4(),
            }, // unknown
        ],
    };
    let res = dao::update_deleted_entries(&mut conn, v.clone(), &user)
        .await
        .unwrap();
    assert_eq!(0, res.delta.len());
    assert_eq!(1, res.ignored.len());
    assert_eq!(2, res.invalid.len());
    // check that only 1 got deleted, the valid one
    let empty_req = EntryDeletedRequest {
        since: None,
        entries: vec![],
    };
    let res = dao::update_deleted_entries(&mut conn, empty_req, &user)
        .await
        .unwrap();
    assert_eq!(1, res.delta.len());
    let entry = res.delta.get(&entries1[0].uuid).unwrap();
    assert_eq!(entry.list, entries1[0].list);
    check_del_equal(entry, &v.entries[0]);
    // change list3 perm to write shared
    insert_list_perm(&mut conn, &user, &list3.uuid, true, true).await;
    let del_shared = EntryDeletedRequest {
        since: None,
        entries: vec![EntryDeleteEntry {
            list: list3.uuid.clone(),
            entry: entries3[0].uuid.clone(),
        }],
    };
    let res = dao::update_deleted_entries(&mut conn, del_shared.clone(), &user)
        .await
        .unwrap();
    assert_eq!(1, res.delta.len());
    assert_eq!(0, res.invalid.len());
    assert_eq!(0, res.ignored.len());
    // now check for deltas + write-shared allowed
    // TODO: delta check
    //std::thread::sleep(std::time::Duration::from_secs(1));
    let empty_req = EntryDeletedRequest {
        since: None,
        entries: vec![],
    };
    let res = dao::update_deleted_entries(&mut conn, empty_req, &user)
        .await
        .unwrap();
    assert_eq!(2, res.delta.len());
    assert_eq!(0, res.invalid.len());
    assert_eq!(0, res.ignored.len());
    let shared = res.delta.get(&del_shared.entries[0].entry).unwrap();
    check_del_equal(shared, &del_shared.entries[0]);

    db.drop_async().await;
}

fn check_del_equal(a: &EntryDeleteEntry, b: &EntryDeleteEntry) {
    assert_eq!(a.entry, b.entry);
    assert_eq!(a.list, b.list);
}
