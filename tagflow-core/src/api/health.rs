//! 健康检查端点：供 Docker HEALTHCHECK 与编排系统 liveness probe 使用。
//!
//! 设计约束：
//! - 无需认证（公开路由）
//! - 不依赖数据库 / 外部 IO（避免把数据库压力耦合到 liveness 判定）
//! - 直接返回 200，请求日志由 [`crate::request_logging_middleware`] 统一记录

use axum::http::StatusCode;

/// `GET /api/health`：返回 200 OK 表示进程存活
///
/// # 路由
/// GET /api/health
///
/// # 成功响应 (200)
/// 空响应体
///
/// # 说明
/// 本端点不检查数据库连接，避免因数据库瞬时抖动导致容器被频繁重启。
/// readiness 与 liveness 都用同一端点；如需独立 readiness 探针可后续扩展。
pub async fn health() -> StatusCode {
    StatusCode::OK
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn health_返回_200() {
        assert_eq!(health().await, StatusCode::OK);
    }
}
