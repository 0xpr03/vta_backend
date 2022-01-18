use std::thread;

use base64ct::Encoding;
use chrono::Duration;
use rand_core::RngCore;
use sqlx::{Connection, Executor};

use super::models::*;
use super::*;
use crate::prelude::tests::*;
use crate::prelude::*;

#[actix_rt::test]
async fn test_create_sharecode_multiuse() {
    let db = DatabaseGuard::new().await;
    let mut conn = &mut *db.conn().await;
    let mut rng = rand::thread_rng();

    let user = UserId(register_test_user(&mut conn, &mut rng).await);
    let user2 = UserId(register_test_user(&mut conn, &mut rng).await);
    let list1 = gen_list(&mut rng);
    let list_id = dao::create_list(&mut conn, &user, list1.clone()).await.unwrap();

    let share_data = NewTokenData { write: true,
        reshare: false,
        reusable: true,
        deadline: random_future_date(&mut rng),
    };

    let res = dao::generate_share_code(&mut conn, &user, &list_id,share_data).await.unwrap();
    
    let id = dao::use_share_code(&mut conn, &user2, &res.token_a, &res.token_b).await.unwrap();
    // assert_eq!(list_id.0,id.0);
    let id = dao::use_share_code(&mut conn, &user2, &res.token_a, &res.token_b).await.unwrap();
    
    db.drop_async().await;
}

#[actix_rt::test]
async fn test_create_sharecode_single() {
    let db = DatabaseGuard::new().await;
    let mut conn = &mut *db.conn().await;
    let mut rng = rand::thread_rng();

    let user = UserId(register_test_user(&mut conn, &mut rng).await);
    let user2 = UserId(register_test_user(&mut conn, &mut rng).await);
    let list1 = gen_list(&mut rng);
    let list_id = dao::create_list(&mut conn, &user, list1.clone()).await.unwrap();

    let share_data = NewTokenData { write: true,
        reshare: false,
        reusable: false,
        deadline: random_future_date(&mut rng),
    };

    let res = dao::generate_share_code(&mut conn, &user, &list_id,share_data).await.unwrap();
    
    let id = dao::use_share_code(&mut conn, &user2, &res.token_a, &res.token_b).await.unwrap();
    assert_eq!(list_id.0,id.0);
    match dao::use_share_code(&mut conn, &user2, &res.token_a, &res.token_b).await {
        Err(ListError::SharecodeInvalid) => (),
        v => panic!("expected SharecodeInvalid, got {:?}",v),
    }
    db.drop_async().await;
}

#[actix_rt::test]
async fn test_sharecode_invalid_data() {
    let db = DatabaseGuard::new().await;
    let mut conn = &mut *db.conn().await;
    let mut rng = rand::thread_rng();

    let user = UserId(register_test_user(&mut conn, &mut rng).await);
    let user2 = UserId(register_test_user(&mut conn, &mut rng).await);
    let list1 = gen_list(&mut rng);
    let list_id = dao::create_list(&mut conn, &user, list1.clone()).await.unwrap();

    let share_data = NewTokenData { write: true,
        reshare: false,
        reusable: true,
        deadline: random_future_date(&mut rng),
    };

    let res = dao::generate_share_code(&mut conn, &user, &list_id,share_data).await.unwrap();

    for i in 0..1_000_0 {
        let mut token_b = [0u8; 16];
        rng.fill_bytes(&mut token_b);
        let _ = dao::use_share_code(&mut conn, &user2, &res.token_a, &base64::encode(token_b.as_slice())).await.unwrap_err();
    }

    db.drop_async().await;
}