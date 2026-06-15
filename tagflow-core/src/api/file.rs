use crate::models::db::FileEntry;
use crate::models::dto::{FileItem, FileQuery, FileResponse};
use axum::{
    Json,
    body::Body,
    extract::{Path, Query, State},
    http::{StatusCode, header},
    response::Response,
};
use sqlx::SqlitePool;
use tokio::fs::File;
use tokio_util::io::ReaderStream;
use tracing::error;

pub async fn list_files(
    State(pool): State<SqlitePool>,
    Query(query): Query<FileQuery>,
) -> Result<Json<FileResponse>, StatusCode> {
    let limit = query.limit.unwrap_or(50);
    let offset = (query.page.unwrap_or(1) - 1) * limit;
    let recursive = query.recursive.unwrap_or(true);

    // 合并 tag_ids 与旧版 tag_id，去重保序
    let mut tag_ids: Vec<i32> = Vec::new();
    for id in query.tag_ids.into_iter().chain(query.tag_id.into_iter()) {
        if !tag_ids.contains(&id) {
            tag_ids.push(id);
        }
    }

    let (items, total) = if tag_ids.is_empty() {
        query_all_files(&pool, limit, offset).await?
    } else if recursive {
        query_files_by_tags_recursive(&pool, &tag_ids, limit, offset).await?
    } else {
        query_files_by_tags_direct(&pool, &tag_ids, limit, offset).await?
    };

    let items: Vec<FileItem> = items.into_iter().map(|e| e.into()).collect();
    Ok(Json(FileResponse { items, total }))
}

/// 生成 `n` 个 `?` 用逗号连接，用于动态 `IN (...)` 占位符。
/// 仅拼接占位符字面量，所有值仍走 `.bind()`，无注入风险。
fn placeholders(n: usize) -> String {
    std::iter::repeat_n("?", n).collect::<Vec<_>>().join(", ")
}

async fn query_all_files(
    pool: &SqlitePool,
    limit: i64,
    offset: i64,
) -> Result<(Vec<FileEntry>, i64), StatusCode> {
    let items = sqlx::query_as::<_, FileEntry>(
        "SELECT * FROM files WHERE status = 1 ORDER BY mtime DESC LIMIT ? OFFSET ?",
    )
    .bind(limit)
    .bind(offset)
    .fetch_all(pool)
    .await
    .map_err(|e| {
        error!("查询文件列表失败: {}", e);
        StatusCode::INTERNAL_SERVER_ERROR
    })?;

    let total = sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM files WHERE status = 1")
        .fetch_one(pool)
        .await
        .map_err(|e| {
            error!("统计文件总数失败: {}", e);
            StatusCode::INTERNAL_SERVER_ERROR
        })?;

    Ok((items, total))
}

/// 多标签 AND（递归展开子树）。文件须同时命中所有 tag 的子树。
async fn query_files_by_tags_recursive(
    pool: &SqlitePool,
    tag_ids: &[i32],
    limit: i64,
    offset: i64,
) -> Result<(Vec<FileEntry>, i64), StatusCode> {
    let ph = placeholders(tag_ids.len());
    let n = tag_ids.len() as i64;

    let items_sql = format!(
        r#"
        WITH RECURSIVE sub_tags(tag_id, root_id) AS (
            SELECT id, id FROM tags WHERE id IN ({ph})
            UNION ALL
            SELECT t.id, st.root_id FROM tags t
            JOIN sub_tags st ON t.parent_id = st.tag_id
        )
        SELECT f.* FROM files f
        WHERE f.status = 1 AND f.id IN (
            SELECT ft.file_id FROM file_tags ft
            JOIN sub_tags st ON ft.tag_id = st.tag_id
            GROUP BY ft.file_id
            HAVING COUNT(DISTINCT st.root_id) = ?
        )
        ORDER BY f.mtime DESC LIMIT ? OFFSET ?
        "#
    );
    let mut q = sqlx::query_as::<_, FileEntry>(&items_sql);
    for id in tag_ids {
        q = q.bind(id);
    }
    let items = q
        .bind(n)
        .bind(limit)
        .bind(offset)
        .fetch_all(pool)
        .await
        .map_err(|e| {
            error!("多标签递归查询失败: {}", e);
            StatusCode::INTERNAL_SERVER_ERROR
        })?;

    let count_sql = format!(
        r#"
        WITH RECURSIVE sub_tags(tag_id, root_id) AS (
            SELECT id, id FROM tags WHERE id IN ({ph})
            UNION ALL
            SELECT t.id, st.root_id FROM tags t
            JOIN sub_tags st ON t.parent_id = st.tag_id
        )
        SELECT COUNT(*) FROM files f
        WHERE f.status = 1 AND f.id IN (
            SELECT ft.file_id FROM file_tags ft
            JOIN sub_tags st ON ft.tag_id = st.tag_id
            GROUP BY ft.file_id
            HAVING COUNT(DISTINCT st.root_id) = ?
        )
        "#
    );
    let mut q = sqlx::query_scalar::<_, i64>(&count_sql);
    for id in tag_ids {
        q = q.bind(id);
    }
    let total = q
        .bind(n)
        .fetch_one(pool)
        .await
        .map_err(|e| {
            error!("多标签递归计数失败: {}", e);
            StatusCode::INTERNAL_SERVER_ERROR
        })?;

    Ok((items, total))
}

/// 多标签 AND（不展开子树，仅精确匹配 tag）。
async fn query_files_by_tags_direct(
    pool: &SqlitePool,
    tag_ids: &[i32],
    limit: i64,
    offset: i64,
) -> Result<(Vec<FileEntry>, i64), StatusCode> {
    let ph = placeholders(tag_ids.len());
    let n = tag_ids.len() as i64;

    let items_sql = format!(
        r#"
        SELECT f.* FROM files f
        WHERE f.status = 1 AND f.id IN (
            SELECT ft.file_id FROM file_tags ft
            WHERE ft.tag_id IN ({ph})
            GROUP BY ft.file_id
            HAVING COUNT(DISTINCT ft.tag_id) = ?
        )
        ORDER BY f.mtime DESC LIMIT ? OFFSET ?
        "#
    );
    let mut q = sqlx::query_as::<_, FileEntry>(&items_sql);
    for id in tag_ids {
        q = q.bind(id);
    }
    let items = q
        .bind(n)
        .bind(limit)
        .bind(offset)
        .fetch_all(pool)
        .await
        .map_err(|e| {
            error!("多标签直接查询失败: {}", e);
            StatusCode::INTERNAL_SERVER_ERROR
        })?;

    let count_sql = format!(
        r#"
        SELECT COUNT(*) FROM files f
        WHERE f.status = 1 AND f.id IN (
            SELECT ft.file_id FROM file_tags ft
            WHERE ft.tag_id IN ({ph})
            GROUP BY ft.file_id
            HAVING COUNT(DISTINCT ft.tag_id) = ?
        )
        "#
    );
    let mut q = sqlx::query_scalar::<_, i64>(&count_sql);
    for id in tag_ids {
        q = q.bind(id);
    }
    let total = q
        .bind(n)
        .fetch_one(pool)
        .await
        .map_err(|e| {
            error!("多标签直接计数失败: {}", e);
            StatusCode::INTERNAL_SERVER_ERROR
        })?;

    Ok((items, total))
}

/// 获取文件缩略图
///
/// # 路由
/// GET /api/v1/files/:id/thumbnail
///
/// # 成功响应 (200)
/// 返回 WebP 格式的缩略图图片
///
/// # 失败响应
/// - 404: 缩略图不存在
pub async fn get_thumbnail(Path(id): Path<i32>) -> Result<Response, StatusCode> {
    let cache_dir = crate::infra::config::cache_dir();
    let thumbnail_path = format!("{}/{}.webp", cache_dir, id);

    match File::open(&thumbnail_path).await {
        Ok(file) => {
            let stream = ReaderStream::new(file);
            let body = Body::from_stream(stream);

            Ok(Response::builder()
                .status(StatusCode::OK)
                .header(header::CONTENT_TYPE, "image/webp")
                .header(header::CACHE_CONTROL, "public, max-age=86400")
                .body(body)
                .unwrap())
        }
        Err(_) => Err(StatusCode::NOT_FOUND),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn placeholders_builds_correct_count() {
        assert_eq!(placeholders(1), "?");
        assert_eq!(placeholders(3), "?, ?, ?");
        assert_eq!(placeholders(0), "");
    }

    /// 构造单连接内存库（保证 schema 在同连接内可见），建表 + 开外键。
    async fn setup_db() -> SqlitePool {
        let pool = sqlx::sqlite::SqlitePoolOptions::new()
            .max_connections(1)
            .connect("sqlite::memory:")
            .await
            .expect("connect in-memory");
        sqlx::query("PRAGMA foreign_keys = ON")
            .execute(&pool)
            .await
            .unwrap();
        for stmt in [
            "CREATE TABLE tags (id INTEGER PRIMARY KEY AUTOINCREMENT, name TEXT NOT NULL, category TEXT NOT NULL, parent_id INTEGER REFERENCES tags(id) ON DELETE CASCADE, UNIQUE(name, parent_id))",
            "CREATE TABLE files (id INTEGER PRIMARY KEY AUTOINCREMENT, library_id INTEGER NOT NULL, parent_path TEXT NOT NULL, filename TEXT NOT NULL, extension TEXT, size INTEGER NOT NULL, mtime INTEGER NOT NULL, hash TEXT, status INTEGER DEFAULT 1, indexed_at DATETIME DEFAULT CURRENT_TIMESTAMP)",
            "CREATE TABLE file_tags (file_id INTEGER NOT NULL REFERENCES files(id) ON DELETE CASCADE, tag_id INTEGER NOT NULL REFERENCES tags(id) ON DELETE CASCADE, source TEXT DEFAULT 'auto', PRIMARY KEY(file_id, tag_id))",
        ] {
            sqlx::query(stmt).execute(&pool).await.unwrap();
        }
        pool
    }

    /// 插入一个标签并返回 id。
    async fn insert_tag(pool: &SqlitePool, name: &str, category: &str, parent: Option<i32>) -> i32 {
        sqlx::query("INSERT INTO tags (name, category, parent_id) VALUES (?, ?, ?)")
            .bind(name)
            .bind(category)
            .bind(parent)
            .execute(pool)
            .await
            .unwrap()
            .last_insert_rowid() as i32
    }

    /// 插入一个文件并返回 id。
    async fn insert_file(pool: &SqlitePool, filename: &str) -> i32 {
        sqlx::query("INSERT INTO files (library_id, parent_path, filename, size, mtime) VALUES (1, '', ?, 1, 0)")
            .bind(filename)
            .execute(pool)
            .await
            .unwrap()
            .last_insert_rowid() as i32
    }

    async fn link(pool: &SqlitePool, file_id: i32, tag_id: i32) {
        sqlx::query("INSERT OR IGNORE INTO file_tags (file_id, tag_id, source) VALUES (?, ?, 'auto')")
            .bind(file_id)
            .bind(tag_id)
            .execute(pool)
            .await
            .unwrap();
    }

    #[tokio::test]
    async fn single_tag_recursive_matches_descendant_tagged_files() {
        let pool = setup_db().await;
        // 层级：year 2024 -> month 2024-05
        let year = insert_tag(&pool, "2024", "time", None).await;
        let month = insert_tag(&pool, "2024-05", "time", Some(year)).await;
        // 文件只挂在 month（叶子）
        let f1 = insert_file(&pool, "a.txt").await;
        let f2 = insert_file(&pool, "b.txt").await;
        link(&pool, f1, month).await;
        link(&pool, f2, month).await;
        // 另一个无标签文件
        insert_file(&pool, "c.txt").await;

        // 查 year：递归应命中挂在 month 上的两个文件
        let (items, total) = query_files_by_tags_recursive(&pool, &[year], 50, 0).await.unwrap();
        assert_eq!(total, 2);
        assert_eq!(items.len(), 2);
    }

    #[tokio::test]
    async fn multi_tag_and_returns_intersection() {
        let pool = setup_db().await;
        let txt = insert_tag(&pool, "txt", "ext", None).await;
        let text = insert_tag(&pool, "text", "type", None).await;
        let md = insert_tag(&pool, "md", "ext", None).await;

        // f1: txt + text（同时命中）
        let f1 = insert_file(&pool, "a.txt").await;
        link(&pool, f1, txt).await;
        link(&pool, f1, text).await;
        // f2: 只有 txt
        let f2 = insert_file(&pool, "b.txt").await;
        link(&pool, f2, txt).await;
        // f3: md + text
        let f3 = insert_file(&pool, "c.md").await;
        link(&pool, f3, md).await;
        link(&pool, f3, text).await;

        // txt AND text → 只有 f1
        let (items, total) = query_files_by_tags_direct(&pool, &[txt, text], 50, 0).await.unwrap();
        assert_eq!(total, 1);
        assert_eq!(items[0].filename, "a.txt");

        // 单个 text → f1, f3
        let (_, total) = query_files_by_tags_direct(&pool, &[text], 50, 0).await.unwrap();
        assert_eq!(total, 2);
    }

    #[tokio::test]
    async fn multi_tag_and_empty_when_no_intersection() {
        let pool = setup_db().await;
        let txt = insert_tag(&pool, "txt", "ext", None).await;
        let mp4 = insert_tag(&pool, "mp4", "ext", None).await;
        let f1 = insert_file(&pool, "a.txt").await;
        link(&pool, f1, txt).await;

        // txt AND mp4 → 空（没有任何文件同时命中）
        let (items, total) = query_files_by_tags_direct(&pool, &[txt, mp4], 50, 0).await.unwrap();
        assert_eq!(total, 0);
        assert!(items.is_empty());
    }
}
