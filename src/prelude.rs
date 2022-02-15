use std::{fmt, ops::Deref};

use chrono::NaiveDateTime;

pub type Timestamp = NaiveDateTime;
pub type DbConn = sqlx::MySqlConnection;
pub use tracing::*;
pub use uuid::Uuid;
pub use color_eyre::eyre::Context;
pub use serde::{Deserialize, Serialize};
pub use crate::state::AppState;

/// Check query result for duplicate-entry error. Returns true if found.
pub fn check_duplicate(res: std::result::Result<sqlx::mysql::MySqlQueryResult, sqlx::Error>) -> std::result::Result<bool,sqlx::Error> {
    if let Err(e) = res {
        if let sqlx::Error::Database(ref e) = e {
            if e.code() == Some(std::borrow::Cow::Borrowed("23000")) {
                return Ok(true);
            }
        }
        return Err(e.into());
    } else {
        return Ok(false)
    }
}

#[derive(Debug, Serialize)]
pub struct ErrorResponse {
    pub code: i32,
    pub message: &'static str,
}

#[cfg_attr(test, derive(Clone, PartialEq))]
pub struct ListId(pub Uuid);
#[cfg_attr(test, derive(Clone, PartialEq))]
pub struct EntryId(pub Uuid);
#[cfg_attr(test, derive(Clone, PartialEq))]
pub struct UserId(pub Uuid);

impl fmt::Display for ListId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}
impl fmt::Display for EntryId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}
impl fmt::Display for UserId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}
impl fmt::Debug for ListId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "ListId({})", self.0)
    }
}
impl fmt::Debug for EntryId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "EntryId({})", self.0)
    }
}
impl fmt::Debug for UserId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "UserId({})", self.0)
    }
}

#[cfg(test)]
pub mod tests {
    use actix_rt::spawn;
    use chrono::{NaiveDateTime, Utc};
    use rand::{Rng, distributions::{Alphanumeric, Standard}, prelude::ThreadRng};
    use sqlx::{mysql::{MySqlConnectOptions, MySqlPoolOptions}, pool::PoolConnection, MySql, Executor};
    use uuid::Uuid;

    use crate::{Pool, users::user::{RegisterClaims, KeyType}};

    use super::*;

    pub struct DatabaseGuard {
        pub db: Pool,
        pub db_name: String,
    }
    
    impl DatabaseGuard {
        pub async fn new() -> Self {
            // ignore on purpose, can only be installed once
            // let _ = color_eyre::install();
            let mut rng = rand::thread_rng();
            let db_name: String = format!("testing_temp_{}",random_string(&mut rng,7));
            println!("Temp DB: {}",db_name);
            
            let options = MySqlConnectOptions::new()
            .host("127.0.0.1")
            .port(3306)
            .username("root");

            let conn_uri = std::env::var("DATABASE_URL").ok();
    
            {
                let opts = MySqlPoolOptions::new().after_connect(|conn| Box::pin(async move {
                    conn.execute("SET SESSION sql_mode=STRICT_ALL_TABLES; SET SESSION innodb_strict_mode=ON;").await.unwrap();
                    Ok(())
                }));
                let db_pool = match conn_uri.as_deref() {
                    Some(v) => opts.connect(v).await.unwrap(),
                    None => opts.connect_with(options.clone()).await.unwrap(),
                };
                let conn = &mut *db_pool.acquire().await.unwrap();
                sqlx::query(format!("CREATE DATABASE {}",db_name).as_str()).execute(conn).await.unwrap();
            }
    
            let options = options.database(&db_name);
            // hack to avoid problems with URI/ENV connections that already select a database
            // which prevents us from doing so via MySqlConnectOptions
            let conn_db_switch = format!("use `{}`",db_name);
            let opts = MySqlPoolOptions::new().after_connect(move|conn| {
                let c = conn_db_switch.clone();
                Box::pin(async move {
                conn.execute("SET SESSION sql_mode=STRICT_ALL_TABLES; SET SESSION innodb_strict_mode=ON;").await.unwrap();
                conn.execute(c.as_str()).await.unwrap();
                Ok(())
            })});
            let db_pool = match conn_uri.as_deref() {
                Some(v) => opts.connect(v).await.unwrap(),
                None => opts.connect_with(options.clone()).await.unwrap(),
            };

            let conn = &mut *db_pool.begin().await.unwrap();

            let res = sqlx::query_as::<_,(String,)>("SELECT DATABASE() FROM DUAL").fetch_optional(&mut *conn).await.unwrap();
            println!("Selected Database: {:?}",res);
    
            sqlx::migrate!()
            .run(&mut *db_pool.begin().await.unwrap()).await.unwrap();
    
            Self {
                db: db_pool,
                db_name,
            }
        }
    
        pub async fn conn(&self) -> PoolConnection<MySql> {
            self.db.acquire().await.unwrap()
        }
        
        /// Has to be called manually, hack due to problem with async in drop code
        pub async fn drop_async(self) {
            sqlx::query(format!("DROP DATABASE IF EXISTS `{}`",self.db_name).as_str())
                .execute(&mut *self.db.acquire().await.unwrap()).await.unwrap();
        }
    }
    
    impl Drop for DatabaseGuard {
        fn drop(&mut self) {
            // TODO: fixme, doesn't actually work, blocking will result in a deadlock
            let db = self.db.clone();
            let name = self.db_name.clone();
            spawn(async move {
                sqlx::query(format!("DROP DATABASE IF EXISTS `{}`",name).as_str()).execute(&mut *db.begin().await.unwrap()).await.unwrap();
                println!("Dropped");
            });
        }
    }
    
    #[actix_rt::test]
    async fn test_setup() {
        let db = DatabaseGuard::new().await;
        db.drop_async().await;
    }

    /// Generates user claims,key,key_type for register_user
    pub fn gen_user() -> (RegisterClaims,Vec<u8>,KeyType) {
        let mut rng = rand::thread_rng();
        let claims = RegisterClaims {
            iss: Uuid::new_v4(),
            name: random_string(&mut rng,7),
            delete_after: Some(3600),
        };
        let key: Vec<u8> = rng.sample_iter(Standard).take(16).collect();
        let key_type = KeyType::EC_PEM;
        (claims,key,key_type)
    }

    pub fn random_string(rng: &mut ThreadRng,length: usize) -> String {
        rng.sample_iter(Alphanumeric)
        .take(length)
        .map(char::from)
        .collect()
    }

    pub fn random_naive_date(rng: &mut ThreadRng,past: bool) -> NaiveDateTime {
        NaiveDateTime::from_timestamp(if past {
            rng.gen_range(0..Utc::now().naive_utc().timestamp())
        } else {
            rng.gen()
        },0)
    }

    pub fn random_future_date(rng: &mut ThreadRng) -> NaiveDateTime {
        NaiveDateTime::from_timestamp(
            rng.gen_range(Utc::now().naive_utc().timestamp()..i32::MAX as i64),0)
    }

    /// Generate user and register
    pub async fn register_test_user(conn: &mut DbConn, rng: &mut ThreadRng) -> Uuid {
        let(claims,key,key_type) = gen_user();
        crate::users::dao::register_user(conn,&claims,&key,key_type.clone()).await.unwrap();
        claims.iss
    }
}