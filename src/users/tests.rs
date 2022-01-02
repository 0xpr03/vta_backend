use super::dao;
use super::user::*;
use chrono::Duration;
use chrono::Utc;
use rand::distributions::Standard;
use rand::{distributions::Alphanumeric, Rng};

use crate::prelude::*;
use crate::prelude::tests::*;
use super::AuthError;


/// Verify key register, retrieval and user info
#[actix_rt::test]
async fn test_register() {
    let db = DatabaseGuard::new().await;
    let mut conn = &mut *db.conn().await;

    let(claims,key,key_type) = gen_user();
    let t_now = Utc::now().naive_utc();
    dao::register_user(&mut conn,&claims,&key,key_type.clone()).await.unwrap();

    // now try to re-register
    let (mut claims_new,new_key,_) = gen_user();
    claims_new.iss = claims.iss.clone();
    let res = dao::register_user(&mut conn,&claims,&new_key,key_type.clone()).await;
    match res {
        Err(AuthError::ExistingUser) => (),
        e => panic!("expected ExistingUser, got {:?}",e),
    }
    // and verify the user created is actually from the first call

    let res = dao::user_key(&mut conn,&claims.iss).await.unwrap();
    let res = res.expect("no key found");
    assert_eq!(key,res.auth_key);
    assert_eq!(key_type,res.key_type);
    

    let res_user = dao::user_by_uuid(&mut conn, &claims.iss).await.unwrap();
    let res_user = res_user.expect("no user found");
    assert_eq!(claims.iss,res_user.uuid);
    assert_eq!(claims.name,res_user.name);
    assert_eq!(None,res_user.locked);
    assert!(t_now - res_user.last_seen < Duration::seconds(5));
    assert_eq!(claims.delete_after,res_user.delete_after);

    db.drop_async().await;
}

#[actix_rt::test]
async fn test_password_login() {
    let db = DatabaseGuard::new().await;
    let mut conn = &mut *db.conn().await;

    let(claims,key,key_type) = gen_user();
    dao::register_user(&mut conn,&claims,&key,key_type.clone()).await.unwrap();

    let(email,password) = gen_mail_pw();
    let pw_hash = super::routes::hash_pw(password.clone()).unwrap();
    dao::create_password_login(&mut conn, &claims.iss, &email, &pw_hash).await.unwrap();

    // try to do it again
    let res = dao::create_password_login(&mut conn, &claims.iss, &email, &pw_hash).await;
    match res {
        Err(AuthError::ExistingLogin) => (),
        e => panic!("expected ExistingLogin, got {:?}",e),
    }

    // and verify only the first call succeded
    let res = dao::user_by_email(&mut conn, &email).await.unwrap();
    let res = res.expect("no login for email");
    assert_eq!(claims.iss,res.user_id);
    assert_eq!(email,res.email);
    assert_eq!(pw_hash,res.password);
    assert_eq!(false,res.verified);

    super::routes::verify_pw(password,res.password).unwrap();

    db.drop_async().await;
}

fn gen_user() -> (RegisterClaims,Vec<u8>,KeyType) {
    let mut rng = rand::thread_rng();
    let claims = RegisterClaims {
        iss: Uuid::new_v4(),
        name: (&mut rng).sample_iter(Alphanumeric)
        .take(7)
        .map(char::from)
        .collect(),
        delete_after: Some(3600),
    };
    let key: Vec<u8> = rng.sample_iter(Standard).take(16).collect();
    let key_type = KeyType::EC_PEM;
    (claims,key,key_type)
}

fn gen_mail_pw() -> (String,String) {
    let mut rng = rand::thread_rng();
    let email: String = (&mut rng).sample_iter(Alphanumeric)
        .take(7)
        .map(char::from)
        .collect();
    let password = (&mut rng).sample_iter(Alphanumeric)
    .take(40)
    .map(char::from)
    .collect();
    
    (email,password)
}