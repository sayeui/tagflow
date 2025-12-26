mod models;
mod infra;

use tracing::{info, Level};
use tracing_subscriber::FmtSubscriber;

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

    // TODO: 启动扫描引擎与 API 服务
    
    Ok(())
}