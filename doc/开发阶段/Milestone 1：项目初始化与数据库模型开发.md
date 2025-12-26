好的，我们正式进入 **Milestone 1：项目初始化与数据库模型开发**。

在这一步，我们将搭建 Rust 项目骨架，配置必要的依赖，并使用 **SQLx** 实现数据库的 Schema 落地和模型映射。

---

### 1. 项目初始化与依赖配置

首先，创建一个新的 Rust 项目并配置 `Cargo.toml`。

```bash
cargo new tagflow-core
cd tagflow-core
```

编辑 **`Cargo.toml`**，引入核心依赖：

```toml
[package]
name = "tagflow-core"
version = "0.1.0"
edition = "2021"

[dependencies]
# 异步运行时
tokio = { version = "1", features = ["full"] }
# Web 框架
axum = "0.7"
# 数据库适配器 (SQLite + 异步)
sqlx = { version = "0.8", features = ["runtime-tokio", "sqlite", "chrono", "macros"] }
# 序列化
serde = { version = "1.0", features = ["derive"] }
serde_json = "1.0"
# 时间处理
chrono = { version = "0.4", features = ["serde"] }
# 错误处理
thiserror = "1.0"
anyhow = "1.0"
# 日志
tracing = "0.1"
tracing-subscriber = "0.3"
# 存储抽象 (未来使用)
opendal = "0.50"
```

---

### 2. 数据库迁移脚本 (Migrations)

创建 `migrations` 文件夹，并编写第一个 SQL 迁移文件，以确保数据库结构的确定性。

创建目录：`mkdir migrations`
创建文件：`migrations/202512260001_init_schema.sql`

```sql
-- 1. 资源库表
CREATE TABLE IF NOT EXISTS libraries (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    name TEXT NOT NULL,
    protocol TEXT NOT NULL,         -- 'local', 'webdav'
    base_path TEXT NOT NULL,
    config_json TEXT,               -- 存储加密凭据或排除规则
    last_scanned_at DATETIME
);

-- 2. 标签表 (层级结构)
CREATE TABLE IF NOT EXISTS tags (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    name TEXT NOT NULL,
    category TEXT NOT NULL,         -- 'path', 'type', 'user', 'time'
    parent_id INTEGER REFERENCES tags(id) ON DELETE CASCADE,
    UNIQUE(name, parent_id)
);

-- 3. 文件索引表
CREATE TABLE IF NOT EXISTS files (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    library_id INTEGER NOT NULL REFERENCES libraries(id) ON DELETE CASCADE,
    parent_path TEXT NOT NULL,      -- 相对路径
    filename TEXT NOT NULL,
    extension TEXT,                 -- 后缀
    size INTEGER NOT NULL,
    mtime INTEGER NOT NULL,         -- 修改时间戳
    hash TEXT,                      -- 用于检测移动
    status INTEGER DEFAULT 1,       -- 1:在线, 0:丢失
    indexed_at DATETIME DEFAULT CURRENT_TIMESTAMP
);

-- 4. 文件-标签关联表
CREATE TABLE IF NOT EXISTS file_tags (
    file_id INTEGER NOT NULL REFERENCES files(id) ON DELETE CASCADE,
    tag_id INTEGER NOT NULL REFERENCES tags(id) ON DELETE CASCADE,
    source TEXT DEFAULT 'auto',     -- 'auto', 'manual'
    PRIMARY KEY(file_id, tag_id)
);

-- 5. 创建基础索引
CREATE INDEX IF NOT EXISTS idx_files_lookup ON files(library_id, parent_path, filename);
CREATE INDEX IF NOT EXISTS idx_tags_parent ON tags(parent_id);
```

---

### 3. 定义 Rust 领域模型 (Models)

在 `src/models/db.rs` 中定义与数据库对应的结构体。利用 SQLx 的 `FromRow` 宏实现自动映射。

```rust
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::FromRow;

#[derive(Debug, Serialize, Deserialize, FromRow)]
pub struct Library {
    pub id: i32,
    pub name: String,
    pub protocol: String,
    pub base_path: String,
    pub config_json: Option<String>,
    pub last_scanned_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Serialize, Deserialize, FromRow)]
pub struct Tag {
    pub id: i32,
    pub name: String,
    pub category: String,
    pub parent_id: Option<i32>,
}

#[derive(Debug, Serialize, Deserialize, FromRow)]
pub struct FileEntry {
    pub id: i32,
    pub library_id: i32,
    pub parent_path: String,
    pub filename: String,
    pub extension: Option<String>,
    pub size: i64,
    pub mtime: i64,
    pub hash: Option<String>,
    pub status: i32,
    pub indexed_at: DateTime<Utc>,
}
```

---

### 4. 基础设施：数据库连接池初始化

在 `src/infra/db.rs` 中实现数据库连接和自动迁移逻辑。

```rust
use sqlx::sqlite::{SqlitePool, SqlitePoolOptions};
use std::time::Duration;

pub async fn init_db(database_url: &str) -> anyhow::Result<SqlitePool> {
    // 1. 创建连接池
    let pool = SqlitePoolOptions::new()
        .max_connections(5)
        .acquire_timeout(Duration::from_secs(3))
        .connect(database_url)
        .await?;

    // 2. 强制开启 WAL 模式以支持并发
    sqlx::query("PRAGMA journal_mode = WAL;").execute(&pool).await?;
    sqlx::query("PRAGMA foreign_keys = ON;").execute(&pool).await?;

    // 3. 执行迁移脚本
    sqlx::migrate!("./migrations")
        .run(&pool)
        .await?;

    Ok(pool)
}
```

---

### 5. 主程序入口

在 `src/main.rs` 中将各部分连接起来。

```rust
mod models;
mod infra;

use tracing::{info, Level};
use tracing_subscriber::FmtSubscriber;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // 初始化日志
    let subscriber = FmtSubscriber::builder()
        .with_max_level(Level::INFO)
        .finish();
    tracing::subscriber::set_global_default(subscriber)?;

    info!("TagFlow Core 正在启动...");

    // 初始化数据库 (本地文件 tagflow.db)
    let db_url = "sqlite:tagflow.db?mode=rwc";
    let pool = infra::db::init_db(db_url).await?;
    
    info!("数据库初始化成功并已应用迁移。");

    // TODO: 启动扫描引擎与 API 服务
    
    Ok(())
}
```

---

### 下一步建议

现在我们已经完成了 **Milestone 1**。项目已经可以启动，并自动在本地生成带 Schema 的 `tagflow.db` 文件。

**Milestone 2 的核心工作是：增量扫描引擎的实现。**
我们需要实现：
1. `Scanner` 结构体。
2. 调用 `OpenDAL` 遍历文件系统。
3. 编写 SQL 逻辑对比文件指纹，实现增量入库。

**是否继续进行 Milestone 2 的 Scanner 模块开发？**