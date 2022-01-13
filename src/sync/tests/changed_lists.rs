use crate::prelude::*;
use crate::prelude::tests::*;
use super::*;

#[actix_rt::test]
async fn test_basic_changed_lists() {
    let db = DatabaseGuard::new().await;
    let mut conn = &mut *db.conn().await;
    let mut rng = rand::thread_rng();

    let user = UserId(register_test_user(&mut conn, &mut rng).await);
    let second_user = UserId(register_test_user(&mut conn, &mut rng).await);

    // insert two lists for the user
    let change_req = ListChangedRequest {
        client: Uuid::new_v4(),
        lists: vec![
            gen_list(None),gen_list(None)
        ]
    };
    let res = dao::update_changed_lists(&mut conn, change_req.clone(), &user).await.unwrap();
    assert_eq!(0,res.delta.len());
    assert_eq!(0,res.failures.len());
    let change_empty_d = ListChangedRequest {client: change_req.client.clone(),lists: vec![]};
    let res = dao::update_changed_lists(&mut conn, change_empty_d, &user).await.unwrap();
    assert_eq!(0,res.delta.len());
    assert_eq!(0,res.failures.len());

    let change_empty = ListChangedRequest {client: Uuid::new_v4(),lists: vec![]};
    let res = dao::update_changed_lists(&mut conn, change_empty.clone(), &user).await.unwrap();
    assert_eq!(2,res.delta.len());
    assert_eq!(0,res.failures.len());
    assert_list_eq(&change_req.lists[0], res.delta.get(&change_req.lists[0].uuid).expect("list not found"),ListPermissions::Owner);
    assert_list_eq(&change_req.lists[1], res.delta.get(&change_req.lists[1].uuid).expect("list not found"),ListPermissions::Owner);

    // give read to second user and try to receive it
    insert_list_perm(&mut conn,&second_user.0,&change_req.lists[0].uuid,false,true).await;
    let res = dao::update_changed_lists(&mut conn, change_empty.clone(), &second_user).await.unwrap();
    assert_eq!(1,res.delta.len());
    assert_eq!(0,res.failures.len());
    assert_list_eq(&change_req.lists[0], res.delta.get(&change_req.lists[0].uuid).expect("list not found"),ListPermissions::Read);
    // try to write it, should fail
    let mut change_unperm = ListChangedRequest {client: change_empty.client.clone(),lists: vec![change_req.lists[0].clone()]};
    change_unperm.lists[0].name = String::from("should never be visible");
    let res = dao::update_changed_lists(&mut conn, change_unperm.clone(), &second_user).await.unwrap();
    assert_eq!(0,res.delta.len());
    assert_eq!(1,res.failures.len());
    assert_eq!(change_req.lists[0].uuid,res.failures[0].id);
    // assert the list didn't change
    let change_empty = ListChangedRequest {client: Uuid::new_v4(),lists: vec![]};
    let res = dao::update_changed_lists(&mut conn, change_empty, &second_user).await.unwrap();
    assert_eq!(1,res.delta.len());
    assert_eq!(0,res.failures.len());
    assert_eq!(change_req.lists[0].name,res.delta[&change_req.lists[0].uuid].name);
    
    db.drop_async().await;
}

fn assert_list_eq(recv: &ListChangedEntryRecv, send: &ListChangedEntrySend, permission: ListPermissions) {
    assert_eq!(send.permissions,permission);
    assert_eq!(send.uuid,recv.uuid);
    assert_eq!(send.name,recv.name);
    assert_eq!(send.name_a,recv.name_a);
    assert_eq!(send.name_b,recv.name_b);
    assert_eq!(send.changed,recv.changed);
    assert_eq!(send.created,recv.created);
}