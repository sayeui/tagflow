use crate::core::tag::TagManager;
use crate::infra::storage::StorageManager;
use crate::models::db::{FileEntry, Library};
use crate::models::dto::{
    AddTagRequest, FileDetail, FileItem, FileQuery, FileResponse, FileTagInfo,
};
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
use tracing::{error, info, warn};

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

    // 文件名搜索：trim 后空串视为不过滤，保留原行为
    let keyword = query
        .keyword
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(|s| s.to_string());

    let (items, total) = if tag_ids.is_empty() {
        query_all_files(&pool, keyword.as_deref(), limit, offset).await?
    } else if recursive {
        query_files_by_tags_recursive(&pool, &tag_ids, keyword.as_deref(), limit, offset).await?
    } else {
        query_files_by_tags_direct(&pool, &tag_ids, keyword.as_deref(), limit, offset).await?
    };

    let items: Vec<FileItem> = items.into_iter().map(|e| e.into()).collect();
    Ok(Json(FileResponse { items, total }))
}

/// 生成 `n` 个 `?` 用逗号连接，用于动态 `IN (...)` 占位符。
/// 仅拼接占位符字面量，所有值仍走 `.bind()`，无注入风险。
fn placeholders(n: usize) -> String {
    std::iter::repeat_n("?", n).collect::<Vec<_>>().join(", ")
}

/// 构造 `filename LIKE` 的匹配模式（`%kw%`），并转义 LIKE 通配符与转义符本身。
///
/// 转义规则（ESCAPE '\'）：`\` → `\\`、`%` → `\%`、`_` → `\_`，
/// 之后用 `%` 包裹做子串匹配。调用方需在同一 SQL 中声明 `ESCAPE '\'`。
fn like_pattern(kw: &str) -> String {
    let mut out = String::with_capacity(kw.len() + 2);
    for ch in kw.chars() {
        match ch {
            '\\' | '%' | '_' => {
                out.push('\\');
                out.push(ch);
            }
            _ => out.push(ch),
        }
    }
    format!("%{}%", out)
}

async fn query_all_files(
    pool: &SqlitePool,
    keyword: Option<&str>,
    limit: i64,
    offset: i64,
) -> Result<(Vec<FileEntry>, i64), StatusCode> {
    let kw_clause = keyword.map(|_| " AND filename LIKE ? ESCAPE '\\'");
    let items_sql = format!(
        "SELECT * FROM files WHERE status = 1{kw} ORDER BY mtime DESC LIMIT ? OFFSET ?",
        kw = kw_clause.unwrap_or("")
    );
    let mut q = sqlx::query_as::<_, FileEntry>(&items_sql);
    if let Some(kw) = keyword {
        q = q.bind(like_pattern(kw));
    }
    let items = q
        .bind(limit)
        .bind(offset)
        .fetch_all(pool)
        .await
        .map_err(|e| {
            error!("查询文件列表失败: {}", e);
            StatusCode::INTERNAL_SERVER_ERROR
        })?;

    let count_sql = format!(
        "SELECT COUNT(*) FROM files WHERE status = 1{kw}",
        kw = kw_clause.unwrap_or("")
    );
    let mut q = sqlx::query_scalar::<_, i64>(&count_sql);
    if let Some(kw) = keyword {
        q = q.bind(like_pattern(kw));
    }
    let total = q.fetch_one(pool).await.map_err(|e| {
        error!("统计文件总数失败: {}", e);
        StatusCode::INTERNAL_SERVER_ERROR
    })?;

    Ok((items, total))
}

/// 多标签 AND（递归展开子树）。文件须同时命中所有 tag 的子树。
async fn query_files_by_tags_recursive(
    pool: &SqlitePool,
    tag_ids: &[i32],
    keyword: Option<&str>,
    limit: i64,
    offset: i64,
) -> Result<(Vec<FileEntry>, i64), StatusCode> {
    let ph = placeholders(tag_ids.len());
    let n = tag_ids.len() as i64;
    let kw_clause = keyword.map(|_| " AND f.filename LIKE ? ESCAPE '\\'");

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
        ){kw}
        ORDER BY f.mtime DESC LIMIT ? OFFSET ?
        "#,
        kw = kw_clause.unwrap_or("")
    );
    let mut q = sqlx::query_as::<_, FileEntry>(&items_sql);
    for id in tag_ids {
        q = q.bind(id);
    }
    q = q.bind(n);
    if let Some(kw) = keyword {
        q = q.bind(like_pattern(kw));
    }
    let items = q
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
        ){kw}
        "#,
        kw = kw_clause.unwrap_or("")
    );
    let mut q = sqlx::query_scalar::<_, i64>(&count_sql);
    for id in tag_ids {
        q = q.bind(id);
    }
    q = q.bind(n);
    if let Some(kw) = keyword {
        q = q.bind(like_pattern(kw));
    }
    let total = q.fetch_one(pool).await.map_err(|e| {
        error!("多标签递归计数失败: {}", e);
        StatusCode::INTERNAL_SERVER_ERROR
    })?;

    Ok((items, total))
}

/// 多标签 AND（不展开子树，仅精确匹配 tag）。
async fn query_files_by_tags_direct(
    pool: &SqlitePool,
    tag_ids: &[i32],
    keyword: Option<&str>,
    limit: i64,
    offset: i64,
) -> Result<(Vec<FileEntry>, i64), StatusCode> {
    let ph = placeholders(tag_ids.len());
    let n = tag_ids.len() as i64;
    let kw_clause = keyword.map(|_| " AND f.filename LIKE ? ESCAPE '\\'");

    let items_sql = format!(
        r#"
        SELECT f.* FROM files f
        WHERE f.status = 1 AND f.id IN (
            SELECT ft.file_id FROM file_tags ft
            WHERE ft.tag_id IN ({ph})
            GROUP BY ft.file_id
            HAVING COUNT(DISTINCT ft.tag_id) = ?
        ){kw}
        ORDER BY f.mtime DESC LIMIT ? OFFSET ?
        "#,
        kw = kw_clause.unwrap_or("")
    );
    let mut q = sqlx::query_as::<_, FileEntry>(&items_sql);
    for id in tag_ids {
        q = q.bind(id);
    }
    q = q.bind(n);
    if let Some(kw) = keyword {
        q = q.bind(like_pattern(kw));
    }
    let items = q
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
        ){kw}
        "#,
        kw = kw_clause.unwrap_or("")
    );
    let mut q = sqlx::query_scalar::<_, i64>(&count_sql);
    for id in tag_ids {
        q = q.bind(id);
    }
    q = q.bind(n);
    if let Some(kw) = keyword {
        q = q.bind(like_pattern(kw));
    }
    let total = q.fetch_one(pool).await.map_err(|e| {
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

/// 查询文件的全部标签（含 `source`），按 category、name 排序。
///
/// 供 `get_file_detail` / `add_file_tag` / `remove_file_tag` 共用，
/// 避免三处 JOIN 查询彼此漂移（参见 code-reuse-thinking-guide.md）。
async fn fetch_file_tags(pool: &SqlitePool, file_id: i32) -> Result<Vec<FileTagInfo>, StatusCode> {
    #[derive(sqlx::FromRow)]
    struct TagRow {
        id: i32,
        name: String,
        category: String,
        source: String,
    }
    let rows = sqlx::query_as::<_, TagRow>(
        r#"SELECT t.id, t.name, t.category, ft.source
           FROM file_tags ft JOIN tags t ON ft.tag_id = t.id
           WHERE ft.file_id = ?
           ORDER BY t.category, t.name"#,
    )
    .bind(file_id)
    .fetch_all(pool)
    .await
    .map_err(|e| {
        error!("查询文件标签失败 file_id={}: {}", file_id, e);
        StatusCode::INTERNAL_SERVER_ERROR
    })?;

    Ok(rows
        .into_iter()
        .map(|r| FileTagInfo {
            id: r.id,
            name: r.name,
            category: r.category,
            source: r.source,
        })
        .collect())
}

/// 文件详情端点
///
/// # 路由
/// GET /api/v1/files/:id
///
/// 返回完整元数据 + 该文件全部标签（按 category 分组展示，含 source）。
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

    let tags = fetch_file_tags(&pool, id).await?;

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

// ========== 手动标签写操作 ==========

/// 单个标签名长度上限（按字符计）。
const MAX_TAG_NAME_LEN: usize = 64;

/// 解析「/」分隔的标签路径为有序段：trim 后过滤空段。
///
/// 返回 `Some(parts)` 当且仅当存在至少一个非空段，且每段长度 ≤ 上限、
/// 不含控制字符；全空 / 超长 / 含非法字符返回 `None`（调用方映射为 400）。
fn parse_tag_path(path: &str) -> Option<Vec<String>> {
    let parts: Vec<String> = path
        .split('/')
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .collect();
    if parts.is_empty() {
        return None;
    }
    if parts
        .iter()
        .any(|s| s.chars().count() > MAX_TAG_NAME_LEN || s.chars().any(|c| c.is_control()))
    {
        return None;
    }
    Some(parts)
}

/// 确认文件存在且在线（status=1）；不存在返回 404。
async fn ensure_file_exists(pool: &SqlitePool, file_id: i32) -> Result<(), StatusCode> {
    let exists: Option<(i32,)> = sqlx::query_as("SELECT id FROM files WHERE id = ? AND status = 1")
        .bind(file_id)
        .fetch_optional(pool)
        .await
        .map_err(|e| {
            error!("查询文件存在性失败 file_id={}: {}", file_id, e);
            StatusCode::INTERNAL_SERVER_ERROR
        })?;
    if exists.is_none() {
        warn!("文件不存在或已离线: file_id={}", file_id);
        return Err(StatusCode::NOT_FOUND);
    }
    Ok(())
}

/// 按 `parts` 逐层建/复用 `category='user'` 标签节点，返回叶子 id。
///
/// 复用 [`TagManager::ensure_tag`]（SELECT-then-INSERT，正确处理 NULL parent_id 的 UNIQUE 语义）。
async fn ensure_user_tag_path(pool: &SqlitePool, parts: &[String]) -> Result<i32, StatusCode> {
    let tm = TagManager::new(pool.clone());
    let mut parent: Option<i32> = None;
    for part in parts {
        parent = Some(tm.ensure_tag(part, "user", parent).await.map_err(|e| {
            error!("建 user 标签失败 part={}: {}", part, e);
            StatusCode::INTERNAL_SERVER_ERROR
        })?);
    }
    Ok(parent.expect("parse_tag_path 已保证 parts 非空"))
}

/// 删除无引用且无子节点的标签，并向上递归清理因之变空的祖先。
///
/// 适用所有类别（path / type / time / user / ext 等）：COUNT(file_tags)=0 且
/// COUNT(子节点)=0 则删，并向父递归——既覆盖手动删除标签关联后的 `user` 空链，
/// 也覆盖删库后失去全部关联的 path/ext/type/time 孤儿节点。
///
/// best-effort：任何查询/删除失败记 error 后退出循环，不影响已删除的关联
/// （`unwrap_or(1)` 保守视为「仍有引用」停止清理，非 panic 降级安全）。
pub(crate) async fn cleanup_orphan_tag(pool: &SqlitePool, mut tag_id: i32) {
    loop {
        #[derive(sqlx::FromRow)]
        struct NodeRow {
            parent_id: Option<i32>,
        }
        let node = match sqlx::query_as::<_, NodeRow>("SELECT parent_id FROM tags WHERE id = ?")
            .bind(tag_id)
            .fetch_optional(pool)
            .await
        {
            Ok(n) => n,
            Err(e) => {
                error!("清理孤儿标签查询失败 tag_id={}: {}", tag_id, e);
                break;
            }
        };
        let Some(node) = node else { break };

        // 仍被文件引用或有子节点 → 不是孤儿，停止向上清理。
        // unwrap_or(1) 非 panic：查询失败时保守视为「仍有引用」，停止清理（降级安全）。
        let refs: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM file_tags WHERE tag_id = ?")
            .bind(tag_id)
            .fetch_one(pool)
            .await
            .unwrap_or(1);
        let kids: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM tags WHERE parent_id = ?")
            .bind(tag_id)
            .fetch_one(pool)
            .await
            .unwrap_or(1);
        if refs > 0 || kids > 0 {
            break;
        }

        let parent = node.parent_id;
        if let Err(e) = sqlx::query("DELETE FROM tags WHERE id = ?")
            .bind(tag_id)
            .execute(pool)
            .await
        {
            error!("删除空标签失败 tag_id={}: {}", tag_id, e);
            break;
        }
        info!("自动清理空标签: tag_id={}", tag_id);

        match parent {
            Some(pid) => tag_id = pid,
            None => break,
        }
    }
}

/// 移除文件的手动标签关联：校验来源为 `manual`（auto 返回 403），关联不存在返回 404。
///
/// 不做自动清理（由调用方按需触发 [`cleanup_orphan_tag`]）。
async fn remove_manual_link(
    pool: &SqlitePool,
    file_id: i32,
    tag_id: i32,
) -> Result<(), StatusCode> {
    #[derive(sqlx::FromRow)]
    struct LinkRow {
        source: String,
    }
    let link = sqlx::query_as::<_, LinkRow>(
        "SELECT source FROM file_tags WHERE file_id = ? AND tag_id = ?",
    )
    .bind(file_id)
    .bind(tag_id)
    .fetch_optional(pool)
    .await
    .map_err(|e| {
        error!(
            "查询标签关联失败 file_id={} tag_id={}: {}",
            file_id, tag_id, e
        );
        StatusCode::INTERNAL_SERVER_ERROR
    })?;

    match link {
        None => {
            warn!(
                "移除标签失败：关联不存在 file_id={} tag_id={}",
                file_id, tag_id
            );
            Err(StatusCode::NOT_FOUND)
        }
        Some(r) if r.source != "manual" => {
            warn!(
                "移除标签失败：自动标签不可删 file_id={} tag_id={}",
                file_id, tag_id
            );
            Err(StatusCode::FORBIDDEN)
        }
        Some(_) => {
            sqlx::query("DELETE FROM file_tags WHERE file_id = ? AND tag_id = ?")
                .bind(file_id)
                .bind(tag_id)
                .execute(pool)
                .await
                .map_err(|e| {
                    error!(
                        "删除标签关联失败 file_id={} tag_id={}: {}",
                        file_id, tag_id, e
                    );
                    StatusCode::INTERNAL_SERVER_ERROR
                })?;
            Ok(())
        }
    }
}

/// 添加手动标签
///
/// # 路由
/// POST /api/v1/files/:id/tags
///
/// # 请求体
/// `{ "path": "项目/TagFlow" }` —— `/` 分隔层级，逐层建/复用 user 节点。
///
/// # 成功响应 (200)
/// 返回该文件更新后的全部标签列表（含 source）。
///
/// # 失败响应
/// - 400: path 为空 / 含超长或非法段
/// - 404: 文件不存在或离线
/// - 500: 数据库错误
pub async fn add_file_tag(
    State(pool): State<SqlitePool>,
    Path(id): Path<i32>,
    Json(payload): Json<AddTagRequest>,
) -> Result<Json<Vec<FileTagInfo>>, StatusCode> {
    let Some(parts) = parse_tag_path(&payload.path) else {
        warn!(
            "添加标签失败：路径为空或非法 file_id={} path={:?}",
            id, payload.path
        );
        return Err(StatusCode::BAD_REQUEST);
    };

    ensure_file_exists(&pool, id).await?;
    let leaf = ensure_user_tag_path(&pool, &parts).await?;

    // 挂载 manual 关联（INSERT OR IGNORE，已存在则幂等）
    TagManager::new(pool.clone())
        .link_file_to_tag(id, leaf, "manual")
        .await
        .map_err(|e| {
            error!("挂载标签失败 file_id={} tag_id={}: {}", id, leaf, e);
            StatusCode::INTERNAL_SERVER_ERROR
        })?;

    info!("添加手动标签: file_id={}, path={}", id, payload.path);

    Ok(Json(fetch_file_tags(&pool, id).await?))
}

/// 移除手动标签
///
/// # 路由
/// DELETE /api/v1/files/:id/tags/:tag_id
///
/// # 成功响应 (200)
/// 返回该文件更新后的全部标签列表（含 source）。
///
/// # 失败响应
/// - 403: 该关联为自动标签（source=auto），受保护不可手动删除
/// - 404: 文件或关联不存在
/// - 500: 数据库错误
pub async fn remove_file_tag(
    State(pool): State<SqlitePool>,
    Path((id, tag_id)): Path<(i32, i32)>,
) -> Result<Json<Vec<FileTagInfo>>, StatusCode> {
    ensure_file_exists(&pool, id).await?;
    remove_manual_link(&pool, id, tag_id).await?;
    info!("移除手动标签: file_id={} tag_id={}", id, tag_id);
    // 删除关联后，向上递归清理空节点（best-effort，泛化到所有类别）
    cleanup_orphan_tag(&pool, tag_id).await;
    Ok(Json(fetch_file_tags(&pool, id).await?))
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

    #[test]
    fn like_pattern_wraps_and_escapes() {
        // 普通子串：两侧加 %
        assert_eq!(like_pattern("abc"), "%abc%");
        // 中文直接透传（无大小写问题）
        assert_eq!(like_pattern("报告"), "%报告%");
        // % 与 _ 转义
        assert_eq!(like_pattern("50%_off"), "%50\\%\\_off%");
        // 反斜杠自身转义
        assert_eq!(like_pattern(r"a\b"), r"%a\\b%");
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
        link_with_source(pool, file_id, tag_id, "auto").await;
    }

    /// 以指定来源挂载关联（auto/manual）；`link` 是 `source="auto"` 的快捷。
    async fn link_with_source(pool: &SqlitePool, file_id: i32, tag_id: i32, source: &str) {
        sqlx::query("INSERT OR IGNORE INTO file_tags (file_id, tag_id, source) VALUES (?, ?, ?)")
            .bind(file_id)
            .bind(tag_id)
            .bind(source)
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
        let (items, total) = query_files_by_tags_recursive(&pool, &[year], None, 50, 0)
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
        let (items, total) = query_files_by_tags_direct(&pool, &[txt, text], None, 50, 0)
            .await
            .unwrap();
        assert_eq!(total, 1);
        assert_eq!(items[0].filename, "a.txt");

        // 单个 text → f1, f3
        let (_, total) = query_files_by_tags_direct(&pool, &[text], None, 50, 0)
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
        let (items, total) = query_files_by_tags_direct(&pool, &[txt, mp4], None, 50, 0)
            .await
            .unwrap();
        assert_eq!(total, 0);
        assert!(items.is_empty());
    }

    #[tokio::test]
    async fn keyword_filters_by_filename_substring() {
        let pool = setup_db().await;
        insert_file(&pool, "report_2024.txt").await;
        insert_file(&pool, "REPORT_draft.txt").await; // 大小写不敏感
        insert_file(&pool, "notes.md").await;
        insert_file(&pool, "summary.txt").await;

        // "report" 命中两个（ASCII 不区分大小写）
        let (items, total) = query_all_files(&pool, Some("report"), 50, 0).await.unwrap();
        assert_eq!(total, 2);
        assert_eq!(items.len(), 2);
        let names: Vec<_> = items.iter().map(|f| f.filename.as_str()).collect();
        assert!(names.contains(&"report_2024.txt"));
        assert!(names.contains(&"REPORT_draft.txt"));

        // 无关键词 → 全部 4 个
        let (_, total) = query_all_files(&pool, None, 50, 0).await.unwrap();
        assert_eq!(total, 4);

        // 中文子串
        insert_file(&pool, "季度报告.docx").await;
        let (items, total) = query_all_files(&pool, Some("报告"), 50, 0).await.unwrap();
        assert_eq!(total, 1);
        assert_eq!(items[0].filename, "季度报告.docx");
    }

    #[tokio::test]
    async fn keyword_and_tags_combine() {
        let pool = setup_db().await;
        let txt = insert_tag(&pool, "txt", "ext", None).await;
        let f1 = insert_file(&pool, "alpha.txt").await;
        link(&pool, f1, txt).await;
        let f2 = insert_file(&pool, "alpha_beta.txt").await;
        link(&pool, f2, txt).await;
        let f3 = insert_file(&pool, "beta.txt").await;
        link(&pool, f3, txt).await;

        // tag=txt AND keyword=alpha → 2 个（alpha.txt、alpha_beta.txt）
        let (items, total) = query_files_by_tags_direct(&pool, &[txt], Some("alpha"), 50, 0)
            .await
            .unwrap();
        assert_eq!(total, 2);
        assert_eq!(items.len(), 2);
    }

    #[tokio::test]
    async fn keyword_escapes_like_wildcards() {
        let pool = setup_db().await;
        insert_file(&pool, "50%_off.txt").await; // 字面量含 % 与 _
        insert_file(&pool, "500off.txt").await; // 仅作为对照（不含下划线/百分号字面）
        insert_file(&pool, "normal.txt").await;

        // 精确搜索字面量 "50%_off"，应只命中那一个文件而非被当成通配符
        let (items, total) = query_all_files(&pool, Some("50%_off"), 50, 0)
            .await
            .unwrap();
        assert_eq!(total, 1);
        assert_eq!(items[0].filename, "50%_off.txt");
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

    // ========== 手动标签：parse_tag_path ==========

    #[test]
    fn parse_tag_path_splits_and_filters_empty_segments() {
        assert_eq!(parse_tag_path("a/b"), Some(vec!["a".into(), "b".into()]));
        // 前导/末尾/连续「/」与空白：空段被过滤
        assert_eq!(
            parse_tag_path(" a /// b "),
            Some(vec!["a".into(), "b".into()])
        );
        // 单段
        assert_eq!(parse_tag_path("收藏"), Some(vec!["收藏".into()]));
        // 全空
        assert_eq!(parse_tag_path(""), None);
        assert_eq!(parse_tag_path("   "), None);
        assert_eq!(parse_tag_path("/"), None);
        assert_eq!(parse_tag_path("///"), None);
        // 恰好上限通过，超长拒绝
        assert_eq!(parse_tag_path(&"x".repeat(64)), Some(vec!["x".repeat(64)]));
        assert_eq!(parse_tag_path(&"x".repeat(65)), None);
    }

    // ========== 手动标签：建层级 + 挂载 ==========

    #[tokio::test]
    async fn ensure_user_tag_path_builds_hierarchy() {
        let pool = setup_db().await;
        let leaf = ensure_user_tag_path(&pool, &parse_tag_path("项目/TagFlow").unwrap())
            .await
            .unwrap();
        // 项目(parent=NULL) → TagFlow，两个 user 节点
        let user_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM tags WHERE category='user'")
            .fetch_one(&pool)
            .await
            .unwrap();
        assert_eq!(user_count, 2);
        // TagFlow 的父应存在（指向「项目」）
        let parent_id: Option<i32> = sqlx::query_scalar("SELECT parent_id FROM tags WHERE id = ?")
            .bind(leaf)
            .fetch_one(&pool)
            .await
            .unwrap();
        assert!(parent_id.is_some());
    }

    #[tokio::test]
    async fn ensure_user_tag_path_reuses_existing_nodes() {
        let pool = setup_db().await;
        let parts = parse_tag_path("项目/TagFlow").unwrap();
        let leaf1 = ensure_user_tag_path(&pool, &parts).await.unwrap();
        // 同路径二次建：叶子 id 不变，不产生重复节点
        let leaf2 = ensure_user_tag_path(&pool, &parts).await.unwrap();
        assert_eq!(leaf1, leaf2);
        let user_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM tags WHERE category='user'")
            .fetch_one(&pool)
            .await
            .unwrap();
        assert_eq!(user_count, 2);
    }

    #[tokio::test]
    async fn add_manual_tag_links_with_manual_source() {
        let pool = setup_db().await;
        let f = insert_file(&pool, "a.txt").await;
        let leaf = ensure_user_tag_path(&pool, &parse_tag_path("项目/TagFlow").unwrap())
            .await
            .unwrap();
        TagManager::new(pool.clone())
            .link_file_to_tag(f, leaf, "manual")
            .await
            .unwrap();

        let tags = fetch_file_tags(&pool, f).await.unwrap();
        assert_eq!(tags.len(), 1);
        assert_eq!(tags[0].name, "TagFlow");
        assert_eq!(tags[0].category, "user");
        assert_eq!(tags[0].source, "manual");
    }

    // ========== 手动标签：移除 + 自动清理 ==========

    #[tokio::test]
    async fn remove_manual_link_rejects_auto_and_missing() {
        let pool = setup_db().await;
        let f = insert_file(&pool, "a.txt").await;
        // auto 关联（用 ext 标签模拟扫描器打的自动标签）
        let auto_tag = insert_tag(&pool, "txt", "ext", None).await;
        link(&pool, f, auto_tag).await; // source='auto'

        // auto 不可删 → 403
        assert_eq!(
            remove_manual_link(&pool, f, auto_tag).await,
            Err(StatusCode::FORBIDDEN)
        );
        // 不存在的关联 → 404
        assert_eq!(
            remove_manual_link(&pool, f, 999_999).await,
            Err(StatusCode::NOT_FOUND)
        );
    }

    #[tokio::test]
    async fn cleanup_keeps_node_with_other_refs_and_prunes_orphan_chain() {
        let pool = setup_db().await;
        let parts = parse_tag_path("项目/TagFlow").unwrap();
        let leaf = ensure_user_tag_path(&pool, &parts).await.unwrap();
        // 两个文件都挂到同一叶子
        let f1 = insert_file(&pool, "a.txt").await;
        let f2 = insert_file(&pool, "b.txt").await;
        let tm = TagManager::new(pool.clone());
        tm.link_file_to_tag(f1, leaf, "manual").await.unwrap();
        tm.link_file_to_tag(f2, leaf, "manual").await.unwrap();

        // 移除 f1 的关联：叶子仍有 f2 引用 → 不清理
        remove_manual_link(&pool, f1, leaf).await.unwrap();
        cleanup_orphan_tag(&pool, leaf).await;
        let leaf_alive: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM tags WHERE id = ?")
            .bind(leaf)
            .fetch_one(&pool)
            .await
            .unwrap();
        assert_eq!(leaf_alive, 1);

        // 再移除 f2 的关联：叶子无引用无子 → 删；父「项目」也无引用无子 → 递归删
        remove_manual_link(&pool, f2, leaf).await.unwrap();
        cleanup_orphan_tag(&pool, leaf).await;
        let user_left: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM tags WHERE category='user'")
            .fetch_one(&pool)
            .await
            .unwrap();
        assert_eq!(user_left, 0); // 父子链全清
    }

    #[tokio::test]
    async fn cleanup_also_handles_auto_categories() {
        let pool = setup_db().await;
        // 泛化后：非 user 类别（如 ext）失去引用后同样应被清理（删库场景的核心需求）。
        let ext_tag = insert_tag(&pool, "txt", "ext", None).await;
        // 无 file_tags 关联，无子节点 → 孤儿，应被删
        cleanup_orphan_tag(&pool, ext_tag).await;
        let alive: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM tags WHERE id = ?")
            .bind(ext_tag)
            .fetch_one(&pool)
            .await
            .unwrap();
        assert_eq!(alive, 0);
    }

    #[tokio::test]
    async fn cleanup_prunes_auto_chain_when_orphan() {
        let pool = setup_db().await;
        // path 层级：Projects(根) → 2024(子)，无文件关联 → 整条链都应被清
        let root = insert_tag(&pool, "Projects", "path", None).await;
        let child = insert_tag(&pool, "2024", "path", Some(root)).await;
        // 从叶子向上清
        cleanup_orphan_tag(&pool, child).await;
        let remaining: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM tags WHERE id IN (?, ?)")
            .bind(root)
            .bind(child)
            .fetch_one(&pool)
            .await
            .unwrap();
        assert_eq!(remaining, 0); // 父子链全清
    }

    #[tokio::test]
    async fn cleanup_keeps_auto_tag_when_still_referenced() {
        let pool = setup_db().await;
        // 即便泛化后，有关联的 auto 标签仍应保留（不会误删跨库共享 / 还有引用的标签）。
        let ext_tag = insert_tag(&pool, "txt", "ext", None).await;
        let f = insert_file(&pool, "a.txt").await;
        link(&pool, f, ext_tag).await;

        cleanup_orphan_tag(&pool, ext_tag).await;
        let alive: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM tags WHERE id = ?")
            .bind(ext_tag)
            .fetch_one(&pool)
            .await
            .unwrap();
        assert_eq!(alive, 1);
    }
}
