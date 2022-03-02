use super::dao;
use super::AuthError;
use crate::prelude::tests::*;
use crate::prelude::*;
use chrono::Duration;
use chrono::Utc;

/// Verify key register, retrieval and user info
#[actix_rt::test]
async fn test_register() {
    let db = DatabaseGuard::new().await;
    let mut conn = &mut *db.conn().await;

    let mut rng = rand::thread_rng();
    let (claims, key, key_type) = gen_user(&mut rng);
    let t_now = Utc::now().naive_utc();
    dao::register_user(&mut conn, &claims, &key, key_type.clone())
        .await
        .unwrap();

    // now try to re-register
    let (mut claims_new, new_key, _) = gen_user(&mut rng);
    claims_new.iss = claims.iss.clone();
    let res = dao::register_user(&mut conn, &claims, &new_key, key_type.clone()).await;
    match res {
        Err(AuthError::ExistingUser) => (),
        e => panic!("expected ExistingUser, got {:?}", e),
    }
    // and verify the user created is actually from the first call
    let user = UserId(claims.iss);
    let res = dao::user_key(&mut conn, &user).await.unwrap();
    let res = res.expect("no key found");
    assert_eq!(key, res.auth_key);
    assert_eq!(key_type, res.key_type);

    let res_user = dao::user_by_uuid(&mut conn, &user).await.unwrap();
    let res_user = res_user.expect("no user found");
    assert_eq!(claims.iss, res_user.uuid);
    assert_eq!(claims.name, res_user.name);
    assert_eq!(None, res_user.locked);
    assert!(t_now - res_user.last_seen < Duration::seconds(5));
    assert_eq!(claims.delete_after, res_user.delete_after);

    db.drop_async().await;
}

#[actix_rt::test]
async fn test_password_login() {
    let db = DatabaseGuard::new().await;
    let mut conn = &mut *db.conn().await;

    let mut rng = rand::thread_rng();
    let (claims, key, key_type) = gen_user(&mut rng);
    let user_id = UserId(claims.iss);
    dao::register_user(&mut conn, &claims, &key, key_type.clone())
        .await
        .unwrap();

    let (email, password) = gen_mail_pw();
    let pw_hash = super::routes::hash_pw(password.clone()).unwrap();
    dao::create_password_login(&mut conn, &user_id, &email, &pw_hash)
        .await
        .unwrap();

    // try to do it again
    let (email2, password2) = gen_mail_pw();
    // we just pass the password itself, doesn't matter here
    let res = dao::create_password_login(&mut conn, &user_id, &email2, &password2).await;
    match res {
        Err(AuthError::ExistingLogin) => (),
        e => panic!("expected ExistingLogin, got {:?}", e),
    }

    // and verify only the first call succeded
    let res = dao::user_by_email(&mut conn, &email).await.unwrap();
    let res = res.expect("no login for email");
    assert_eq!(claims.iss, res.user_id);
    assert_eq!(email, res.email);
    assert_eq!(pw_hash, res.password);
    assert_eq!(false, res.verified);

    super::routes::verify_pw(password, res.password).unwrap();

    db.drop_async().await;
}

#[actix_rt::test]
async fn test_user_delete() {
    // test user deletion
    // - user existing, no crash
    // - user not locked
    // - tombstone is created
    // testing for deleted_list_shared is done in sync::tests

    let db = DatabaseGuard::new().await;
    let mut conn = &mut *db.conn().await;

    let (claims, key, key_type) = gen_user(&mut rand::thread_rng());
    let user = UserId(claims.iss);
    dao::register_user(&mut conn, &claims, &key, key_type.clone())
        .await
        .unwrap();

    let (email, password) = gen_mail_pw();
    let pw_hash = super::routes::hash_pw(password.clone()).unwrap();
    dao::create_password_login(&mut conn, &user, &email, &pw_hash)
        .await
        .unwrap();

    dao::delete_user(&mut conn, &user).await.unwrap();

    let res = dao::user_by_email(&mut conn, &email).await.unwrap();
    assert!(res.is_none());

    let res = dao::user_by_uuid(&mut conn, &user).await.unwrap();
    assert!(res.is_none());

    // tombstone exists
    let res = dao::user_deleted(&mut conn, &user).await.unwrap();
    assert_eq!(true, res);

    db.drop_async().await;
}

#[actix_rt::test]
async fn test_last_seen() {
    let db = DatabaseGuard::new().await;
    let mut conn = &mut *db.conn().await;

    let (claims, key, key_type) = gen_user(&mut rand::thread_rng());
    let user = UserId(claims.iss);
    dao::register_user(&mut conn, &claims, &key, key_type.clone())
        .await
        .unwrap();

    let time = Utc::now().naive_utc();
    dao::update_last_seen(&mut conn, &user, time).await.unwrap();

    let res: Timestamp =
        sqlx::query_scalar::<_, Timestamp>("SELECT last_seen FROM users WHERE uuid = ?")
            .bind(user.0)
            .fetch_one(&mut *conn)
            .await
            .unwrap();
    assert_eq!(res.timestamp(), time.timestamp());

    db.drop_async().await;
}

fn gen_mail_pw() -> (String, String) {
    let mut rng = rand::thread_rng();
    let email: String = random_string(&mut rng, 7);
    let password = random_string(&mut rng, 40);

    (email, password)
}
