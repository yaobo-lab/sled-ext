pub use bincode::{Decode, Encode};
use chrono::Local;
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

    let db = KvDb::new(cfg).expect("db init failed");
    db.clean().expect(" clean failed");
    let db = Arc::new(db);
    def_ttl_cleanup(db.clone(), Some(Duration::from_secs(5)), Some(100));
    set_expire_event(db.clone(), |key| {
        let now = Local::now().format("%Y-%m-%d %H:%M:%S");
        println!("{}: expire key-->: {key}", now);
    });

    //basic
    println!("\n\n basic  test------------------->");
    let key = "hello-1";
    let v = db.get::<_, i32>(&key);
    println!("get clean key: {},value: {:?}", key, v);

    db.insert_or_update(&key, 12).expect("insert failed");
    let v = db.get::<_, i32>(&key);
    println!("get key: {},value: {:?}", key, v);
    db.insert_or_update(&key, 14).expect("insert failed");
    println!("get key: {},value: {:?}", key, v);

    //delete
    println!("\n\ndelete test------------------->");
    let key_delete = "key-delete-1";
    db.insert_or_update(&key_delete, 12).expect("insert failed");
    let v = db.get::<_, i32>(&key_delete);
    println!("get key: {},value: {:?}", key_delete, v);
    db.remove(&key_delete).expect("remove failed");
    let v = db.get::<_, i32>(&key_delete);
    println!("get key: {},value: {:?}", key_delete, v);

    //update
    println!("\n\n update test------------------->");
    let key_update = "key-update-1";
    db.insert_or_update(&key_update, 13).expect("insert failed");
    let v = db.get::<_, i32>(&key_update);
    println!("get key: {},value: {:?}", key_update, v);
    db.insert_or_update(&key_update, 14).expect("insert failed");
    let v = db.get::<_, i32>(&key_update);
    println!("get key: {},value: {:?}", key_update, v);

    //other
    println!("\n\n other test------------------->");
    let key_other = "key-other-1";
    db.insert_or_update(&key_other, 12).expect("insert failed");
    let v = db.get::<_, Vec<u8>>(&key_other);
    println!("get key_other: {},value: {:?}", key_other, v);

    let key_other2 = "key-other-2";
    db.insert_or_update(&key_other2, true)
        .expect("insert failed");
    let v = db.get::<_, Vec<u8>>(&key_other2);
    println!("get key: {},value: {:?}", key_other2, v);

    //struct
    println!("\n\n struct test------------------->");
    let struct_key = "struct-1";
    let s = TestStruct::default();
    db.insert_or_update(struct_key, &s).expect("insert failed");
    sleep(Duration::from_secs(1)).await;
    let v = db.get::<_, TestStruct>(struct_key);
    println!("get struct key: {},value: {:?}", struct_key, v);

    //ttl
    println!("\n\n ttl test------------------->");
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

    // ttl refresh
    println!("\n\n ttl refresh test------------------->");
    let ttl_ref_key = "hello-refresh-2";
    println!("current time: {}", Local::now().format("%Y-%m-%d %H:%M:%S"));
    db.insert_ttl(&ttl_ref_key, 13, Duration::from_secs(5))
        .expect("insert failed");
    let at = db.get_ttl_at(&ttl_ref_key);
    println!("get_ttl_at: {},at: {:?}", ttl_ref_key, at);

    sleep(Duration::from_secs(3)).await;
    let at = db.get_ttl_at(&ttl_ref_key);
    println!("after 3 sec get ttl_key: {},at: {:?}", ttl_ref_key, at);

    println!(
        "refresh current time: {}",
        Local::now().format("%Y-%m-%d %H:%M:%S")
    );
    db.refresh_ttl(&ttl_ref_key, Duration::from_secs(5))
        .expect(" refresh ttl failed");
    let at = db.get_ttl_at(&ttl_ref_key);
    println!("get_ttl_at: {},at: {:?}", ttl_ref_key, at);

    sleep(Duration::from_secs(6)).await;
    std::process::exit(0);
}
