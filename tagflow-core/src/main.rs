use tracing::{info, Level};
use tracing_subscriber::FmtSubscriber;
use axum::{routing::{get, post}, Router, middleware};
use std::net::SocketAddr;
use sqlx::SqlitePool;

// 从库 crate 中导入模块
use tagflow_core::{infra, core, api};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // 初始化日志
    let subscriber = FmtSubscriber::builder()
        .with_max_level(Level::INFO)
        .finish();
    tracing::subscriber::set_global_default(subscriber)?;

    info!("TagFlow Core 正在启动...");

    // 初始化数据库 (本地文件 tagflow.db)
    let db_url = "sqlite:tagflow.db?mode=rwc";
    let pool = infra::db::init_db(db_url).await?;

    info!("数据库初始化成功并已应用迁移。");

    // 初始化管理员用户（如果不存在）
    ensure_admin_user(&pool).await?;

    // 构建路由
    // 1. 公开路由（无需认证）
    let auth_routes = Router::new()
        .route("/api/auth/login", post(api::auth::login));

    // 2. 受保护的路由（需要认证）
    let protected_routes = Router::new()
        .route("/api/v1/tags/tree", get(api::tag::get_tag_tree))
        .route("/api/v1/files", get(api::file::list_files))
        .route("/api/auth/update-password", post(api::auth::update_password))
        .layer(middleware::from_fn(api::auth::auth_middleware));

    // 合并路由
    let app = Router::new()
        .merge(auth_routes)
        .merge(protected_routes)
        .with_state(pool);

    // 启动服务器
    let addr = SocketAddr::from(([0, 0, 0, 0], 8080));
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
        let admin_username = std::env::var("TAGFLOW_ADMIN_USERNAME")
            .unwrap_or_else(|_| "admin".to_string());
        let admin_password = std::env::var("TAGFLOW_ADMIN_PASSWORD")
            .unwrap_or_else(|_| "PhVENfYaWv".to_string());

        // 哈希密码
        let password_hash = core::auth::hash_password(&admin_password)?;

        // 创建管理员用户
        sqlx::query(
            "INSERT INTO users (username, password_hash) VALUES (?, ?)"
        )
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
