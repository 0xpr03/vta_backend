use std::thread;

use chrono::Duration;

use crate::prelude::*;
use crate::prelude::tests::*;
use super::*;


#[test_log::test(actix_rt::test)]
async fn test_basic_changed_entries() {
    let db = DatabaseGuard::new().await;
    let mut conn = &mut *db.conn().await;
    let mut rng = rand::thread_rng();

    let user = UserId(register_test_user(&mut conn, &mut rng).await);

    // prepare lists
    let list1 = gen_list(None);
    let list2 = gen_list(None);
    insert_list(&mut conn, &user.0, &list1).await;
    insert_list(&mut conn, &user.0, &list2).await;
    
    // insert some entries
    let client1 = Uuid::new_v4();

    let entries1: Vec<_> = (0..10).into_iter().map(|_|gen_entry(&list1.uuid,None)).collect();
    let entries2: Vec<_> = (0..5).into_iter().map(|_|gen_entry(&list2.uuid,None)).collect();

    let entries: Vec<EntryChangedEntry> = entries1.iter().chain(entries2.iter()).map(|v|v.clone()).collect();
    let data = EntryChangedRequest { client: client1.clone(), entries };

    let resp = dao::update_changed_entries(&mut conn, data, &user).await.unwrap();
    assert_eq!(resp.ignored.len(),0);
    assert_eq!(resp.invalid.len(),0);
    assert_eq!(resp.delta.len(),0);
    // read them back
    let client2 = Uuid::new_v4();
    let resp = dao::update_changed_entries(&mut conn, EntryChangedRequest { client: client2.clone(), entries: Vec::new() }, &user).await.unwrap();
    assert_eq!(resp.ignored.len(),0);
    assert_eq!(resp.invalid.len(),0);
    assert_eq!(resp.delta.len(),entries1.len()+entries2.len());
    dbg!(entries1.len()+entries2.len());

    for e_exp in entries1.iter() {
        // verify we've insertes entries1
        let e_res = resp.delta.get(&e_exp.uuid).expect("inserted entry not found");
        assert_entry_eq(e_res,e_exp);
    }
    for e_exp in entries2.iter() {
        // verify we've insertes entries2
        let e_res = resp.delta.get(&e_exp.uuid).expect("inserted entry not found");
        assert_entry_eq(e_res,e_exp);
    }

    // TODO: assert we're reading 0 back after 1 second delay

    // delta test: change one, insert one new
    let new = gen_entry(&list1.uuid,None);
    let changed_date = entries2[0].changed + Duration::seconds(1);
    let mut changed = gen_entry(&list2.uuid,Some(changed_date.clone()));
    changed.uuid = entries2[0].uuid.clone();
    
    // also delete one for deleted-test
    let deleted_date = entries1[0].changed + Duration::seconds(2);
    let mut deleted = gen_entry(&list1.uuid,Some(deleted_date.clone()));
    deleted.uuid = entries1[0].uuid.clone();
    let del_req = EntryDeletedRequest {
        client: Uuid::new_v4(),
        entries: vec![
            EntryDeleteEntry {
                list: list1.uuid.clone(),
                entry: deleted.uuid.clone(),
                time: entries1[0].changed + Duration::seconds(1)
            }]
    };
    let resp = dao::update_deleted_entries(&mut conn, del_req,&user).await.unwrap();
    assert_eq!(resp.ignored.len(),0);
    assert_eq!(resp.invalid.len(),0);
    assert_eq!(resp.delta.len(),0);

    // wait for 1 second to wait for the changes to settle
    thread::sleep(std::time::Duration::from_secs(2));

    // now perform the change request
    let entries_changed = vec![&new,&changed,&deleted];
    println!("{:#?}",entries_changed);
    let resp = dao::update_changed_entries(&mut conn, 
        EntryChangedRequest { client: client2.clone(),
            entries: entries_changed.iter().map(|v|(*v).clone()).collect()
        }, &user).await.unwrap();
    assert_eq!(resp.ignored,vec![deleted.uuid.clone()]);
    assert_eq!(resp.invalid.len(),0);
    // we will get out previous delta here
    // trading redundant data for support of parallel updates (same second)
    // entries1+entries2 - 1 deleted - 1 changed in the same call, thus not send back
    assert_eq!(resp.delta.len(),entries1.len()+entries2.len()-2);

    // and read those changes back with client1
    // this only works correctly if we use the updated-date and not the transmitted "changed" date
    let resp = dao::update_changed_entries(&mut conn, EntryChangedRequest { client: client1.clone(), entries: Vec::new() }, &user).await.unwrap();
    assert_eq!(resp.ignored.len(),0);
    assert_eq!(resp.invalid.len(),0);
    assert_eq!(resp.delta.len(),entries1.len()+entries2.len()); // -1 deleted + 1 new
    // ensure the new entry is there and the changed on is correct
    let c_ret = resp.delta.get(&changed.uuid).unwrap();
    assert_entry_eq(&c_ret,&changed);
    assert_entry_eq(&resp.delta.get(&new.uuid).unwrap(),&new);
    // now ensure all other entries are also there
    for e_exp in entries1.iter() {
        if e_exp.uuid == deleted.uuid {
            continue;
        }
        // verify we've insertes entries1
        let e_res = resp.delta.get(&e_exp.uuid).expect("inserted entry not found");
        assert_entry_eq(e_res,e_exp);
    }
    for e_exp in entries2.iter() {
        if e_exp.uuid == changed.uuid {
            continue;
        }
        // verify we've insertes entries2
        let e_res = resp.delta.get(&e_exp.uuid).expect("inserted entry not found");
        assert_entry_eq(e_res,e_exp);
    }

    db.drop_async().await;
}

#[test_log::test(actix_rt::test)]
async fn test_basic_changed_entries_shared() {
    let db = DatabaseGuard::new().await;
    let mut conn = &mut *db.conn().await;
    let mut rng = rand::thread_rng();

    let user = UserId(register_test_user(&mut conn, &mut rng).await);
    let second_user = UserId(register_test_user(&mut conn, &mut rng).await);

    // prepare lists
    let list1 = gen_list(None);
    let list2 = gen_list(None);
    let list3 = gen_list(None);
    let list4 = gen_list(None);
    insert_list(&mut conn, &user.0, &list1).await;
    insert_list(&mut conn, &second_user.0, &list2).await;
    insert_list(&mut conn, &second_user.0, &list3).await;
    insert_list(&mut conn, &second_user.0, &list4).await;

    // give specific access
    insert_list_perm(&mut conn,&user.0,&list2.uuid,false,true).await;
    insert_list_perm(&mut conn,&user.0,&list3.uuid,true,true).await;
    
    // insert some entries
    let client1 = Uuid::new_v4();

    let e_l1 = gen_entry(&list1.uuid,None);
    let e_l2 = gen_entry(&list2.uuid,None);
    let e_l3 = gen_entry(&list3.uuid,None);
    let e_l4 = gen_entry(&list4.uuid,None);

    let entries = vec![&e_l1,&e_l2,&e_l3,&e_l4];

    let data = EntryChangedRequest { client: client1.clone(), entries: entries.iter().map(|v|(*v).clone()).collect() };

    let resp = dao::update_changed_entries(&mut conn, data, &user).await.unwrap();
    assert_eq!(resp.ignored.len(),0);
    assert_eq!(resp.invalid.len(),2);
    assert_eq!(resp.delta.len(),0);
    // read them back
    let resp = dao::update_changed_entries(&mut conn, EntryChangedRequest { client: client1.clone(), entries: Vec::new() }, &user).await.unwrap();
    assert_eq!(resp.ignored.len(),0);
    assert_eq!(resp.delta.len(),2);
    // assert that only these two were written
    assert_entry_eq(resp.delta.get(&e_l1.uuid).unwrap(),&e_l1);
    assert_entry_eq(resp.delta.get(&e_l3.uuid).unwrap(),&e_l3);

    db.drop_async().await;
}

fn assert_entry_eq(recv: &EntryChangedEntry, send: &EntryChangedEntry) {
    assert_eq!(send.uuid,recv.uuid);
    assert_eq!(send.changed,recv.changed);
    assert_eq!(send.list,recv.list);
    assert_eq!(send.tip,recv.tip);
    assert_eq!(send.meanings,recv.meanings);
}