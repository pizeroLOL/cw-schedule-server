use std::{
    sync::{Arc, LazyLock, RwLock},
    time::Duration,
};

use serde::{Deserialize, Serialize};
use sqlx::{sqlite::SqliteConnectOptions, Executor, Pool, Sqlite};

pub static GLOBLE_STORE: LazyLock<Arc<RwLock<Option<Store>>>> =
    LazyLock::new(|| Arc::new(RwLock::new(None)));

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Config {
    pub db: String,
    pub heartbeat: Duration,
    pub timeout: Duration,
    pub max_timeout: u8,
}

impl Default for Config {
    fn default() -> Self {
        Config {
            db: "db.sqlite".to_string(),
            heartbeat: Duration::from_secs(5),
            timeout: Duration::from_secs(20),
            max_timeout: 6,
        }
    }
}

#[derive(Debug, Clone)]
#[allow(unused)]
pub struct Store {
    pub db: Pool<Sqlite>,
    pub config: Arc<Config>,
}

impl Store {
    pub async fn new(cfg: &Config) -> Self {
        Self {
            db: Self::init_db(&cfg.db).await,
            config: Arc::new(cfg.clone()),
        }
    }
    async fn init_db(db_path: &str) -> Pool<Sqlite> {
        let options = SqliteConnectOptions::new()
            .create_if_missing(true)
            .filename(db_path);
        let pool = Pool::connect_with(options).await.unwrap();
        pool.execute(include_str!("init.sql")).await.unwrap();
        pool
    }
}
