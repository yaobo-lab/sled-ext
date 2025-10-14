pub use bincode::{Decode, Encode};
use serde::{Deserialize, Serialize};
use sled_ext::{KvDb, KvDbConfig, def_ttl_cleanup, set_expire_event};
use std::time::Duration;
use std::{process, sync::Arc};
use tokio::time::sleep;
use toolkit_rs::{
    logger::{self, LogConfig},
    painc::{PaincConf, set_panic_handler},
};

#[derive(Decode, Encode, Debug, Serialize, Deserialize)]
pub struct TestStruct {
    pub level: u8,
    pub size: usize,
    pub console: bool,
    pub file: String,
    pub bounded: Option<usize>,
    pub filters: Option<Vec<String>>,
}
impl Default for TestStruct {
    fn default() -> Self {
        Self {
            level: 1,
            size: 1024 * 1024,
            console: true,
            file: "log.log".to_string(),
            bounded: Some(10),
            filters: None,
        }
    }
}

#[tokio::main]
async fn main() {
    set_panic_handler(PaincConf::default());

    let cfg = LogConfig {
        filters: Some(vec!["sled".to_string()]),
        ..LogConfig::default()
    };

    logger::setup(cfg).unwrap_or_else(|e| {
        println!("log setup err:{}", e);
        process::exit(1);
    });

    let cfg = KvDbConfig {
        path: "kv_db".to_string(),
        cache_capacity: 1024 * 1024,
        flush_every_ms: 1000,
    };
    let key = "hello-1";

    let db = KvDb::new(cfg).expect("db init failed");
    let db = Arc::new(db);
    def_ttl_cleanup(db.clone(), Some(Duration::from_secs(5)), Some(100));
    set_expire_event(db.clone(), |key| println!("expire key-->: {key}"));

    //basic
    db.insert(&key, 12).expect("insert failed");
    let v = db.get::<_, i32>(&key);
    println!("get key: {},value: {:?}", key, v);
    db.insert(&key, 14).expect("insert failed");
    println!("get key: {},value: {:?}", key, v);

    //struct
    let struct_key = "struct-1";
    let s = TestStruct::default();
    db.insert(struct_key, &s).expect("insert failed");
    sleep(Duration::from_secs(1)).await;
    let v = db.get::<_, TestStruct>(struct_key);
    println!("get struct key: {},value: {:?}", struct_key, v);

    //ttl
    let ttl_key = "hello-2";
    db.insert_ttl(&ttl_key, 13, Duration::from_secs(5))
        .expect("insert failed");

    sleep(Duration::from_secs(3)).await;
    let v = db.get::<_, i32>(&ttl_key);
    println!("after 3 sec get ttl_key: {},value: {:?}", ttl_key, v);
    let ttl_v = db.get_ttl_at(&ttl_key);
    println!("get_ttl_at value: {:?}", ttl_v);

    let is_expired = db.is_expired(&ttl_key);
    println!("is_expired : {:?}", is_expired);

    let contains_key = db.contains_key(&ttl_key);
    println!("contains_key : {}", contains_key);

    sleep(Duration::from_secs(3)).await;
    let v = db.get::<_, i32>(&ttl_key);
    println!("after 3 sec get ttl_key: {},value: {:?}", ttl_key, v);

    let contains_key = db.contains_key(&ttl_key);
    println!("contains_key : {}", contains_key);
}
