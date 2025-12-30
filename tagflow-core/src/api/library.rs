//! Library API - 资源库管理
//!
//! 提供资源库的 CRUD 操作、连接测试和扫描触发功能。

use axum::{
    extract::{Path as AxumPath, State},
    http::StatusCode,
    Json,
};
use sqlx::SqlitePool;
use tracing::{debug, info, warn};

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
///     "last_scanned_at": "2024-01-01T00:00:00Z"
///   }
/// ]
/// ```
pub async fn list_libraries(
    State(pool): State<SqlitePool>,
) -> Result<Json<Vec<LibraryResponse>>, StatusCode> {
    debug!("获取资源库列表");

    let libraries = sqlx::query_as::<_, crate::models::db::Library>(
        "SELECT * FROM libraries ORDER BY id"
    )
    .fetch_all(&pool)
    .await
    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    let response: Vec<LibraryResponse> = libraries.into_iter().map(|lib| lib.into()).collect();

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
    info!("创建资源库: name={}, protocol={}, path={}", payload.name, payload.protocol, payload.base_path);

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

    sqlx::query(
        "INSERT INTO libraries (name, protocol, base_path, config_json)
         VALUES (?, ?, ?, ?)"
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
pub async fn delete_library(
    State(pool): State<SqlitePool>,
    AxumPath(id): AxumPath<i32>,
) -> StatusCode {
    info!("删除资源库: id={}", id);

    let result = sqlx::query("DELETE FROM libraries WHERE id = ?")
        .bind(id)
        .execute(&pool)
        .await;

    match result {
        Ok(res) if res.rows_affected() > 0 => {
            info!("资源库删除成功: id={}", id);
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
    debug!("测试连接: protocol={}, path={}", payload.protocol, payload.base_path);

    if payload.protocol == "local" {
        // 路径安全验证
        if let Err(err_msg) = validate_path_security(&payload.base_path) {
            warn!("连接测试路径安全验证失败: {} - {}", payload.base_path, err_msg);
            return Json(TestConnectionResponse {
                reachable: false,
                message: err_msg.to_string(),
            });
        }

        // 检查本地目录是否存在且可读
        let path = std::path::Path::new(&payload.base_path);

        let (reachable, message) = if path.exists() {
            if path.is_dir() {
                // 检查是否可读
                match std::fs::read_dir(path) {
                    Ok(_) => {
                        info!("路径测试成功: {}", payload.base_path);
                        (true, "路径可访问".to_string())
                    }
                    Err(e) => {
                        warn!("路径无权限: {} - {}", payload.base_path, e);
                        (false, "无权限访问此目录".to_string())
                    }
                }
            } else {
                warn!("路径不是目录: {}", payload.base_path);
                (false, "路径不是目录".to_string())
            }
        } else {
            warn!("路径不存在: {}", payload.base_path);
            (false, "路径不存在".to_string())
        };

        Json(TestConnectionResponse { reachable, message })
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
/// - 501: 扫描功能未实现
pub async fn trigger_scan(
    State(pool): State<SqlitePool>,
    AxumPath(id): AxumPath<i32>,
) -> StatusCode {
    // 获取资源库配置
    let _library = match sqlx::query_as::<_, crate::models::db::Library>(
        "SELECT * FROM libraries WHERE id = ?"
    )
    .bind(id)
    .fetch_optional(&pool)
    .await
    {
        Ok(Some(lib)) => lib,
        Ok(None) => return StatusCode::NOT_FOUND,
        Err(_) => return StatusCode::INTERNAL_SERVER_ERROR,
    };

    // TODO: 实现扫描功能
    // 当前扫描功能尚未完全实现，返回 NOT_IMPLEMENTED
    StatusCode::NOT_IMPLEMENTED
}
