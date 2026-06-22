//! Library API - 资源库管理
//!
//! 提供资源库的 CRUD 操作、连接测试和扫描触发功能。

use axum::{
    Json,
    extract::{Path as AxumPath, State},
    http::StatusCode,
};
use sqlx::SqlitePool;
use tracing::{debug, error, info, warn};

use crate::engine::scanner::{release_scan_lock, try_acquire_scan_lock};
use crate::models::dto::{CreateLibraryRequest, LibraryResponse, TestConnectionResponse};

/// 验证路径安全性（防止路径遍历攻击）
///
/// # 规则
/// - 路径不能包含 `..` (父目录遍历)
/// - 路径不能包含 `./` 或 `.\` (当前目录引用)
/// - 路径必须是绝对路径
fn validate_path_security(path: &str) -> Result<(), &'static str> {
    // 检测路径遍历攻击
    if path.contains("..") {
        warn!("路径安全检查失败: 包含 '..' - {}", path);
        return Err("路径不能包含 '..'（路径遍历检测）");
    }

    if path.contains("./") || path.contains(".\\") {
        warn!("路径安全检查失败: 包含 './' 或 '.\\' - {}", path);
        return Err("路径不能包含 './' 或 '.\\'");
    }

    // 检查是否为绝对路径
    let is_unix_path = path.starts_with('/');
    let is_windows_path = path.len() >= 3
        && path.as_bytes()[0].is_ascii_alphabetic()
        && path.as_bytes()[1] == b':'
        && (path.as_bytes()[2] == b'\\' || path.as_bytes()[2] == b'/');

    if !is_unix_path && !is_windows_path {
        warn!("路径安全检查失败: 不是绝对路径 - {}", path);
        return Err("必须使用绝对路径（如 /mnt/data 或 C:\\Data）");
    }

    debug!("路径安全检查通过: {}", path);
    Ok(())
}

/// 校验本地路径在文件系统上的可达性：必须存在、是目录、可读。
///
/// 抽取自 `test_library_connection` 的核心校验逻辑，供 `create_library` 与
/// `test_library_connection` 共用，避免两处对 `exists`/`is_dir`/`read_dir`
/// 的判断彼此漂移（参见 code-reuse-thinking-guide.md「同一份输出由不对称机制
/// 产生」的预防要点）。
///
/// 返回 `Ok(())` 表示路径可访问；`Err(message)` 携带面向用户的中文提示。
fn validate_local_path_readable(base_path: &str) -> Result<(), String> {
    let path = std::path::Path::new(base_path);

    if !path.exists() {
        warn!("路径不存在: {}", base_path);
        return Err("路径不存在".to_string());
    }

    if !path.is_dir() {
        warn!("路径不是目录: {}", base_path);
        return Err("路径不是目录".to_string());
    }

    // 检查是否可读（read_dir 是判定目录可读的标准探针）
    if let Err(e) = std::fs::read_dir(path) {
        warn!("路径无权限: {} - {}", base_path, e);
        return Err("无权限访问此目录".to_string());
    }

    debug!("路径可达性校验通过: {}", base_path);
    Ok(())
}

/// 获取所有已配置的资源库
///
/// # 路由
/// GET /api/v1/libraries
///
/// # 成功响应 (200)
/// ```json
/// [
///   {
///     "id": 1,
///     "name": "我的照片",
///     "protocol": "local",
///     "base_path": "/mnt/photos",
///     "last_scanned_at": "2024-01-01T00:00:00Z",
///     "scan_interval_secs": 3600
///   }
/// ]
/// ```
pub async fn list_libraries(
    State(pool): State<SqlitePool>,
) -> Result<Json<Vec<LibraryResponse>>, StatusCode> {
    debug!("获取资源库列表");

    let libraries =
        sqlx::query_as::<_, crate::models::db::Library>("SELECT * FROM libraries ORDER BY id")
            .fetch_all(&pool)
            .await
            .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    // 全局扫描间隔（env TAGFLOW_SCAN_INTERVAL，clamp ≥60s），所有库共享同一值。
    // 前端据此推算「预计下次扫描」= last_scanned_at + scan_interval_secs。
    let scan_interval_secs = crate::infra::config::scan_interval_secs() as i64;
    let response: Vec<LibraryResponse> = libraries
        .into_iter()
        .map(|lib| LibraryResponse::from_library(lib, scan_interval_secs))
        .collect();

    info!("返回 {} 个资源库", response.len());
    Ok(Json(response))
}

/// 创建新的资源库
///
/// # 路由
/// POST /api/v1/libraries
///
/// # 请求体
/// ```json
/// {
///   "name": "我的照片",
///   "protocol": "local",
///   "base_path": "/mnt/photos",
///   "config_json": null
/// }
/// ```
///
/// # 成功响应 (201)
/// 无响应体
pub async fn create_library(
    State(pool): State<SqlitePool>,
    Json(payload): Json<CreateLibraryRequest>,
) -> Result<StatusCode, StatusCode> {
    info!(
        "创建资源库: name={}, protocol={}, path={}",
        payload.name, payload.protocol, payload.base_path
    );

    // 验证 protocol
    if payload.protocol != "local" && payload.protocol != "webdav" {
        warn!("无效的协议类型: {}", payload.protocol);
        return Err(StatusCode::BAD_REQUEST);
    }

    // 路径安全验证
    if let Err(err_msg) = validate_path_security(&payload.base_path) {
        warn!("路径安全验证失败: {} - {}", payload.base_path, err_msg);
        return Err(StatusCode::BAD_REQUEST);
    }

    // 路径可达性验证：避免错填路径后 OpenDAL Fs 自动建目录产生「幽灵空库」
    // （与「非侵入式」定位有张力）。仅 local 协议需要文件系统校验。
    // 注意：`validate_local_path_readable` 内部已对失败原因 `warn!`，此处不再重复
    // 打日志，避免同一拒绝在日志中出现两遍（参见 logging-guidelines.md）。
    if payload.protocol == "local" && validate_local_path_readable(&payload.base_path).is_err() {
        return Err(StatusCode::BAD_REQUEST);
    }

    sqlx::query(
        "INSERT INTO libraries (name, protocol, base_path, config_json)
         VALUES (?, ?, ?, ?)",
    )
    .bind(&payload.name)
    .bind(&payload.protocol)
    .bind(&payload.base_path)
    .bind(&payload.config_json)
    .execute(&pool)
    .await
    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    info!("资源库创建成功: {}", payload.name);
    Ok(StatusCode::CREATED)
}

/// 删除资源库
///
/// # 路由
/// DELETE /api/v1/libraries/:id
///
/// # 成功响应 (204)
/// 无响应体
///
/// # 失败响应
/// - 404: 资源库不存在
/// - 500: 服务器错误
///
/// # 标签清理
/// libraries 与 tags 表无外键关联，CASCADE 链（files/file_tags/tasks）不会清到
/// tags。删除前先收集该库文件关联过的 tag_ids，DELETE 后对每个 tag_id 调
/// [`crate::api::file::cleanup_orphan_tag`]：COUNT(file_tags)=0 且无子节点的标签
/// （含 path/ext/type/time/user 所有类别）会被递归剪枝，跨库共享标签因他库仍有
/// status=1 关联而保留。清理失败仅记 error，不阻塞删库结果（删库本身已成功）。
pub async fn delete_library(
    State(pool): State<SqlitePool>,
    AxumPath(id): AxumPath<i32>,
) -> StatusCode {
    info!("删除资源库: id={}", id);

    // 删库前：收集该库文件关联过的 tag_ids（DELETE 后 file_tags 会被 CASCADE 清空，
    // 届时无法回溯受影响的标签）。
    let affected_tag_ids: Vec<i32> = match sqlx::query_scalar::<_, i32>(
        "SELECT DISTINCT ft.tag_id FROM file_tags ft \
         JOIN files f ON ft.file_id = f.id \
         WHERE f.library_id = ?",
    )
    .bind(id)
    .fetch_all(&pool)
    .await
    {
        Ok(ids) => ids,
        Err(e) => {
            error!("删库前收集受影响 tag_ids 失败 library_id={}: {}", id, e);
            // 不阻塞删库：直接 DELETE，仅放弃 tag 清理。
            Vec::new()
        }
    };

    let result = sqlx::query("DELETE FROM libraries WHERE id = ?")
        .bind(id)
        .execute(&pool)
        .await;

    match result {
        Ok(res) if res.rows_affected() > 0 => {
            info!("资源库删除成功: id={}", id);
            // best-effort 清理孤儿 tags；失败仅记日志，不影响删库已成功的 204。
            for tag_id in &affected_tag_ids {
                crate::api::file::cleanup_orphan_tag(&pool, *tag_id).await;
            }
            StatusCode::NO_CONTENT
        }
        Ok(_) => {
            warn!("资源库不存在: id={}", id);
            StatusCode::NOT_FOUND
        }
        Err(_) => StatusCode::INTERNAL_SERVER_ERROR,
    }
}

/// 测试资源库连接
///
/// # 路由
/// POST /api/v1/libraries/test
///
/// # 请求体
/// ```json
/// {
///   "name": "测试",
///   "protocol": "local",
///   "base_path": "/mnt/photos",
///   "config_json": null
/// }
/// ```
///
/// # 成功响应 (200)
/// ```json
/// {
///   "reachable": true,
///   "message": "路径可访问"
/// }
/// ```
pub async fn test_library_connection(
    Json(payload): Json<CreateLibraryRequest>,
) -> Json<TestConnectionResponse> {
    debug!(
        "测试连接: protocol={}, path={}",
        payload.protocol, payload.base_path
    );

    if payload.protocol == "local" {
        // 路径安全验证
        if let Err(err_msg) = validate_path_security(&payload.base_path) {
            warn!(
                "连接测试路径安全验证失败: {} - {}",
                payload.base_path, err_msg
            );
            return Json(TestConnectionResponse {
                reachable: false,
                message: err_msg.to_string(),
            });
        }

        // 路径可达性验证（与 create_library 共用同一份逻辑）
        match validate_local_path_readable(&payload.base_path) {
            Ok(_) => {
                info!("路径测试成功: {}", payload.base_path);
                Json(TestConnectionResponse {
                    reachable: true,
                    message: "路径可访问".to_string(),
                })
            }
            Err(message) => Json(TestConnectionResponse {
                reachable: false,
                message,
            }),
        }
    } else if payload.protocol == "webdav" {
        warn!("WebDAV 协议暂未实现");
        // WebDAV 暂不支持
        Json(TestConnectionResponse {
            reachable: false,
            message: "WebDAV 协议暂未实现".to_string(),
        })
    } else {
        warn!("不支持的协议类型: {}", payload.protocol);
        Json(TestConnectionResponse {
            reachable: false,
            message: "不支持的协议类型".to_string(),
        })
    }
}

/// 手动触发资源库扫描
///
/// # 路由
/// POST /api/v1/libraries/:id/scan
///
/// # 成功响应 (202)
/// 扫描任务已接受，将在后台异步执行
///
/// # 失败响应
/// - 404: 资源库不存在
/// - 409: 该资源库已有扫描进行中
/// - 500: 服务器错误
///
/// # 实现说明
/// 同步语义：调用方在 HTTP 响应里即可知道是否被 409 拒绝。
/// 共享扫描逻辑由 [`crate::engine::scanner::run_scan_with_lock_held`] 提供，
/// 与定时调度（`engine::scheduler`）共用，杜绝复制粘贴（详见
/// code-reuse-thinking-guide.md）。本函数职责收敛为：
/// 1. 同步查库（404）；
/// 2. 同步尝试加 409 锁（409）；
/// 3. spawn 后台任务调 `run_scan_with_lock_held` + 释放锁，立即返 202。
pub async fn trigger_scan(
    State(pool): State<SqlitePool>,
    AxumPath(id): AxumPath<i32>,
) -> StatusCode {
    // 1. 同步查库：不存在 → 404，DB 错误 → 500
    let library = match sqlx::query_as::<_, crate::models::db::Library>(
        "SELECT * FROM libraries WHERE id = ?",
    )
    .bind(id)
    .fetch_optional(&pool)
    .await
    {
        Ok(Some(lib)) => lib,
        Ok(None) => return StatusCode::NOT_FOUND,
        Err(_) => return StatusCode::INTERNAL_SERVER_ERROR,
    };

    // 2. 同步 try lock：同库扫描进行中 → 409（调用方立即拿到拒绝信号）
    if !try_acquire_scan_lock(id) {
        warn!("资源库正在扫描中，拒绝重复触发: id={}", id);
        return StatusCode::CONFLICT;
    }

    info!("扫描任务已接受: id={}, name={}", id, library.name);

    // 3. 锁已持有，spawn 后台执行共享扫描函数；完成后释放锁
    tokio::spawn(async move {
        let outcome = crate::engine::scanner::run_scan_with_lock_held(&pool, id).await;
        // 无论 outcome（Performed / NotFound / Err），都释放本任务持有的锁
        release_scan_lock(id);
        // 共享函数内部已对成功/失败分别记 info/error 日志，这里只在异常时额外 debug
        if let Err(e) = outcome {
            debug!("后台扫描任务结束于错误: id={}, {}", id, e);
        }
    });

    StatusCode::ACCEPTED
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_validate_local_path_readable_nonexistent() {
        // 不存在的绝对路径
        let ghost = "/tmp/tagflow-ghost-does-not-exist-1234567890";
        // 确保前提成立：该路径确实不存在
        assert!(!std::path::Path::new(ghost).exists());

        let result = validate_local_path_readable(ghost);
        assert!(result.is_err());
        assert_eq!(result.unwrap_err(), "路径不存在");
    }

    #[test]
    fn test_validate_local_path_readable_not_a_directory() {
        // 用本测试文件本身作为「存在但非目录」的样本
        let this_file = env!("CARGO_MANIFEST_DIR").to_string() + "/src/api/library.rs";
        assert!(
            std::path::Path::new(&this_file).exists(),
            "测试样本文件应存在"
        );
        assert!(!std::path::Path::new(&this_file).is_dir());

        let result = validate_local_path_readable(&this_file);
        assert!(result.is_err());
        assert_eq!(result.unwrap_err(), "路径不是目录");
    }

    #[test]
    fn test_validate_local_path_readable_existing_directory() {
        // target 目录在 cargo test 运行时必然存在且可读
        let target_dir = env!("CARGO_MANIFEST_DIR").to_string() + "/target";
        let target_dir = if std::path::Path::new(&target_dir).is_dir() {
            target_dir
        } else {
            // 兜底：用系统临时目录
            std::env::temp_dir().to_string_lossy().into_owned()
        };
        assert!(std::path::Path::new(&target_dir).is_dir());

        let result = validate_local_path_readable(&target_dir);
        assert!(result.is_ok(), "存在的目录应通过校验");
    }

    #[test]
    fn test_validate_path_security_rejects_traversal() {
        assert!(validate_path_security("../etc/passwd").is_err());
        assert!(validate_path_security("/data/./foo").is_err());
        assert!(validate_path_security("relative/path").is_err());
        assert!(validate_path_security("/mnt/photos").is_ok());
    }

    // ========== delete_library 孤儿标签清理 ==========

    /// 构造单连接内存库（schema 与 migrations/202512260001_init.sql 对齐）。
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
            "CREATE TABLE libraries (id INTEGER PRIMARY KEY AUTOINCREMENT, name TEXT NOT NULL, protocol TEXT NOT NULL, base_path TEXT NOT NULL, config_json TEXT, last_scanned_at DATETIME)",
            "CREATE TABLE tags (id INTEGER PRIMARY KEY AUTOINCREMENT, name TEXT NOT NULL, category TEXT NOT NULL, parent_id INTEGER REFERENCES tags(id) ON DELETE CASCADE, UNIQUE(name, parent_id))",
            "CREATE TABLE files (id INTEGER PRIMARY KEY AUTOINCREMENT, library_id INTEGER NOT NULL REFERENCES libraries(id) ON DELETE CASCADE, parent_path TEXT NOT NULL, filename TEXT NOT NULL, extension TEXT, size INTEGER NOT NULL, mtime INTEGER NOT NULL, hash TEXT, status INTEGER DEFAULT 1, indexed_at DATETIME DEFAULT CURRENT_TIMESTAMP)",
            "CREATE TABLE file_tags (file_id INTEGER NOT NULL REFERENCES files(id) ON DELETE CASCADE, tag_id INTEGER NOT NULL REFERENCES tags(id) ON DELETE CASCADE, source TEXT DEFAULT 'auto', PRIMARY KEY(file_id, tag_id))",
        ] {
            sqlx::query(stmt).execute(&pool).await.unwrap();
        }
        pool
    }

    async fn insert_library(pool: &SqlitePool, name: &str) -> i32 {
        sqlx::query("INSERT INTO libraries (name, protocol, base_path) VALUES (?, 'local', '/tmp')")
            .bind(name)
            .execute(pool)
            .await
            .unwrap()
            .last_insert_rowid() as i32
    }

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

    async fn insert_file(pool: &SqlitePool, library_id: i32, filename: &str) -> i32 {
        sqlx::query(
            "INSERT INTO files (library_id, parent_path, filename, size, mtime) VALUES (?, '', ?, 1, 0)",
        )
        .bind(library_id)
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

    async fn count_tags_by_id(pool: &SqlitePool, ids: &[i32]) -> i64 {
        if ids.is_empty() {
            return 0;
        }
        let placeholders = std::iter::repeat_n("?".to_string(), ids.len())
            .collect::<Vec<_>>()
            .join(",");
        let sql = format!("SELECT COUNT(*) FROM tags WHERE id IN ({})", placeholders);
        let mut q = sqlx::query_scalar::<_, i64>(&sql);
        for id in ids {
            q = q.bind(id);
        }
        q.fetch_one(pool).await.unwrap()
    }

    /// 删库后该库独有的 path/ext/type/time 标签应被清理（无残留孤儿）。
    #[tokio::test]
    async fn delete_library_cleans_orphan_tags_from_deleted_library() {
        let pool = setup_db().await;
        let lib = insert_library(&pool, "lib").await;

        // 建库独有的标签：path 层级 + ext + type
        let projects = insert_tag(&pool, "Projects", "path", None).await;
        let year2024 = insert_tag(&pool, "2024", "path", Some(projects)).await;
        let ext_png = insert_tag(&pool, "png", "ext", None).await;
        let type_image = insert_tag(&pool, "image", "type", None).await;

        // 文件挂在 lib，并关联这些标签
        let f = insert_file(&pool, lib, "a.png").await;
        link(&pool, f, year2024).await;
        link(&pool, f, ext_png).await;
        link(&pool, f, type_image).await;

        // 删库
        let status = delete_library(State(pool.clone()), AxumPath(lib)).await;
        assert_eq!(status, StatusCode::NO_CONTENT);

        // 所有受影响标签（叶子 + 向上递归）应被清空
        let remaining = count_tags_by_id(&pool, &[projects, year2024, ext_png, type_image]).await;
        assert_eq!(remaining, 0, "删库后该库独有标签应被清理");
    }

    /// 跨库共享标签保留：删一个库后，他库仍有 status=1 文件关联的标签应保留。
    #[tokio::test]
    async fn delete_library_keeps_cross_library_shared_tags() {
        let pool = setup_db().await;
        let lib_a = insert_library(&pool, "lib-a").await;
        let lib_b = insert_library(&pool, "lib-b").await;

        // 共享 ext 标签：被两个库的文件都关联
        let shared_ext = insert_tag(&pool, "png", "ext", None).await;
        // lib_a 独有的 path 标签
        let a_only = insert_tag(&pool, "OnlyInA", "path", None).await;

        let fa = insert_file(&pool, lib_a, "a.png").await;
        let fb = insert_file(&pool, lib_b, "b.png").await;
        link(&pool, fa, shared_ext).await;
        link(&pool, fb, shared_ext).await;
        link(&pool, fa, a_only).await;

        // 删 lib_a
        let status = delete_library(State(pool.clone()), AxumPath(lib_a)).await;
        assert_eq!(status, StatusCode::NO_CONTENT);

        // shared_ext 仍被 lib_b 的文件关联 → 保留
        let shared_alive: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM tags WHERE id = ?")
            .bind(shared_ext)
            .fetch_one(&pool)
            .await
            .unwrap();
        assert_eq!(shared_alive, 1, "跨库共享标签应保留");

        // lib_a 独有的 path 标签应被清
        let a_only_alive: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM tags WHERE id = ?")
            .bind(a_only)
            .fetch_one(&pool)
            .await
            .unwrap();
        assert_eq!(a_only_alive, 0, "lib_a 独有标签应被清理");
    }

    /// 向上递归剪枝：叶子删后父变空也删，清空整条空链。
    #[tokio::test]
    async fn delete_library_prunes_orphan_chain_recursively() {
        let pool = setup_db().await;
        let lib = insert_library(&pool, "lib").await;

        // path 层级 Projects → 2024 → Design（三层），仅叶子关联文件
        let projects = insert_tag(&pool, "Projects", "path", None).await;
        let y2024 = insert_tag(&pool, "2024", "path", Some(projects)).await;
        let design = insert_tag(&pool, "Design", "path", Some(y2024)).await;

        let f = insert_file(&pool, lib, "x.png").await;
        link(&pool, f, design).await;

        // 删库 → CASCADE 删 files/file_tags → 三层 path 链全部变空 → 全部清
        let status = delete_library(State(pool.clone()), AxumPath(lib)).await;
        assert_eq!(status, StatusCode::NO_CONTENT);

        let remaining = count_tags_by_id(&pool, &[projects, y2024, design]).await;
        assert_eq!(remaining, 0, "整条空链都应被递归剪枝");
    }

    /// 删不存在的库 → 404，且不触发任何清理。
    #[tokio::test]
    async fn delete_library_returns_404_for_missing_id() {
        let pool = setup_db().await;
        let status = delete_library(State(pool.clone()), AxumPath(999_987)).await;
        assert_eq!(status, StatusCode::NOT_FOUND);
    }
}
