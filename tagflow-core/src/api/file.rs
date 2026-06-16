use crate::infra::storage::StorageManager;
use crate::models::db::{FileEntry, Library};
use crate::models::dto::{FileDetail, FileItem, FileQuery, FileResponse, FileTagInfo};
use axum::{
    Json,
    body::Body,
    extract::{Path, Query, State},
    http::{HeaderMap, StatusCode, header},
    response::Response,
};
use percent_encoding::{NON_ALPHANUMERIC, utf8_percent_encode};
use serde::Deserialize;
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
    let total = q.bind(n).fetch_one(pool).await.map_err(|e| {
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
    let total = q.bind(n).fetch_one(pool).await.map_err(|e| {
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

// ========== 文件内容 / 详情端点 ==========

/// 内容端点查询参数。
#[derive(Deserialize)]
pub struct ContentQuery {
    /// `download=1` 触发下载（追加 `Content-Disposition: attachment`）。
    #[serde(default)]
    pub download: Option<String>,
}

/// 推断常见媒体类型的 Content-Type（基于扩展名，小写匹配）。
/// 未知扩展名或无扩展名回退 `application/octet-stream`。
fn content_type_for(extension: &Option<String>) -> &'static str {
    match extension
        .as_deref()
        .map(|e| e.to_ascii_lowercase())
        .as_deref()
    {
        Some("jpg") | Some("jpeg") => "image/jpeg",
        Some("png") => "image/png",
        Some("gif") => "image/gif",
        Some("webp") => "image/webp",
        Some("bmp") => "image/bmp",
        Some("svg") => "image/svg+xml",
        Some("mp4") => "video/mp4",
        Some("webm") => "video/webm",
        Some("mkv") => "video/x-matroska",
        Some("mov") => "video/quicktime",
        Some("avi") => "video/x-msvideo",
        Some("mp3") => "audio/mpeg",
        Some("wav") => "audio/wav",
        Some("ogg") => "audio/ogg",
        Some("flac") => "audio/flac",
        Some("aac") => "audio/aac",
        Some("m4a") => "audio/mp4",
        Some("pdf") => "application/pdf",
        _ => "application/octet-stream",
    }
}

/// 判断是否为文本类扩展名（需后端转码并以 `text/plain` 返回）。
/// 涵盖纯文本、标记、配置、常见代码（代码不做语法高亮，仅纯文本展示）。
fn is_text_extension(extension: &Option<String>) -> bool {
    matches!(
        extension
            .as_deref()
            .map(|e| e.to_ascii_lowercase())
            .as_deref(),
        Some(
            "txt"
                | "md"
                | "markdown"
                | "log"
                | "csv"
                | "tsv"
                | "json"
                | "xml"
                | "html"
                | "htm"
                | "yaml"
                | "yml"
                | "ini"
                | "conf"
                | "toml"
                | "js"
                | "ts"
                | "css"
                | "scss"
                | "py"
                | "rs"
                | "go"
                | "java"
                | "c"
                | "cpp"
                | "h"
                | "hpp"
                | "sh"
                | "bat"
                | "sql"
                | "vue"
                | "srt"
                | "vtt"
        )
    )
}

/// 将原始字节解码为 UTF-8 字符串。
///
/// 顺序：UTF-8 BOM 去头 → UTF-16 BOM 用 encoding_rs → 严格 UTF-8 → 失败回退 GBK
/// （覆盖 GBK/GB18030 编码的中文小说）。
fn decode_text(bytes: Vec<u8>) -> String {
    if bytes.starts_with(&[0xEF, 0xBB, 0xBF]) {
        return String::from_utf8_lossy(&bytes[3..]).into_owned();
    }
    if bytes.starts_with(&[0xFF, 0xFE]) {
        return encoding_rs::UTF_16LE.decode(&bytes).0.into_owned();
    }
    if bytes.starts_with(&[0xFE, 0xFF]) {
        return encoding_rs::UTF_16BE.decode(&bytes).0.into_owned();
    }
    match String::from_utf8(bytes) {
        Ok(s) => s,
        Err(e) => encoding_rs::GBK.decode(&e.into_bytes()).0.into_owned(),
    }
}

/// 解析 `Range: bytes=start-end`（end 可省略，表示到文件末尾）。
///
/// 返回 `(start, end_inclusive)`；越界或格式非法返回 `None`（调用方按无 Range 全量响应）。
fn parse_range(range_header: Option<&str>, total: u64) -> Option<(u64, u64)> {
    let h = range_header?.strip_prefix("bytes=")?;
    let mut parts = h.splitn(2, '-');
    let start: u64 = parts.next()?.trim().parse().ok()?;
    let end_str = parts.next().unwrap_or("").trim();
    if total == 0 || start >= total {
        return None;
    }
    let end: u64 = if end_str.is_empty() {
        total - 1
    } else {
        end_str.parse().ok()?
    };
    if start > end {
        return None;
    }
    Some((start, end.min(total - 1)))
}

/// 构造下载用的 `Content-Disposition: attachment; filename*=UTF-8''<pct>` 头。
///
/// 百分号编码确保任意文件名（含中文）合法且不含 CRLF（HeaderValue 安全）。
fn download_disposition(filename: &str) -> axum::http::HeaderValue {
    let encoded = utf8_percent_encode(filename, NON_ALPHANUMERIC);
    let value = format!("attachment; filename*=UTF-8''{}", encoded);
    axum::http::HeaderValue::from_str(&value)
        .expect("percent-encoded filename is always valid HeaderValue")
}

/// 文件内容端点
///
/// # 路由
/// GET /api/v1/files/:id/content[?download=1]
///
/// 行为：按扩展名分流——
/// - 文本类：全量读取 + 编码转码，返回 `text/plain; charset=utf-8`（不走 Range）
/// - 图片/视频/音频/PDF：原始字节流，支持 `Range`（206 Partial Content）
/// - 其他：`application/octet-stream`，支持 Range
///
/// `?download=1` 追加 `Content-Disposition: attachment`。
/// 鉴权：受 auth_middleware 保护；媒体 src 可走 `?token=<jwt>` 兜底。
pub async fn get_content(
    State(pool): State<SqlitePool>,
    Path(id): Path<i32>,
    Query(params): Query<ContentQuery>,
    headers: HeaderMap,
) -> Result<Response, StatusCode> {
    // 1. 解析文件行（file_id 走 DB，防目录遍历）
    let file = sqlx::query_as::<_, FileEntry>("SELECT * FROM files WHERE id = ? AND status = 1")
        .bind(id)
        .fetch_optional(&pool)
        .await
        .map_err(|e| {
            error!("查询文件失败 id={}: {}", id, e);
            StatusCode::INTERNAL_SERVER_ERROR
        })?
        .ok_or(StatusCode::NOT_FOUND)?;

    // 2. 解析所属资源库
    let library = sqlx::query_as::<_, Library>("SELECT * FROM libraries WHERE id = ?")
        .bind(file.library_id)
        .fetch_optional(&pool)
        .await
        .map_err(|e| {
            error!("查询资源库失败 library_id={}: {}", file.library_id, e);
            StatusCode::INTERNAL_SERVER_ERROR
        })?
        .ok_or(StatusCode::NOT_FOUND)?;

    // 3. OpenDAL operator + 库内相对路径
    let op = StorageManager::get_operator(&library).map_err(|e| {
        error!("存储初始化失败 library={}: {}", library.name, e);
        StatusCode::INTERNAL_SERVER_ERROR
    })?;
    let rel_path = if file.parent_path.is_empty() {
        file.filename.clone()
    } else {
        format!(
            "{}/{}",
            file.parent_path.trim_end_matches('/'),
            file.filename
        )
    };

    let is_download = params.download.as_deref() == Some("1");
    let total = file.size.max(0) as u64;

    // 4. 文本类：全量读 + 转码
    if is_text_extension(&file.extension) {
        let bytes = op.read(&rel_path).await.map_err(|e| {
            error!("读取文本失败 path={}: {}", rel_path, e);
            StatusCode::INTERNAL_SERVER_ERROR
        })?;
        let body = Body::from(decode_text(bytes.to_vec()));
        let mut builder = Response::builder()
            .status(StatusCode::OK)
            .header(header::CONTENT_TYPE, "text/plain; charset=utf-8")
            .header(header::ACCEPT_RANGES, "bytes");
        if is_download {
            builder = builder.header(
                header::CONTENT_DISPOSITION,
                download_disposition(&file.filename),
            );
        }
        return Ok(builder.body(body).unwrap());
    }

    // 5. 媒体/二进制：支持 Range
    let content_type = content_type_for(&file.extension);
    let range = parse_range(
        headers.get(header::RANGE).and_then(|v| v.to_str().ok()),
        total,
    );

    let mut builder = Response::builder()
        .header(header::CONTENT_TYPE, content_type)
        .header(header::ACCEPT_RANGES, "bytes");
    if is_download {
        builder = builder.header(
            header::CONTENT_DISPOSITION,
            download_disposition(&file.filename),
        );
    }

    match range {
        Some((start, end)) => {
            // OpenDAL range 接受 RangeBounds，用 inclusive 读取 [start, end] 字节
            let bytes = op
                .read_with(&rel_path)
                .range(start..=end)
                .await
                .map_err(|e| {
                    error!(
                        "Range 读取失败 path={} range={}-{}: {}",
                        rel_path, start, end, e
                    );
                    StatusCode::INTERNAL_SERVER_ERROR
                })?;
            let len = end - start + 1;
            builder
                .status(StatusCode::PARTIAL_CONTENT)
                .header(
                    header::CONTENT_RANGE,
                    format!("bytes {}-{}/{}", start, end, total),
                )
                .header(header::CONTENT_LENGTH, len.to_string())
                .body(Body::from(bytes.to_vec()))
                .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)
        }
        None => {
            // 无 Range：全量读（图片等小文件路径；大视频浏览器会带 Range）
            let bytes = op.read(&rel_path).await.map_err(|e| {
                error!("读取文件失败 path={}: {}", rel_path, e);
                StatusCode::INTERNAL_SERVER_ERROR
            })?;
            builder
                .status(StatusCode::OK)
                .header(header::CONTENT_LENGTH, bytes.len().to_string())
                .body(Body::from(bytes.to_vec()))
                .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)
        }
    }
}

/// 文件详情端点
///
/// # 路由
/// GET /api/v1/files/:id
///
/// 返回完整元数据 + 该文件全部标签（按 category 分组展示）。
pub async fn get_file_detail(
    State(pool): State<SqlitePool>,
    Path(id): Path<i32>,
) -> Result<Json<FileDetail>, StatusCode> {
    let file = sqlx::query_as::<_, FileEntry>("SELECT * FROM files WHERE id = ? AND status = 1")
        .bind(id)
        .fetch_optional(&pool)
        .await
        .map_err(|e| {
            error!("查询文件详情失败 id={}: {}", id, e);
            StatusCode::INTERNAL_SERVER_ERROR
        })?
        .ok_or(StatusCode::NOT_FOUND)?;

    #[derive(sqlx::FromRow)]
    struct TagRow {
        id: i32,
        name: String,
        category: String,
    }
    let rows = sqlx::query_as::<_, TagRow>(
        r#"SELECT t.id, t.name, t.category
           FROM file_tags ft JOIN tags t ON ft.tag_id = t.id
           WHERE ft.file_id = ?
           ORDER BY t.category, t.name"#,
    )
    .bind(id)
    .fetch_all(&pool)
    .await
    .map_err(|e| {
        error!("查询文件标签失败 file_id={}: {}", id, e);
        StatusCode::INTERNAL_SERVER_ERROR
    })?;

    let tags = rows
        .into_iter()
        .map(|r| FileTagInfo {
            id: r.id,
            name: r.name,
            category: r.category,
        })
        .collect();

    Ok(Json(FileDetail {
        id: file.id,
        filename: file.filename,
        extension: file.extension,
        size: file.size,
        mtime: file.mtime,
        parent_path: file.parent_path,
        tags,
    }))
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
        sqlx::query(
            "INSERT OR IGNORE INTO file_tags (file_id, tag_id, source) VALUES (?, ?, 'auto')",
        )
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
        let (items, total) = query_files_by_tags_recursive(&pool, &[year], 50, 0)
            .await
            .unwrap();
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
        let (items, total) = query_files_by_tags_direct(&pool, &[txt, text], 50, 0)
            .await
            .unwrap();
        assert_eq!(total, 1);
        assert_eq!(items[0].filename, "a.txt");

        // 单个 text → f1, f3
        let (_, total) = query_files_by_tags_direct(&pool, &[text], 50, 0)
            .await
            .unwrap();
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
        let (items, total) = query_files_by_tags_direct(&pool, &[txt, mp4], 50, 0)
            .await
            .unwrap();
        assert_eq!(total, 0);
        assert!(items.is_empty());
    }

    #[test]
    fn parse_range_handles_open_and_closed() {
        assert_eq!(parse_range(Some("bytes=0-"), 1000), Some((0, 999)));
        assert_eq!(parse_range(Some("bytes=100-200"), 1000), Some((100, 200)));
        assert_eq!(parse_range(Some("bytes=900-"), 1000), Some((900, 999)));
        // 越界
        assert_eq!(parse_range(Some("bytes=1000-"), 1000), None);
        assert_eq!(parse_range(Some("bytes=1500-"), 1000), None);
        // end 超过 total → clamp 到末尾
        assert_eq!(parse_range(Some("bytes=950-2000"), 1000), Some((950, 999)));
        // start > end 非法
        assert_eq!(parse_range(Some("bytes=500-100"), 1000), None);
        // 无 header / 非 bytes 前缀
        assert_eq!(parse_range(None, 1000), None);
        assert_eq!(parse_range(Some("items=0-10"), 1000), None);
        // total = 0 无有效区间
        assert_eq!(parse_range(Some("bytes=0-"), 0), None);
    }

    #[test]
    fn decode_text_utf8_passthrough() {
        assert_eq!(decode_text("你好".as_bytes().to_vec()), "你好");
    }

    #[test]
    fn decode_text_gbk_fallback() {
        // GBK 编码的 "中文"：D6 D0 CE C4（非合法 UTF-8，应回退 GBK 解码）
        let gbk = vec![0xD6, 0xD0, 0xCE, 0xC4];
        assert_eq!(decode_text(gbk), "中文");
    }

    #[test]
    fn decode_text_strips_utf8_bom() {
        let mut b = vec![0xEF, 0xBB, 0xBF];
        b.extend_from_slice("hello".as_bytes());
        assert_eq!(decode_text(b), "hello");
    }

    #[test]
    fn text_extension_detection_case_insensitive() {
        assert!(is_text_extension(&Some("txt".into())));
        assert!(is_text_extension(&Some("MD".into())));
        assert!(is_text_extension(&Some("Csv".into())));
        assert!(!is_text_extension(&Some("mp4".into())));
        assert!(!is_text_extension(&Some("pdf".into())));
        assert!(!is_text_extension(&None));
    }

    #[test]
    fn content_type_inference_known_extensions() {
        assert_eq!(content_type_for(&Some("mp4".into())), "video/mp4");
        assert_eq!(content_type_for(&Some("jpg".into())), "image/jpeg");
        assert_eq!(content_type_for(&Some("jpeg".into())), "image/jpeg");
        assert_eq!(content_type_for(&Some("PDF".into())), "application/pdf");
        assert_eq!(content_type_for(&None), "application/octet-stream");
        assert_eq!(
            content_type_for(&Some("xyz".into())),
            "application/octet-stream"
        );
    }

    #[test]
    fn download_disposition_encodes_and_is_crlf_safe() {
        let v = download_disposition("小说.txt");
        let s = v.to_str().unwrap();
        assert!(s.starts_with("attachment; filename*=UTF-8''"));
        assert!(!s.contains('\n'));
        assert!(!s.contains('\r'));
        // 中文应被百分号编码，整串 ASCII 合法
        assert!(s.is_ascii());
    }
}
