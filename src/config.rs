use config::{Config, ConfigError, Environment, File};
use serde::Deserialize;

#[derive(Debug, Deserialize)]
pub struct Database {
    pub host: String,
    pub port: u16,
    pub user: String,
    pub password: Option<String>,
    pub db: String,
    pub max_conn: u32,
}

#[derive(Debug, Deserialize)]
pub struct Settings {
    pub database: Database,
    pub listen_ip: String,
    pub listen_port: u16,
}

impl Settings {
    pub fn new() -> Result<Self, ConfigError> {
        let mut s = Config::default();
        s.merge(File::with_name("config/config.toml").required(false))?;
        s.merge(File::with_name("config/config.ini").required(false))?;

        // Add in a local configuration file
        // This file shouldn't be checked in to git
        s.merge(File::with_name("config/local").required(false))?;

        // Add in settings from the environment (with a prefix of APP)
        // Eg.. `APP_DEBUG=1 ./target/app` would set the `debug` key
        s.merge(Environment::with_prefix("app"))?;
        // You can deserialize (and thus freeze) the entire configuration as
        s.try_into()
    }
}
