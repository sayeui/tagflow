use axum::{
    Router,
    extract::Request,
    middleware,
    middleware::Next,
    response::Response,
    routing::{delete, get, post},
};
use sqlx::SqlitePool;
use std::net::SocketAddr;
use tracing::{debug, error, info, warn};
use tracing_subscriber::{EnvFilter, fmt, prelude::*};

// 从库 crate 中导入模块
use tagflow_core::{api, core, infra};

/// 管理员密码环境变量名
const ADMIN_PASSWORD_ENV: &str = "TAGFLOW_ADMIN_PASSWORD";

/// 管理员密码最小字节数（OWASP 推荐下限）
const MIN_ADMIN_PASSWORD_LEN: usize = 12;

/// 开发默认管理员密码（仅在 debug 构建且未设置环境变量时使用）
///
/// 字面量长度 ≥ [`MIN_ADMIN_PASSWORD_LEN`]，明确标识非生产用途；
/// 生产环境必须通过 `TAGFLOW_ADMIN_PASSWORD` 覆盖。
const DEV_DEFAULT_ADMIN_PASSWORD: &str = "tagflow_dev_only_admin_pw";

/// 校验管理员密码字节数是否满足安全要求
///
/// OWASP 建议用户密码最小长度为 12 字符；本函数按 UTF-8 字节数校验，
/// 用于在首次创建管理员时拒绝弱密码。
fn validate_admin_password_len(len: usize) -> anyhow::Result<()> {
    if len < MIN_ADMIN_PASSWORD_LEN {
        return Err(anyhow::anyhow!(
            "TAGFLOW_ADMIN_PASSWORD 长度 {} < {} 字节，不满足安全要求",
            len,
            MIN_ADMIN_PASSWORD_LEN
        ));
    }
    Ok(())
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // 从环境变量读取日志级别，默认为 INFO
    // 使用方法: RUST_LOG=debug cargo run
    let env_filter = EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| EnvFilter::new("tagflow_core=info,axum=info"));

    // 初始化日志订阅器
    tracing_subscriber::registry()
        .with(env_filter)
        .with(
            fmt::layer()
                .with_target(true) // 显示模块路径
                .with_line_number(true) // 显示行号
                .with_thread_ids(false) // 不显示线程ID（减少噪音）
                .with_thread_names(false)
                .with_file(false), // 不显示文件路径
        )
        .init();

    info!("🚀 TagFlow Core 正在启动...");
    debug!("调试模式已启用");

    // 初始化 JWT 密钥（debug 缺失回退开发默认值，release 缺失则 fail-fast 退出）
    core::auth::init_jwt_secret()?;
    info!("JWT 密钥已初始化");

    // 读取运行时配置（DB 路径与缩略图缓存目录）
    let db_path = infra::config::db_path();
    let cache_dir = infra::config::cache_dir();
    info!("数据库路径: {}", db_path);
    info!("缩略图缓存目录: {}", cache_dir);

    // 初始化数据库（首次启动自动创建文件）
    let db_url = infra::config::db_url();
    let pool = infra::db::init_db(&db_url).await?;

    info!("数据库初始化成功并已应用迁移。");

    // 初始化管理员用户（如果不存在）
    ensure_admin_user(&pool).await?;

    // 启动后台任务 Worker
    let pool_for_worker = pool.clone();
    let cache_dir_for_worker = cache_dir.clone();
    tokio::spawn(async move {
        tagflow_core::engine::worker::start_task_worker(pool_for_worker, cache_dir_for_worker)
            .await;
    });
    info!("后台任务 Worker 已启动");

    // 构建路由
    // 1. 公开路由（无需认证）
    let auth_routes = Router::new()
        .route("/api/auth/login", post(api::auth::login))
        .route("/api/health", get(api::health::health))
        .layer(middleware::from_fn(request_logging_middleware));

    // 2. 受保护的路由（需要认证）
    let protected_routes = Router::new()
        .route("/api/v1/tags/tree", get(api::tag::get_tag_tree))
        .route("/api/v1/files", get(api::file::list_files))
        .route("/api/v1/files/:id/thumbnail", get(api::file::get_thumbnail))
        .route(
            "/api/auth/update-password",
            post(api::auth::update_password),
        )
        // Library 管理 API
        .route("/api/v1/libraries", get(api::library::list_libraries))
        .route("/api/v1/libraries", post(api::library::create_library))
        .route(
            "/api/v1/libraries/test",
            post(api::library::test_library_connection),
        )
        .route(
            "/api/v1/libraries/:id",
            delete(api::library::delete_library),
        )
        .route(
            "/api/v1/libraries/:id/scan",
            post(api::library::trigger_scan),
        )
        .layer(middleware::from_fn(api::auth::auth_middleware))
        .layer(middleware::from_fn(request_logging_middleware));

    // 合并路由：API 路由优先匹配，未命中走前端 SPA fallback
    let app = Router::new()
        .merge(auth_routes)
        .merge(protected_routes)
        .fallback(api::static_files::static_handler)
        .with_state(pool);

    // 启动服务器（监听端口可通过 TAGFLOW_PORT 环境变量覆盖，默认 8080）
    let port: u16 = match std::env::var("TAGFLOW_PORT") {
        Ok(value) => value.parse().unwrap_or_else(|_| {
            warn!("无效的 TAGFLOW_PORT 值: {}，回退到默认端口 8080", value);
            8080
        }),
        Err(_) => 8080,
    };
    let addr = SocketAddr::from(([0, 0, 0, 0], port));
    info!("API 服务器运行在 http://{}", addr);

    let listener = tokio::net::TcpListener::bind(addr).await?;
    axum::serve(listener, app).await?;

    Ok(())
}

/// 确保系统中存在至少一个管理员用户
///
/// 仅在 `users` 表为空（首次启动）时触发：
/// - 用户名：`TAGFLOW_ADMIN_USERNAME` 缺省回退 `admin`
/// - 密码：`TAGFLOW_ADMIN_PASSWORD` 缺省时，debug 构建回退到开发默认值并 `warn!`，
///   release 构建直接返回 `Err` 由 main 透传使进程退出；非空值需通过
///   [`validate_admin_password_len`] 长度校验。
///
/// 非空 users 表（已有管理员）：完全跳过本逻辑，环境变量不会被读取。
async fn ensure_admin_user(pool: &SqlitePool) -> anyhow::Result<()> {
    // 检查用户数量
    let count: i64 = sqlx::query_scalar("SELECT count(*) FROM users")
        .fetch_one(pool)
        .await?;

    if count == 0 {
        // 用户名保持现状：缺省回退 admin（决策 Q1-A）
        let admin_username =
            std::env::var("TAGFLOW_ADMIN_USERNAME").unwrap_or_else(|_| "admin".to_string());
        let admin_password = match std::env::var(ADMIN_PASSWORD_ENV) {
            Ok(s) if !s.is_empty() => {
                validate_admin_password_len(s.len())?;
                s
            }
            _ => {
                if cfg!(debug_assertions) {
                    warn!("TAGFLOW_ADMIN_PASSWORD 未设置，使用开发默认密码（仅 debug 构建可用）");
                    DEV_DEFAULT_ADMIN_PASSWORD.to_string()
                } else {
                    return Err(anyhow::anyhow!(
                        "生产模式首次启动必须设置 TAGFLOW_ADMIN_PASSWORD 环境变量（≥ {} 字节）",
                        MIN_ADMIN_PASSWORD_LEN
                    ));
                }
            }
        };

        // 哈希密码
        let password_hash = core::auth::hash_password(&admin_password)?;

        // 创建管理员用户
        sqlx::query("INSERT INTO users (username, password_hash) VALUES (?, ?)")
            .bind(&admin_username)
            .bind(&password_hash)
            .execute(pool)
            .await?;

        info!("==============================================");
        info!("检测到新系统：已创建默认管理员");
        info!("  用户名: {}", admin_username);
        info!("  密码: {}", admin_password);
        info!("  请在首次登录后修改密码！");
        info!("==============================================");
    }

    Ok(())
}

/// 请求日志中间件
///
/// 记录所有传入的 HTTP 请求，包括方法、路径和状态码
async fn request_logging_middleware(req: Request, next: Next) -> Response {
    let method = req.method().clone();
    let uri = req.uri().clone();
    let path = uri.path();

    // 检查是否有 Authorization 头
    let has_auth = req.headers().get("authorization").is_some();

    // 记录请求开始
    if has_auth {
        debug!("➡️  {} {} | authenticated", method, path);
    } else {
        debug!("➡️  {} {} | public", method, path);
    }

    let start = std::time::Instant::now();
    let response = next.run(req).await;
    let duration = start.elapsed();

    // 记录响应
    let status = response.status();
    let status_code = status.as_u16();

    if status.is_success() {
        debug!("✅ {} {} | {} | {:?}", method, path, status_code, duration);
    } else if status.is_client_error() {
        warn!("⚠️  {} {} | {} | {:?}", method, path, status_code, duration);
    } else if status.is_server_error() {
        error!("❌ {} {} | {} | {:?}", method, path, status_code, duration);
    } else {
        info!("ℹ️  {} {} | {} | {:?}", method, path, status_code, duration);
    }

    response
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_validate_admin_password_len_rejects_short() {
        // 0 字节必须被拒
        assert!(validate_admin_password_len(0).is_err());
        // 11 字节（< 12）必须被拒
        assert!(validate_admin_password_len(11).is_err());
        // 恰好 12 字节通过（OWASP 下限）
        assert!(validate_admin_password_len(12).is_ok());
        // 超过阈值通过
        assert!(validate_admin_password_len(64).is_ok());
    }
}
