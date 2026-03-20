# sled-ext

`sled-ext` 是一个基于 [`sled`](https://github.com/spacejam/sled) 的轻量级扩展库，提供更易用的键值读写接口，以及开箱即用的 TTL 过期能力。

它适合这类场景：

- 想继续使用 `sled` 作为本地嵌入式数据库
- 希望用泛型 API 直接存取 Rust 类型
- 需要为 key 设置过期时间，并定期清理失效数据

## 特性

- 基于 `sled` 的本地 KV 存储封装
- 支持泛型读写，适合结构体和基础类型
- 支持 TTL 写入、续期、过期检查
- 支持后台定时清理过期 key
- 支持监听过期删除事件

## 安装

```bash
cargo add sled-ext
```

默认启用 `ttl` 功能。

## 快速开始

```rust
use serde::{Deserialize, Serialize};
use sled_ext::{Decode, Encode, KvDb, KvDbConfig};

#[derive(Debug, Serialize, Deserialize, Encode, Decode)]
struct User {
    id: u64,
    name: String,
}

fn main() -> anyhow::Result<()> {
    let db = KvDb::new(KvDbConfig {
        path: "data/demo-db".to_string(),
        cache_capacity: 1024 * 1024,
        flush_every_ms: 1000,
    })?;

    let user = User {
        id: 1,
        name: "Alice".to_string(),
    };

    db.insert_or_update("user:1", &user)?;

    let value = db.get::<_, User>("user:1");
    println!("{value:?}");

    Ok(())
}
```

## TTL 用法

启用默认特性后，可以直接使用 TTL 相关能力。

```rust
use sled_ext::{KvDb, KvDbConfig, def_ttl_cleanup, set_expire_event};
use std::sync::Arc;
use std::time::Duration;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let db = Arc::new(KvDb::new(KvDbConfig {
        path: "data/ttl-db".to_string(),
        cache_capacity: 1024 * 1024,
        flush_every_ms: 1000,
    })?);

    db.insert_ttl("session:1", "token-123", Duration::from_secs(10))?;

    def_ttl_cleanup(db.clone(), Some(Duration::from_secs(3)), Some(100));

    set_expire_event(db.clone(), |key| {
        println!("expired key: {key}");
    });

    let expire_at = db.get_ttl_at("session:1");
    println!("expire_at: {expire_at:?}");

    let exists = db.contains_key("session:1");
    println!("contains_key: {exists}");

    db.refresh_ttl("session:1", Duration::from_secs(30))?;

    Ok(())
}
```

## 运行示例

仓库自带了完整示例：

```bash
cargo run --example demo
```

示例覆盖了以下能力：

- 基础写入与读取
- 删除 key
- 更新 value
- 读取结构体
- TTL 写入与到期检查
- TTL 续期
- 过期事件监听

## 主要 API

### `KvDb`

- `KvDb::new(config)`：初始化数据库
- `insert_or_update(key, value)`：插入或覆盖数据
- `get::<_, T>(key)`：按类型读取数据
- `contains_key(key)`：检查 key 是否存在
- `remove(key)`：删除 key
- `clean()`：清空数据库内容

### TTL 相关

- `insert_ttl(key, value, ttl)`：写入带过期时间的数据
- `get_ttl_at(key)`：获取过期时间戳
- `is_expired(key)`：检查 key 是否已过期
- `refresh_ttl(key, ttl)`：刷新过期时间
- `def_ttl_cleanup(db, interval, limit)`：启动后台清理任务
- `set_expire_event(db, callback)`：监听过期删除事件

## 数据编码说明

- 写入数据时使用 `bincode` 编码
- 建议类型同时派生 `Serialize`、`Deserialize`、`Encode`、`Decode`
- 基础类型如 `i32`、`bool`、`String` 可直接存取

示例：

```rust
#[derive(Debug, Serialize, Deserialize, Encode, Decode)]
struct AppConfig {
    enabled: bool,
    retries: u32,
}
```

## 使用建议

- TTL 数据只有在执行清理任务后才会从底层存储中删除
- 如果你依赖自动清理，建议在应用启动时调用 `def_ttl_cleanup`
- `contains_key` 会结合 TTL 状态判断，因此过期 key 会返回 `false`
- 过期事件监听依赖后台删除动作触发

## 项目信息

- Crate: `sled-ext`
- Repository: <https://github.com/yaobo-lab/sled-ext>
- License: MIT
