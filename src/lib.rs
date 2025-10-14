use anyhow::Result;
#[cfg(feature = "ttl")]
use anyhow::anyhow;
pub use bincode::{Decode, Encode};
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
#[cfg(feature = "ttl")]
use sled::Event;
#[cfg(feature = "ttl")]
use sled::Transactional;
#[cfg(feature = "ttl")]
use sled::transaction::ConflictableTransactionError;
use sled::{Config, Db};

#[cfg(feature = "ttl")]
use std::sync::Arc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};
fn _now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs()
}

fn expired_time(ttl: Duration) -> u64 {
    SystemTime::now()
        .checked_add(ttl)
        .unwrap()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs()
}

pub trait ISledExt {
    fn expire<K>(&self, key: K, ttl: Duration) -> Result<bool>
    where
        K: AsRef<[u8]> + Sync + Send;
}

impl ISledExt for Db {
    fn expire<K>(&self, key: K, ttl: Duration) -> Result<bool>
    where
        K: AsRef<[u8]> + Sync + Send,
    {
        let expire_at = expired_time(ttl).to_be_bytes();
        self.insert(key, expire_at.as_slice())?;
        Ok(true)
    }
}

#[derive(Serialize, Deserialize)]
pub struct KvDbConfig {
    pub path: String,
    pub cache_capacity: u64,
    pub flush_every_ms: u64,
}

const KV_TREE: &[u8] = b"__kv_tree@";
const _TTL_TREE: &[u8] = b"__tll_tree@";

pub struct KvDb {
    pub(crate) kv_tree: sled::Tree,
    #[cfg(feature = "ttl")]
    pub(crate) ttl_tree: sled::Tree,
}

#[cfg(feature = "ttl")]
pub fn def_ttl_cleanup(db: Arc<KvDb>, interval: Option<Duration>, limit: Option<usize>) {
    let t = match interval {
        Some(d) => d,
        None => Duration::from_secs(3),
    };
    let limit = match limit {
        Some(l) => l,
        None => 200,
    };
    tokio::spawn(async move {
        loop {
            tokio::time::sleep(t).await;
            loop {
                let now = std::time::Instant::now();
                let count = db.cleanup(limit);
                if count > 0 {
                    log::debug!("cleanup count: {}, cost time: {:?}", count, now.elapsed());
                }
                if count < limit {
                    break;
                }
                tokio::time::sleep(std::time::Duration::from_millis(300)).await;
            }
        }
    });
}

#[cfg(feature = "ttl")]
pub fn set_expire_event<F>(db: Arc<KvDb>, _evt: F)
where
    F: Fn(String) + Send + Sync + 'static,
{
    tokio::spawn(async move {
        for event in db.ttl_tree.watch_prefix(vec![]) {
            match event {
                Event::Remove { key } => {
                    let key = String::from_utf8_lossy(&key).into_owned();
                    _evt(key);
                }
                _ => {}
            }
        }
    });
}

impl KvDb {
    pub fn new(cfg: KvDbConfig) -> Result<Self> {
        let c = Config::default()
            .path(cfg.path)
            .cache_capacity(cfg.cache_capacity)
            .flush_every_ms(Some(cfg.flush_every_ms))
            .mode(sled::Mode::LowSpace);
        let db = c.open()?;
        let kv_tree = db.open_tree(KV_TREE)?;
        #[cfg(feature = "ttl")]
        let ttl_tree = db.open_tree(_TTL_TREE)?;

        // let db = Arc::new(db);
        Ok(KvDb {
            kv_tree,
            #[cfg(feature = "ttl")]
            ttl_tree,
        })
    }

    #[cfg(feature = "ttl")]
    fn cleanup(&self, limit: usize) -> usize {
        let mut count = 0;

        for item in self.ttl_tree.iter() {
            if count > limit {
                break;
            }
            let (key, expire_at_iv) = match item {
                Ok(item) => item,
                Err(e) => {
                    log::error!("cleanup err: {:?}", e);
                    break;
                }
            };

            let expire_at = match expire_at_iv.as_ref().try_into() {
                Ok(at) => u64::from_be_bytes(at),
                Err(e) => {
                    log::error!("cleanup err: {:?}", e);
                    break;
                }
            };

            if expire_at > _now() {
                break;
            }

            if let Err(e) = (&self.kv_tree, &self.ttl_tree).transaction(|(kv, exp)| {
                kv.remove(key.clone())?;
                exp.remove(key.clone())?;
                Ok::<_, ConflictableTransactionError<()>>(())
            }) {
                log::error!("cleanup err: {:?}", e);
            } else {
                count += 1;
            }
        }
        count
    }

    #[cfg(feature = "ttl")]
    pub fn get_ttl_at<K>(&self, key: K) -> Option<u64>
    where
        K: AsRef<[u8]> + Sync + Send,
    {
        let expire_at_iv = match self.ttl_tree.get(key.as_ref()) {
            Ok(Some(at_bytes)) => at_bytes,
            Ok(None) => return None,
            Err(e) => {
                log::error!("get_ttl_at err: {:?}", e);
                return None;
            }
        };

        let expire_at = match expire_at_iv.as_ref().try_into() {
            Ok(at) => u64::from_be_bytes(at),
            Err(e) => {
                log::error!("get_ttl_at err: {:?}", e);
                return None;
            }
        };

        Some(expire_at)
    }

    #[cfg(feature = "ttl")]
    pub fn is_expired<K>(&self, key: K) -> Option<bool>
    where
        K: AsRef<[u8]> + Sync + Send,
    {
        let expire_at = self.get_ttl_at(key);

        let Some(expire_at) = expire_at else {
            return None;
        };

        if _now() > expire_at {
            return Some(true);
        }

        Some(false)
    }

    #[cfg(feature = "ttl")]
    pub fn insert_ttl<K, V>(&self, key: K, value: V, ttl: Duration) -> Result<()>
    where
        K: AsRef<[u8]>,
        V: Serialize + Encode + Sync + Send,
    {
        let v = bincode::encode_to_vec(value, bincode::config::standard())?;
        let expire_at = expired_time(ttl).to_be_bytes();

        if let Err(e) = (&self.kv_tree, &self.ttl_tree).transaction(|(kv, ttl)| {
            kv.insert(key.as_ref(), v.clone())?;
            ttl.insert(key.as_ref(), expire_at.as_slice())?;
            Ok::<_, ConflictableTransactionError<()>>(())
        }) {
            return Err(anyhow!("insert_ttl err: {:?}", e));
        }
        Ok(())
    }

    pub fn insert<K, V>(&self, key: K, value: V) -> Result<()>
    where
        K: AsRef<[u8]>,
        V: Serialize + Encode + Sync + Send,
    {
        let v = bincode::encode_to_vec(value, bincode::config::standard())?;
        self.kv_tree.insert(key, v)?;
        Ok(())
    }

    pub fn contains_key<K>(&self, key: K) -> bool
    where
        K: AsRef<[u8]> + Sync + Send,
    {
        #[cfg(feature = "ttl")]
        {
            let exp_v = self.is_expired(&key);

            //如果ttl 存在，并已过期 则返回false
            if let Some(v) = exp_v
                && v
            {
                return false;
            }
        }

        self.kv_tree.contains_key(key).ok().unwrap_or(false)
    }

    pub fn get<K, V>(&self, key: K) -> Option<V>
    where
        K: AsRef<[u8]>,
        V: DeserializeOwned + Decode<()> + Sync + Send,
    {
        let val = match self.kv_tree.get(key) {
            Ok(v) => v,
            Err(e) => {
                log::error!("kvdb get err: {}", e);
                return None;
            }
        };

        if let Some(v) = val {
            let b = bincode::decode_from_slice::<V, _>(v.as_ref(), bincode::config::standard());
            if let Ok((v, _)) = b {
                return Some(v);
            }
            if let Err(e) = b {
                log::error!("kvdb deserialize error: {}", e.to_string());
            }
            return None;
        }

        None
    }
}
