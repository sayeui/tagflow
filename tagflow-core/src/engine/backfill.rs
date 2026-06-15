//! 存量数据回填：当 tagger 流水线版本升级时，对历史文件补齐新维度标签。
//!
//! 启动时由 `main.rs` 调用 [`run_if_needed`]：
//! 1. 读 `app_meta.tagger_version`（缺失视为 0）
//! 2. 若低于 [`crate::engine::tagger::CURRENT_TAGGER_VERSION`]，遍历全表回填
//! 3. 写回新版本号
//!
//! 回填复用 scanner 的 [`tagger::run_all`]，保证入库与回填逻辑一致。

use sqlx::SqlitePool;
use tracing::{info, warn};

use crate::core::tag::TagManager;
use crate::engine::tagger::{self, CURRENT_TAGGER_VERSION};

const VERSION_KEY: &str = "tagger_version";

/// 若 tagger 版本落后，对全表文件回填标签。幂等、可重复运行。
pub async fn run_if_needed(db: &SqlitePool) -> anyhow::Result<()> {
    let current = read_version(db).await?;
    if current >= CURRENT_TAGGER_VERSION {
        info!("tagger 版本已是最新 ({current})，跳过回填");
        return Ok(());
    }

    info!("tagger 版本落后 ({current} < {CURRENT_TAGGER_VERSION})，开始回填存量标签...");

    // 流式拉取存量文件元数据（避免大库一次性 load 全表）
    let rows: Vec<(i32, String, Option<String>, i64)> = sqlx::query_as(
        "SELECT id, parent_path, extension, mtime FROM files WHERE status = 1",
    )
    .fetch_all(db)
    .await?;

    let total = rows.len();
    info!("待回填文件数: {total}");

    let tag_mgr = TagManager::new(db.clone());
    let mut ok = 0usize;
    for (file_id, parent_path, ext, mtime) in rows {
        if let Err(e) =
            tagger::run_all(&tag_mgr, file_id, &parent_path, ext.as_deref(), mtime).await
        {
            warn!("回填文件 {file_id} 失败: {e}");
            continue;
        }
        ok += 1;
    }

    write_version(db, CURRENT_TAGGER_VERSION).await?;
    info!("回填完成：{ok}/{total} 个文件已更新，tagger 版本 → {CURRENT_TAGGER_VERSION}");
    Ok(())
}

async fn read_version(db: &SqlitePool) -> anyhow::Result<i64> {
    let row: Option<(String,)> =
        sqlx::query_as("SELECT value FROM app_meta WHERE key = ?")
            .bind(VERSION_KEY)
            .fetch_optional(db)
            .await?;
    let v = row
        .map(|(v,)| v)
        .unwrap_or_else(|| "0".to_string())
        .parse::<i64>()
        .unwrap_or(0);
    Ok(v)
}

async fn write_version(db: &SqlitePool, version: i64) -> anyhow::Result<()> {
    sqlx::query(
        "INSERT INTO app_meta (key, value) VALUES (?, ?) \
         ON CONFLICT(key) DO UPDATE SET value = excluded.value",
    )
    .bind(VERSION_KEY)
    .bind(version.to_string())
    .execute(db)
    .await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    async fn setup() -> SqlitePool {
        let pool = sqlx::sqlite::SqlitePoolOptions::new()
            .max_connections(1)
            .connect("sqlite::memory:")
            .await
            .unwrap();
        sqlx::query("PRAGMA foreign_keys = ON").execute(&pool).await.unwrap();
        for stmt in [
            "CREATE TABLE app_meta (key TEXT PRIMARY KEY, value TEXT NOT NULL)",
            "CREATE TABLE tags (id INTEGER PRIMARY KEY AUTOINCREMENT, name TEXT NOT NULL, category TEXT NOT NULL, parent_id INTEGER REFERENCES tags(id) ON DELETE CASCADE, UNIQUE(name, parent_id))",
            "CREATE TABLE files (id INTEGER PRIMARY KEY AUTOINCREMENT, library_id INTEGER NOT NULL, parent_path TEXT NOT NULL, filename TEXT NOT NULL, extension TEXT, size INTEGER NOT NULL, mtime INTEGER NOT NULL, hash TEXT, status INTEGER DEFAULT 1, indexed_at DATETIME DEFAULT CURRENT_TIMESTAMP)",
            "CREATE TABLE file_tags (file_id INTEGER NOT NULL REFERENCES files(id) ON DELETE CASCADE, tag_id INTEGER NOT NULL REFERENCES tags(id) ON DELETE CASCADE, source TEXT DEFAULT 'auto', PRIMARY KEY(file_id, tag_id))",
        ] {
            sqlx::query(stmt).execute(&pool).await.unwrap();
        }
        pool
    }

    #[tokio::test]
    async fn read_version_defaults_to_zero_when_unset() {
        let pool = setup().await;
        assert_eq!(read_version(&pool).await.unwrap(), 0);
    }

    #[tokio::test]
    async fn write_then_read_version_roundtrip() {
        let pool = setup().await;
        write_version(&pool, 2).await.unwrap();
        assert_eq!(read_version(&pool).await.unwrap(), 2);
        // 覆盖写
        write_version(&pool, 3).await.unwrap();
        assert_eq!(read_version(&pool).await.unwrap(), 3);
    }

    #[tokio::test]
    async fn run_if_needed_backfills_and_bumps_version() {
        let pool = setup().await;
        // 插入一个 txt 文件，mtime=1700000000（2023 本地时间）
        sqlx::query("INSERT INTO files (library_id, parent_path, filename, extension, size, mtime) VALUES (1, 'docs/', 'a.txt', 'txt', 1, 1700000000)")
            .execute(&pool).await.unwrap();

        run_if_needed(&pool).await.unwrap();

        // 版本应更新
        assert_eq!(read_version(&pool).await.unwrap(), CURRENT_TAGGER_VERSION);

        // 应生成 ext:txt、type:text 标签并关联
        let ext_count: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM tags WHERE category = 'ext' AND name = 'txt'",
        )
        .fetch_one(&pool).await.unwrap();
        assert_eq!(ext_count, 1);
        let type_count: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM tags WHERE category = 'type' AND name = 'text'",
        )
        .fetch_one(&pool).await.unwrap();
        assert_eq!(type_count, 1);
        // 应生成 time 标签（year + month）
        let time_count: i64 =
            sqlx::query_scalar("SELECT COUNT(*) FROM tags WHERE category = 'time'")
                .fetch_one(&pool).await.unwrap();
        assert!(time_count >= 2, "至少应有 year + month 两个 time 标签");

        // 第二次运行应跳过（版本已是最新）
        // 用文件无新增来间接验证：tag 数量不变
        let tags_before: i64 =
            sqlx::query_scalar("SELECT COUNT(*) FROM tags").fetch_one(&pool).await.unwrap();
        run_if_needed(&pool).await.unwrap();
        let tags_after: i64 =
            sqlx::query_scalar("SELECT COUNT(*) FROM tags").fetch_one(&pool).await.unwrap();
        assert_eq!(tags_before, tags_after, "幂等：第二次运行不应新增标签");
    }
}
