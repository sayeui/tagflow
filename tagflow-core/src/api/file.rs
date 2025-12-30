use axum::{
    body::Body,
    extract::{Path, Query, State},
    http::{header, StatusCode},
    response::Response,
    Json,
};
use sqlx::SqlitePool;
use tokio::fs::File;
use tokio_util::io::ReaderStream;
use crate::models::dto::{FileQuery, FileResponse, FileItem};
use crate::models::db::FileEntry;

pub async fn list_files(
    State(pool): State<SqlitePool>,
    Query(query): Query<FileQuery>,
) -> Json<FileResponse> {
    let limit = query.limit.unwrap_or(50);
    let offset = (query.page.unwrap_or(1) - 1) * limit;

    let items: Result<Vec<FileEntry>, _> = if let Some(tag_id) = query.tag_id {
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

/// 获取文件缩略图
///
/// # 路由
/// GET /api/v1/files/:id/thumbnail
///
/// # 路径参数
/// - `id`: 文件 ID
///
/// # 成功响应 (200)
/// 返回 WebP 格式的缩略图图片
///
/// # 失败响应
/// - 404: 缩略图不存在
pub async fn get_thumbnail(
    Path(id): Path<i32>,
) -> Result<Response, StatusCode> {
    // 缩略图缓存目录
    let cache_dir = "./cache";
    let thumbnail_path = format!("{}/{}.webp", cache_dir, id);

    // 检查缩略图文件是否存在
    match File::open(&thumbnail_path).await {
        Ok(file) => {
            // 获取文件 MIME 类型
            let stream = ReaderStream::new(file);
            let body = Body::from_stream(stream);

            Ok(Response::builder()
                .status(StatusCode::OK)
                .header(header::CONTENT_TYPE, "image/webp")
                .header(header::CACHE_CONTROL, "public, max-age=86400") // 缓存 24 小时
                .body(body)
                .unwrap())
        }
        Err(_) => {
            // 缩略图不存在，返回 404
            Err(StatusCode::NOT_FOUND)
        }
    }
}

