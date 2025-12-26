进入 **Milestone 2：增量扫描引擎实现**。

这一阶段是系统的核心“心脏”。我们需要实现一个能够感知文件变化、并将其高效同步到数据库的引擎。我们将分步实现：存储适配层、扫描核心逻辑、以及增量对比算法。

### 1. 扩展存储适配层 (infra/storage)

我们使用 **OpenDAL** 来屏蔽物理路径的差异。目前先实现本地文件系统（FS）的支持。

在 `src/infra/storage/mod.rs` 中：

```rust
use opendal::{services, Operator, Builder};
use crate::models::db::Library;

pub struct StorageManager;

impl StorageManager {
    /// 根据 Library 配置初始化 OpenDAL 算子
    pub fn get_operator(library: &Library) -> anyhow::Result<Operator> {
        match library.protocol.as_str() {
            "local" => {
                let mut builder = services::Fs::default();
                builder.root(&library.base_path);
                let op = Operator::new(builder)?.finish();
                Ok(op)
            }
            "webdav" => {
                // 预留 WebDAV 实现空间
                anyhow::bail!("WebDAV 协议暂未在此阶段启用")
            }
            _ => anyhow::bail!("不支持的协议: {}", library.protocol),
        }
    }
}
```

### 2. 定义扫描引擎结构 (engine/scanner)

扫描引擎负责协调数据库读取、文件系统遍历和差异对比。

在 `src/engine/scanner/mod.rs` 中：

```rust
use std::collections::HashMap;
use opendal::{Entry, Metakey, Operator};
use sqlx::SqlitePool;
use crate::models::db::{FileEntry, Library};
use crate::infra::storage::StorageManager;
use tracing::{info, warn};

pub struct Scanner {
    db: SqlitePool,
}

impl Scanner {
    pub fn new(db: SqlitePool) -> Self {
        Self { db }
    }

    /// 执行扫描主逻辑
    pub async fn scan_library(&self, library: &Library) -> anyhow::Result<()> {
        info!("开始扫描资源库: {}", library.name);
        let op = StorageManager::get_operator(library)?;
        
        // 1. 获取数据库快照 (Path -> (Size, MTime))
        let snapshot = self.get_db_snapshot(library.id).await?;
        let mut remote_paths = snapshot; // 用于追踪哪些文件还在
        
        // 2. 递归遍历物理文件
        let mut lister = op.lister_with("/").recursive(true).await?;
        
        while let Some(entry_res) = lister.next() {
            let entry = entry_res?;
            let metadata = op.metadata(&entry, Metakey::ContentLength | Metakey::LastModified).await?;
            
            if metadata.is_dir() { continue; }

            let path = entry.path().to_string();
            let size = metadata.content_length() as i64;
            let mtime = metadata.last_modified().map(|t| t.timestamp()).unwrap_or(0);

            // 3. 差异对比
            if let Some((db_size, db_mtime)) = remote_paths.remove(&path) {
                if db_size != size || db_mtime != mtime {
                    // 文件已修改
                    self.update_file(library.id, &path, size, mtime).await?;
                }
                // 如果一致，则什么都不做
            } else {
                // 新增文件
                self.insert_file(library.id, &path, size, mtime).await?;
            }
        }

        // 4. 清理阶段：remote_paths 中剩余的即为物理上已删除的文件
        for (deleted_path, _) in remote_paths {
            self.mark_as_lost(library.id, &deleted_path).await?;
        }

        info!("资源库 {} 扫描完成", library.name);
        Ok(())
    }

    // --- 数据库操作辅助函数 ---

    async fn get_db_snapshot(&self, lib_id: i32) -> anyhow::Result<HashMap<String, (i64, i64)>> {
        let rows: Vec<(String, i64, i64)> = sqlx::query_as(
            "SELECT parent_path || filename as path, size, mtime FROM files WHERE library_id = ?"
        )
        .bind(lib_id)
        .fetch_all(&self.db)
        .await?;

        Ok(rows.into_iter().map(|(p, s, m)| (p, (s, m))).collect())
    }

    async fn insert_file(&self, lib_id: i32, full_path: &str, size: i64, mtime: i64) -> anyhow::Result<()> {
        let (parent, filename) = self.split_path(full_path);
        let ext = filename.split('.').last().map(|s| s.to_lowercase());

        sqlx::query(
            "INSERT INTO files (library_id, parent_path, filename, extension, size, mtime) VALUES (?, ?, ?, ?, ?, ?)"
        )
        .bind(lib_id).bind(parent).bind(filename).bind(ext).bind(size).bind(mtime)
        .execute(&self.db).await?;
        Ok(())
    }

    async fn update_file(&self, lib_id: i32, full_path: &str, size: i64, mtime: i64) -> anyhow::Result<()> {
        let (parent, filename) = self.split_path(full_path);
        sqlx::query(
            "UPDATE files SET size = ?, mtime = ?, status = 1 WHERE library_id = ? AND parent_path = ? AND filename = ?"
        )
        .bind(size).bind(mtime).bind(lib_id).bind(parent).bind(filename)
        .execute(&self.db).await?;
        Ok(())
    }

    async fn mark_as_lost(&self, lib_id: i32, full_path: &str) -> anyhow::Result<()> {
        let (parent, filename) = self.split_path(full_path);
        sqlx::query(
            "UPDATE files SET status = 0 WHERE library_id = ? AND parent_path = ? AND filename = ?"
        )
        .bind(lib_id).bind(parent).bind(filename)
        .execute(&self.db).await?;
        Ok(())
    }

    fn split_path(&self, full_path: &str) -> (String, String) {
        let path = std::path::Path::new(full_path);
        let parent = path.parent().and_then(|p| p.to_str()).unwrap_or("").to_string();
        let filename = path.file_name().and_then(|n| n.to_str()).unwrap_or("").to_string();
        // 保证 parent 以 / 结尾或为空
        let parent = if parent.is_empty() { parent } else { format!("{}/", parent) };
        (parent, filename)
    }
}
```

### 3. 在主程序中集成测试

为了验证效果，我们在 `main.rs` 中手动创建一个测试资源库并运行扫描。

```rust
// src/main.rs (部分更新)
use crate::engine::scanner::Scanner;
use crate::models::db::Library;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // ... 前面的初始化代码 ...
    let pool = infra::db::init_db(db_url).await?;

    // 1. 创建一个测试资源库 (如果不存在)
    sqlx::query("INSERT OR IGNORE INTO libraries (id, name, protocol, base_path) VALUES (1, '测试库', 'local', './test_dir')")
        .execute(&pool).await?;

    // 2. 准备测试库对应的实体
    let test_lib = Library {
        id: 1,
        name: "测试库".to_string(),
        protocol: "local".to_string(),
        base_path: "./test_dir".to_string(), // 请确保本地有这个文件夹
        config_json: None,
        last_scanned_at: None,
    };

    // 3. 执行扫描
    let scanner = Scanner::new(pool.clone());
    scanner.scan_library(&test_lib).await?;

    info!("扫描测试完成。");
    Ok(())
}
```

### 4. Milestone 2 完成要点

1.  **OpenDAL 抽象：** 成功通过 `Operator` 读取文件系统。
2.  **增量逻辑：** 通过 `HashMap` 记录数据库快照，对比 `mtime` 和 `size`。
3.  **状态管理：** 实现了文件丢失（Status=0）的处理逻辑。
4.  **路径处理：** 能够正确拆分 `parent_path` 和 `filename`，为后续的层级标签奠定基础。

### 下一步建议

**Milestone 3：层级标签引擎 (Hierarchical Tagger)。**

我们需要实现：
1.  自动从 `parent_path` 提取目录名称。
2.  递归地在 `tags` 表中创建父子关系。
3.  将扫描到的文件自动关联到对应的标签 ID。

**是否继续进行 Milestone 3 的开发？**