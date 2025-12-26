进入 **Milestone 4：API 层与虚拟视图 (API & Virtual View) 实现**。

在这一阶段，我们将使用 **Axum** 框架构建 RESTful 接口，将数据库中的元数据暴露给前端。核心挑战在于如何实现高效的**分面搜索（Faceted Search）**，即根据标签层级递归过滤文件。

### 1. 定义 DTO (数据传输对象)

在 `src/models/dto.rs` 中定义返回给前端的 JSON 结构。

```rust
use serde::{Deserialize, Serialize};
use crate::models::db::{FileEntry, Tag};

#[derive(Serialize)]
pub struct TagNode {
    pub id: i32,
    pub name: String,
    pub category: String,
    pub children: Vec<TagNode>,
}

#[derive(Serialize)]
pub struct FileResponse {
    pub items: Vec<FileEntry>,
    pub total: i64,
}

#[derive(Deserialize)]
pub struct FileQuery {
    pub tag_id: Option<i32>,      // 选中的标签ID
    pub recursive: Option<bool>,  // 是否包含子标签的文件
    pub page: Option<i64>,
    pub limit: Option<i64>,
}
```

### 2. 实现标签树逻辑 (api/tag.rs)

为了在侧边栏展示类似文件夹的树状结构，我们需要将扁平的 `tags` 表转换为嵌套对象。

```rust
use axum::{extract::State, Json};
use sqlx::SqlitePool;
use crate::models::dto::TagNode;
use crate::models::db::Tag;

pub async fn get_tag_tree(State(pool): State<SqlitePool>) -> Json<Vec<TagNode>> {
    // 1. 获取所有标签
    let tags: Vec<Tag> = sqlx::query_as("SELECT * FROM tags")
        .fetch_all(&pool)
        .await
        .unwrap_or_default();

    // 2. 在内存中构建树 (简单递归)
    let tree = build_tree(&tags, None);
    Json(tree)
}

fn build_tree(tags: &[Tag], parent_id: Option<i32>) -> Vec<TagNode> {
    tags.iter()
        .filter(|t| t.parent_id == parent_id)
        .map(|t| TagNode {
            id: t.id,
            name: t.name.clone(),
            category: t.category.clone(),
            children: build_tree(tags, Some(t.id)),
        })
        .collect()
}
```

### 3. 实现核心检索引擎 (api/file.rs)

这是 TagFlow 的“虚拟视图”核心。如果用户点击一个父标签并开启了 `recursive`，我们需要找到该标签及其所有子孙标签下的文件。

```rust
use axum::{extract::{State, Query}, Json};
use sqlx::SqlitePool;
use crate::models::dto::{FileQuery, FileResponse};
use crate::models::db::FileEntry;

pub async fn list_files(
    State(pool): State<SqlitePool>,
    Query(query): Query<FileQuery>,
) -> Json<FileResponse> {
    let limit = query.limit.unwrap_or(50);
    let offset = (query.page.unwrap_or(1) - 1) * limit;

    let items = if let Some(tag_id) = query.tag_id {
        if query.recursive.unwrap_or(true) {
            // 使用递归 CTE 查找所有子孙标签的文件
            sqlx::query_as::<_, FileEntry>(
                r#"
                WITH RECURSIVE sub_tags(id) AS (
                    SELECT id FROM tags WHERE id = ?
                    UNION ALL
                    SELECT t.id FROM tags t JOIN sub_tags st ON t.parent_id = st.id
                )
                SELECT DISTINCT f.* FROM files f
                JOIN file_tags ft ON f.id = ft.file_id
                WHERE ft.tag_id IN (SELECT id FROM sub_tags)
                ORDER BY f.mtime DESC LIMIT ? OFFSET ?
                "#,
            )
            .bind(tag_id).bind(limit).bind(offset)
            .fetch_all(&pool).await
        } else {
            // 仅查找直接关联该标签的文件
            sqlx::query_as::<_, FileEntry>(
                "SELECT f.* FROM files f JOIN file_tags ft ON f.id = ft.file_id WHERE ft.tag_id = ? ORDER BY f.mtime DESC LIMIT ? OFFSET ?"
            )
            .bind(tag_id).bind(limit).bind(offset)
            .fetch_all(&pool).await
        }
    } else {
        // 无过滤条件，返回所有
        sqlx::query_as::<_, FileEntry>("SELECT * FROM files ORDER BY mtime DESC LIMIT ? OFFSET ?")
            .bind(limit).bind(offset)
            .fetch_all(&pool).await
    };

    let items = items.unwrap_or_default();
    let total = items.len() as i64; // 简化处理，实际应另行查询 COUNT

    Json(FileResponse { items, total })
}
```

### 4. 组装 Axum 路由 (main.rs)

将所有的 Handler 挂载到路由上，并共享数据库连接池。

```rust
mod api;
mod models;
mod infra;
mod core;
mod engine;

use axum::{routing::get, Router};
use std::net::SocketAddr;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // ... 数据库初始化 (参考 Milestone 1) ...
    let pool = infra::db::init_db("sqlite:tagflow.db?mode=rwc").await?;

    // 构建路由
    let app = Router::new()
        .route("/api/v1/tags/tree", get(api::tag::get_tag_tree))
        .route("/api/v1/files", get(api::file::list_files))
        // 这里的 State 会被自动注入到 Handler 中
        .with_state(pool);

    // 启动服务器
    let addr = SocketAddr::from(([0, 0, 0, 0], 8080));
    tracing::info!("服务器运行在 http://{}", addr);
    
    let listener = tokio::net::TcpListener::bind(addr).await?;
    axum::serve(listener, app).await?;

    Ok(())
}
```

### 5. Milestone 4 交付点

1.  **标签树 API：** 前端可以一次性获取嵌套的标签结构，通过递归渲染出侧边栏。
2.  **分面搜索：** 利用 SQL 的递归 CTE (Common Table Expression)，实现了“父标签穿透”查询，这是虚拟目录体验的关键。
3.  **高性能查询：** 所有过滤逻辑都在数据库层完成，避免了内存中处理大量文件数据。

---

### 下一步建议

**Milestone 5：前端基础架构与虚拟滚动视图。**

后端已经能够提供数据，现在我们需要一个现代化的 UI 来展示它：
1. 使用 **Vue 3 + Vite** 初始化项目。
2. 实现一个**左侧标签树 + 右侧文件网格**的布局。
3. 关键挑战：实现**虚拟滚动网格 (Virtual Grid)**，确保在展示成千上万个文件时依然流畅。

**是否继续进行 Milestone 5（前端开发）？**