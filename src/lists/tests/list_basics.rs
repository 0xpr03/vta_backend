use super::*;

#[actix_rt::test]
async fn test_list_create_perm() {
    let db = DatabaseGuard::new().await;
    let mut conn = &mut *db.conn().await;
    let mut rng = rand::thread_rng();

    let user = register_test_user(&mut conn, &mut rng).await;

    assert_eq!(dao::all_lists(&mut conn, &user).await.unwrap().len(), 0);

    let change = ListChange {
        name: random_string(&mut rng, 7),
        name_a: random_string(&mut rng, 7),
        name_b: random_string(&mut rng, 7),
    };
    let list_id = dao::create_list(&mut conn, &user, change.clone()).await.unwrap();

    let lists = dao::all_lists(&mut conn, &user).await.unwrap();
    assert_eq!(lists.len(), 1);
    let expected_list = lists.get(&list_id.0).unwrap();
    test_list_change_equal(expected_list,&change);
    assert_eq!(expected_list.foreign,false);
    assert_eq!(expected_list.change,false);

    let second_user = register_test_user(&mut conn, &mut rng).await;

    // prepare lists for foreign list access
    let list1_content = gen_list(&mut rng);
    let list2_content = gen_list(&mut rng);
    let list3_content = gen_list(&mut rng);
    let list1 = dao::create_list(&mut conn, &second_user, list1_content.clone()).await.unwrap();
    let list2 = dao::create_list(&mut conn, &second_user, list2_content.clone()).await.unwrap();
    // shouldn't be visible for user
    let list3 = dao::create_list(&mut conn, &second_user, list3_content.clone()).await.unwrap();

    insert_list_perm(&mut conn, &user.0, &list1.0, false,false).await;
    insert_list_perm(&mut conn, &user.0, &list2.0, true,false).await;

    // check permissions viewable
    {
    let res = dao::list_sharing(&mut conn, &second_user, &list1).await.unwrap();
    assert_eq!(res.len(),1);
    let entry = res.get(&user.0).unwrap();
    assert_eq!(false,entry.write);
    assert_eq!(false,entry.reshare);
    }
    {
    let res = dao::list_sharing(&mut conn, &second_user, &list2).await.unwrap();
    assert_eq!(res.len(),1);
    let entry = res.get(&user.0).unwrap();
    assert_eq!(true,entry.write);
    assert_eq!(false,entry.reshare);
    }
    // now we expect to see list1 + 2n and our original list

    let lists = dao::all_lists(&mut conn, &user).await.unwrap();
    assert_eq!(lists.len(), 3);

    test_list_change_equal(&lists.get(&list_id.0).unwrap(),&change);
    {
    let expected_l1 = lists.get(&list1.0).unwrap();
    test_list_change_equal(&expected_l1,&list1_content);
    assert_eq!(expected_l1.foreign,true);
    assert_eq!(expected_l1.change,false);
    }
    {
    let expected_l2 = lists.get(&list2.0).unwrap();
    test_list_change_equal(&expected_l2,&list2_content);
    assert_eq!(expected_l2.foreign,true);
    assert_eq!(expected_l2.change,true);
    }

    // check single_list for correct data
    {
    let res = dao::single_list(&mut conn, &user,&list1).await.unwrap();
    test_list_change_equal(&res,&list1_content);
    assert_eq!(res.foreign,true);
    assert_eq!(res.change,false);
    }
    {
    let res = dao::single_list(&mut conn, &user,&list2).await.unwrap();
    test_list_change_equal(&res,&list2_content);
    assert_eq!(res.foreign,true);
    assert_eq!(res.change,true);
    }
    {
    let res = dao::single_list(&mut conn, &user,&list3).await;
    match res {
        Err(ListError::ListPermission) => (),
        v => panic!("invalid result: {:?}",v),
    }
    }
    
    db.drop_async().await;
}

#[actix_rt::test]
async fn test_list_change() {
    let db = DatabaseGuard::new().await;
    let mut conn = &mut *db.conn().await;
    let mut rng = rand::thread_rng();

    let user = register_test_user(&mut conn, &mut rng).await;
    // create
    let l1 = gen_list_create(&mut rng);
    let l1_id = dao::create_list(&mut conn, &user, l1.clone()).await.unwrap();

    let lists = dao::all_lists(&mut conn, &user).await.unwrap();
    test_list_change_equal(lists.get(&l1_id.0).unwrap(),&l1);
    // change
    let change = gen_list_create(&mut rng);
    dao::change_list(&mut conn, &user, l1_id.clone(), change.clone()).await.unwrap();
    // TODO: check updated-date when implemented
    let lists = dao::all_lists(&mut conn, &user).await.unwrap();
    test_list_change_equal(lists.get(&l1_id.0).unwrap(),&change);
    // delete
    dao::delete_list(&mut conn, &user, l1_id.clone()).await.unwrap();

    let lists = dao::all_lists(&mut conn, &user).await.unwrap();
    assert_eq!(0,lists.len());

    //check for deleted_list entry
    let lists = get_deleted_lists(&mut conn, &user).await;
    assert_eq!(lists,vec![l1_id.0]);

    db.drop_async().await;
}

// TODO: verify shared list changes and entry changes

#[actix_rt::test]
async fn test_entry_change() {
    let db = DatabaseGuard::new().await;
    let mut conn = &mut *db.conn().await;
    let mut rng = rand::thread_rng();

    let user = register_test_user(&mut conn, &mut rng).await;
    //# create
    let t_now = Utc::now().naive_utc();
    let l1 = gen_list_create(&mut rng);
    let l1_id = dao::create_list(&mut conn, &user, l1.clone()).await.unwrap();

    let e1 = gen_entry(&mut rng);
    let e1_id = dao::create_entry(&mut conn, user.clone(), l1_id.clone(), e1.clone()).await.unwrap();

    let ret = dao::entries(&mut conn, &user, l1_id.clone()).await.unwrap();
    assert_eq!(ret.len(),1);
    test_entrychange_equal(ret.get(&e1_id.0).unwrap(),&e1,"created entry");
    // check updated date
    let updated = entry_updated_date(&mut conn, &e1_id).await;
    assert!((updated - t_now).num_seconds() < 2);

    //# change
    let change = gen_entry(&mut rng);
    dao::change_entry(&mut conn, &user, e1_id.clone(), change.clone()).await.unwrap();

    let ret = dao::entries(&mut conn, &user, l1_id.clone()).await.unwrap();
    assert_eq!(ret.len(),1);
    test_entrychange_equal(ret.get(&e1_id.0).unwrap(),&change,"changed entry");

    // check updated date
    let updated = entry_updated_date(&mut conn, &e1_id).await;
    assert!((updated - t_now).num_seconds() < 2);

    //# delete
    dao::delete_entry(&mut conn, &user, e1_id.clone()).await.unwrap();

    let ret = dao::entries(&mut conn, &user, l1_id.clone()).await.unwrap();
    assert_eq!(0,ret.len());
    // test for deletion entry
    let deleted = get_deleted_entries(&mut conn,&l1_id).await;
    assert_eq!(deleted,vec![e1_id.0]);

    // sanity check
    let lists = dao::all_lists(&mut conn, &user).await.unwrap();
    assert_eq!(1,lists.len());

    db.drop_async().await;
}

fn test_list_change_equal(list: &List, change: &ListChange) {
    assert_eq!(list.name,change.name);
    assert_eq!(list.name_b,change.name_b);
    assert_eq!(list.name_a,change.name_a);
}

fn test_entry_equal(entry: &Entry, expected: &Entry) {
    assert_eq!(entry.uuid,expected.uuid);
    assert_eq!(entry.tip,expected.tip);
    assert_eq!(entry.meanings,expected.meanings);
}

fn test_entrychange_equal(entry: &Entry, expected: &EntryChange, msg: &'static str) {
    assert_eq!(entry.tip,expected.tip,"{}",msg);
    assert_eq!(entry.meanings,expected.meanings,"{}",msg);
}