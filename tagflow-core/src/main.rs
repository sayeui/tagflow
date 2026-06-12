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

    // 初始化数据库 (本地文件 tagflow.db)
    let db_url = "sqlite:tagflow.db?mode=rwc";
    let pool = infra::db::init_db(db_url).await?;

    info!("数据库初始化成功并已应用迁移。");

    // 初始化管理员用户（如果不存在）
    ensure_admin_user(&pool).await?;

    // 启动后台任务 Worker
    let pool_for_worker = pool.clone();
    tokio::spawn(async move {
        tagflow_core::engine::worker::start_task_worker(pool_for_worker, "./cache".to_string())
            .await;
    });
    info!("后台任务 Worker 已启动");

    // 构建路由
    // 1. 公开路由（无需认证）
    let auth_routes = Router::new()
        .route("/api/auth/login", post(api::auth::login))
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

    // 合并路由
    let app = Router::new()
        .merge(auth_routes)
        .merge(protected_routes)
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
/// 如果数据库中没有用户，则创建默认管理员。
/// 生产环境中应从环境变量读取管理员凭据。
async fn ensure_admin_user(pool: &SqlitePool) -> anyhow::Result<()> {
    // 检查用户数量
    let count: i64 = sqlx::query_scalar("SELECT count(*) FROM users")
        .fetch_one(pool)
        .await?;

    if count == 0 {
        // 从环境变量读取管理员凭据，或使用默认值
        let admin_username =
            std::env::var("TAGFLOW_ADMIN_USERNAME").unwrap_or_else(|_| "admin".to_string());
        let admin_password =
            std::env::var("TAGFLOW_ADMIN_PASSWORD").unwrap_or_else(|_| "PhVENfYaWv".to_string());

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
