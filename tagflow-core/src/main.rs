mod models;
mod infra;
mod engine;

use tracing::{info, Level};
use tracing_subscriber::FmtSubscriber;
use engine::scanner::Scanner;
use models::db::Library;

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

    // 1. 创建一个测试资源库 (如果不存在)
    sqlx::query("INSERT OR IGNORE INTO libraries (id, name, protocol, base_path) VALUES (1, '测试库', 'local', './test_dir')")
        .execute(&pool).await?;

    // 2. 准备测试库对应的实体
    let test_lib = Library {
        id: 1,
        name: "测试库".to_string(),
        protocol: "local".to_string(),
        base_path: "./test_dir".to_string(), // 请确保本地有这个文件夹
        config_json: None,
        last_scanned_at: None,
    };

    // 3. 执行扫描
    let scanner = Scanner::new(pool.clone());
    scanner.scan_library(&test_lib).await?;

    info!("扫描测试完成。");
    Ok(())
}