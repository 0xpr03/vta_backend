use chrono::NaiveDateTime;

pub type Timestamp = NaiveDateTime;
pub use tracing::*;
pub use uuid::Uuid;
pub use color_eyre::eyre::Context;
pub use serde::{Deserialize, Serialize};
pub use crate::state::AppState;

#[cfg(test)]
pub mod tests {
    use std::iter::repeat;

    use actix_rt::spawn;
    use rand::{Rng, distributions::Alphanumeric};
    use sqlx::{mysql::{MySqlConnectOptions, MySqlPoolOptions}, pool::PoolConnection, MySql, Executor};

    use crate::Pool;

    pub struct DatabaseGuard {
        pub db: Pool,
        pub db_name: String,
    }
    
    impl DatabaseGuard {
        pub async fn new() -> Self {
            // ignore on purpose, can only be installed once
            // let _ = color_eyre::install();
            let mut rng = rand::thread_rng();
            let db_name: String = format!("testing_temp_{}",repeat(())
            .map(|()| rng.sample(Alphanumeric))
            .map(char::from)
            .take(7)
            .collect::<String>());
    
            let options = MySqlConnectOptions::new()
            .host("127.0.0.1")
            .port(3306)
            .username("root");
            // if let Some(pw) = Some("root") {
            //     options = options.password(&pw);
            // }
    
            {
                let db_pool = MySqlPoolOptions::new().after_connect(|conn| Box::pin(async move {
                    conn.execute("SET SESSION sql_mode=STRICT_ALL_TABLES; SET SESSION innodb_strict_mode=ON;").await.unwrap();
                    Ok(())
                })).connect_with(options.clone()).await.unwrap();
                let conn = &mut *db_pool.acquire().await.unwrap();
                sqlx::query(format!("CREATE DATABASE {}",db_name).as_str()).execute(conn).await.unwrap();
            }
    
            let options = options.database(&db_name);
            let db_pool = MySqlPoolOptions::new().after_connect(|conn| Box::pin(async move {
                conn.execute("SET SESSION sql_mode=STRICT_ALL_TABLES; SET SESSION innodb_strict_mode=ON;").await.unwrap();
                Ok(())
            })).connect_with(options.clone()).await.unwrap();
    
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
    
        pub async fn drop_async(self) {
            sqlx::query(format!("DROP DATABASE IF EXISTS {}",self.db_name).as_str())
                .execute(&mut *self.db.begin().await.unwrap()).await.unwrap();
        }
    }
    
    impl Drop for DatabaseGuard {
        fn drop(&mut self) {
            // TODO: fixme, doesn't actually work, blocking will result in a deadlock
            let db = self.db.clone();
            let name = self.db_name.clone();
            spawn(async move {
                sqlx::query(format!("DROP DATABASE IF EXISTS {}",name).as_str()).execute(&mut *db.begin().await.unwrap()).await.unwrap();
                println!("Dropped");
            });
        }
    }
    
    #[actix_rt::test]
    async fn test_setup() {
        let db = DatabaseGuard::new().await;
        db.drop_async().await;
    }
}