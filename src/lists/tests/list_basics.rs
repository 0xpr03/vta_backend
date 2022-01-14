use chrono::Duration;

use super::models::*;
use super::*;
use crate::prelude::tests::*;
use crate::prelude::*;

#[actix_rt::test]
async fn test_list_create() {
    let db = DatabaseGuard::new().await;
    let mut conn = &mut *db.conn().await;
    let mut rng = rand::thread_rng();

    let user = UserId(register_test_user(&mut conn, &mut rng).await);

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

    let second_user = UserId(register_test_user(&mut conn, &mut rng).await);

    // prepare lists for foreign list access
    let list1_content = gen_list(&mut rng);
    let list2_content = gen_list(&mut rng);
    let list3_content = gen_list(&mut rng);
    let list1 = dao::create_list(&mut conn, &second_user, list1_content.clone()).await.unwrap();
    let list2 = dao::create_list(&mut conn, &second_user, list2_content.clone()).await.unwrap();
    // shouldn't be visible for user
    let _list3 = dao::create_list(&mut conn, &second_user, list3_content.clone()).await.unwrap();

    insert_list_perm(&mut conn, &user.0, &list1.0, false,false).await;
    insert_list_perm(&mut conn, &user.0, &list2.0, true,false).await;

    // now we expect to see list1 + 2n and our original list

    let lists = dao::all_lists(&mut conn, &user).await.unwrap();
    assert_eq!(lists.len(), 3);

    test_list_change_equal(&lists.get(&list_id.0).unwrap(),&change);
    let expected_l1 = lists.get(&list1.0).unwrap();
    test_list_change_equal(&expected_l1,&list1_content);
    assert_eq!(expected_l1.foreign,true);
    assert_eq!(expected_l1.change,false);
    let expected_l2 = lists.get(&list2.0).unwrap();
    test_list_change_equal(&expected_l2,&list2_content);
    assert_eq!(expected_l2.foreign,true);
    assert_eq!(expected_l2.change,true);

    db.drop_async().await;
}

fn test_list_change_equal(list: &List, change: &ListChange) {
    assert_eq!(list.name,change.name);
    assert_eq!(list.name_b,change.name_b);
    assert_eq!(list.name_a,change.name_a);
}
