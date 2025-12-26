
use sqlx::sqlite::{SqlitePool, SqlitePoolOptions};
use std::time::Duration;

pub async fn init_db(database_url: &str) -> anyhow::Result<SqlitePool> {
    // 1. 创建连接池
    let pool = SqlitePoolOptions::new()
        .max_connections(5)
        .acquire_timeout(Duration::from_secs(3))
        .connect(database_url)
        .await?;

    // 2. 强制开启 WAL 模式以支持并发
    sqlx::query("PRAGMA journal_mode = WAL;").execute(&pool).await?;
    sqlx::query("PRAGMA foreign_keys = ON;").execute(&pool).await?;

    // 3. 执行迁移脚本
    sqlx::migrate!("./migrations")
        .run(&pool)
        .await?;

    Ok(pool)
}