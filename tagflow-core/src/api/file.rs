use axum::{extract::{State, Query}, Json};
use sqlx::SqlitePool;
use crate::models::dto::{FileQuery, FileResponse, FileItem};
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
    let items: Vec<FileItem> = items.into_iter().map(|e| e.into()).collect();
    let total = items.len() as i64;

    Json(FileResponse { items, total })
}
