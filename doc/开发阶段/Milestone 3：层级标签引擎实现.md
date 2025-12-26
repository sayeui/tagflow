进入 **Milestone 3：层级标签引擎 (Hierarchical Tagger) 实现**。

在这一阶段，我们将实现 TagFlow 的核心灵魂——将文件的物理路径及元数据自动转化为逻辑上的层级标签，并建立文件与标签的多对多关联。

### 1. 完善领域模型 (Models)

为了处理标签的层级关系，我们在 `src/models/db.rs` 中补充一些类型定义：

```rust
#[derive(Debug, Clone, PartialEq)]
pub enum TagCategory {
    Path,
    Type,
    User,
    Time,
}

impl ToString for TagCategory {
    fn to_string(&self) -> String {
        match self {
            TagCategory::Path => "path".to_string(),
            TagCategory::Type => "type".to_string(),
            TagCategory::User => "user".to_string(),
            TagCategory::Time => "time".to_string(),
        }
    }
}
```

### 2. 定义标签管理核心 (core/tag)

我们需要一个 `TagManager` 来处理“确保标签链路存在”的逻辑。它会递归地检查或创建父子标签，并返回叶子节点的 ID。

在 `src/core/tag/mod.rs` 中：

```rust
use sqlx::SqlitePool;
use crate::models::db::Tag;
use tracing::debug;

pub struct TagManager {
    db: SqlitePool,
}

impl TagManager {
    pub fn new(db: SqlitePool) -> Self {
        Self { db }
    }

    /// 确保一个层级标签路径存在。例如输入 ["Work", "Design", "2025"]
    /// 返回最后一个标签 ("2025") 的 ID
    pub async fn ensure_path_tags(&self, parts: Vec<String>) -> anyhow::Result<i32> {
        let mut last_parent_id: Option<i32> = None;

        for part in parts {
            if part.is_empty() { continue; }
            
            // 尝试查找或插入当前层级的标签
            // 使用 SQLite 的 INSERT OR IGNORE 和随后查询，或者更简单的逻辑：
            let row: Option<(i32,)> = sqlx::query_as(
                "SELECT id FROM tags WHERE name = ? AND (parent_id = ? OR (parent_id IS NULL AND ? IS NULL))"
            )
            .bind(&part)
            .bind(last_parent_id)
            .bind(last_parent_id)
            .fetch_optional(&self.db)
            .await?;

            let id = if let Some((existing_id,)) = row {
                existing_id
            } else {
                let res = sqlx::query(
                    "INSERT INTO tags (name, category, parent_id) VALUES (?, 'path', ?)"
                )
                .bind(&part)
                .bind(last_parent_id)
                .execute(&self.db)
                .await?;
                res.last_insert_rowid() as i32
            };
            
            last_parent_id = Some(id);
        }

        last_parent_id.ok_or_else(|| anyhow::anyhow!("路径为空，无法生成标签"))
    }

    /// 建立文件与标签的关联
    pub async fn link_file_to_tag(&self, file_id: i32, tag_id: i32, source: &str) -> anyhow::Result<()> {
        sqlx::query(
            "INSERT OR IGNORE INTO file_tags (file_id, tag_id, source) VALUES (?, ?, ?)"
        )
        .bind(file_id)
        .bind(tag_id)
        .bind(source)
        .execute(&self.db)
        .await?;
        Ok(())
    }
}
```

### 3. 实现自动化标签流水线 (engine/tagger)

我们定义一个 `Tagger` trait，方便未来扩展（如日期标签、MIME标签）。目前专注实现 `PathTagger`。

在 `src/engine/tagger/mod.rs` 中：

```rust
use crate::core::tag::TagManager;

pub struct PathTagger {
    tag_manager: TagManager,
}

impl PathTagger {
    pub fn new(tag_manager: TagManager) -> Self {
        Self { tag_manager }
    }

    /// 处理文件的路径标签
    pub async fn process_path(&self, file_id: i32, parent_path: &str) -> anyhow::Result<()> {
        // 将 "Projects/2024/Design/" 拆分为 ["Projects", "2024", "Design"]
        let parts: Vec<String> = parent_path
            .split('/')
            .filter(|s| !s.is_empty())
            .map(|s| s.to_string())
            .collect();

        if !parts.is_empty() {
            // 确保层级标签存在并获取叶子 ID
            let leaf_tag_id = self.tag_manager.ensure_path_tags(parts).await?;
            // 关联文件到叶子标签
            self.tag_manager.link_file_to_tag(file_id, leaf_tag_id, "auto").await?;
        }

        Ok(())
    }
}
```

### 4. 集成到扫描引擎 (Scanner)

现在，我们需要在 `Scanner` 插入新文件后，自动触发标签处理逻辑。

修改 `src/engine/scanner/mod.rs`：

```rust
// ... 之前的 import ...
use crate::engine::tagger::PathTagger;
use crate::core::tag::TagManager;

impl Scanner {
    // ... get_db_snapshot 等函数 ...

    async fn insert_file(&self, lib_id: i32, full_path: &str, size: i64, mtime: i64) -> anyhow::Result<()> {
        let (parent, filename) = self.split_path(full_path);
        let ext = filename.split('.').last().map(|s| s.to_lowercase());

        // 1. 插入文件记录
        let res = sqlx::query(
            "INSERT INTO files (library_id, parent_path, filename, extension, size, mtime) VALUES (?, ?, ?, ?, ?, ?)"
        )
        .bind(lib_id).bind(&parent).bind(&filename).bind(ext).bind(size).bind(mtime)
        .execute(&self.db).await?;
        
        let file_id = res.last_insert_rowid() as i32;

        // 2. 触发标签化 (Milestone 3 核心)
        let tag_mgr = TagManager::new(self.db.clone());
        let path_tagger = PathTagger::new(tag_mgr);
        path_tagger.process_path(file_id, &parent).await?;

        Ok(())
    }
}
```

### 5. 验证 Milestone 3

如果我们在测试目录 `./test_dir` 下建立如下结构：
`./test_dir/Work/Design/logo.png`

**运行扫描后，数据库中将发生：**
1. `tags` 表新增三行：`Work` (parent NULL), `Design` (parent Work)。
2. `files` 表新增一行：`logo.png`。
3. `file_tags` 表新增关联：`logo.png` 的 ID <-> `Design` 的 ID。

这标志着系统已经能够理解物理目录的语义，并将其扁平化为可检索的标签。

---

### 下一步建议

**Milestone 4：API 层与虚拟视图 (API & Virtual View)。**

现在数据已经在库里了，我们需要让用户看到它。
1. 使用 **Axum** 搭建 RESTful API。
2. 实现 `GET /api/v1/files`：支持通过标签 ID 过滤。
3. 实现 `GET /api/v1/tags/tree`：返回嵌套的标签树供前端展示。

**是否继续进行 Milestone 4 的开发？**