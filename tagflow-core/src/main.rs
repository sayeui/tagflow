mod models;
mod infra;
mod engine;
mod core;
mod api;

use tracing::{info, Level};
use tracing_subscriber::FmtSubscriber;
use axum::{routing::get, Router};
use std::net::SocketAddr;

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

    // 构建路由
    let app = Router::new()
        .route("/api/v1/tags/tree", get(api::tag::get_tag_tree))
        .route("/api/v1/files", get(api::file::list_files))
        .with_state(pool);

    // 启动服务器
    let addr = SocketAddr::from(([0, 0, 0, 0], 8080));
    info!("API 服务器运行在 http://{}", addr);

    let listener = tokio::net::TcpListener::bind(addr).await?;
    axum::serve(listener, app).await?;

    Ok(())
}
