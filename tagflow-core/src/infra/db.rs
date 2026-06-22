use sqlx::sqlite::{SqliteConnectOptions, SqliteJournalMode, SqlitePool, SqlitePoolOptions};
use std::str::FromStr;
use std::time::Duration;
use tracing::info;

/// 初始化 SQLite 连接池。
///
/// 使用 [`SqliteConnectOptions`] 对 pool **每个连接**统一配置：
/// - `journal_mode(Wal)`：WAL 模式（db 级，持久）
/// - `foreign_keys(true)`：强制外键（per-connection PRAGMA，必须每个连接都设）
/// - `busy_timeout(5s)`：写锁等待 5s 重试，缓解 scheduler/worker/手动扫描并发写冲突
///
/// 注意：手动执行 `PRAGMA foreign_keys=ON` 只对**当前执行它的连接**生效，
/// pool 其余连接 `foreign_keys=OFF` → `ON DELETE CASCADE` 不强制。
/// 因此连接级 PRAGMA 必须走 `SqliteConnectOptions`，禁止再退回手动 PRAGMA。
pub async fn init_db(database_url: &str) -> anyhow::Result<SqlitePool> {
    let options = SqliteConnectOptions::from_str(database_url)?
        .journal_mode(SqliteJournalMode::Wal)
        .foreign_keys(true)
        .busy_timeout(Duration::from_secs(5));

    let pool = SqlitePoolOptions::new()
        .max_connections(5)
        .acquire_timeout(Duration::from_secs(3))
        .connect_with(options)
        .await?;

    // 执行迁移脚本
    sqlx::migrate!("./migrations").run(&pool).await?;

    info!("数据库连接池就绪: WAL + foreign_keys=ON + busy_timeout=5s (max_connections=5)");

    Ok(pool)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;

    /// 构建临时数据库 URL（文件路径唯一，由 PID + 线程本地计数器拼成）。
    fn temp_db_url() -> String {
        use std::sync::atomic::{AtomicUsize, Ordering};
        use std::time::{SystemTime, UNIX_EPOCH};
        static COUNTER: AtomicUsize = AtomicUsize::new(0);
        let seq = COUNTER.fetch_add(1, Ordering::SeqCst);
        let micros = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_micros())
            .unwrap_or(0);
        let pid = std::process::id();
        let path = format!("/tmp/tagflow_test_{}_{}_{}.db", pid, micros, seq);
        format!("sqlite:{}?mode=rwc", path)
    }

    /// 测试结束清理临时文件（best-effort，失败忽略）。
    fn cleanup_db(url: &str) {
        let path = url
            .strip_prefix("sqlite:")
            .and_then(|s| s.split('?').next())
            .unwrap_or("");
        if !path.is_empty() {
            let _ = std::fs::remove_file(path);
            let _ = std::fs::remove_file(format!("{}-wal", path));
            let _ = std::fs::remove_file(format!("{}-shm", path));
        }
    }

    /// 并发写不锁：spawn 8 个并发任务同时写 `tasks` 表，
    /// 断言全部成功且无 `database is locked` 错误。
    ///
    /// 修复前（busy_timeout 未设）：并发写会立即返回 `SQLITE_BUSY`
    /// → 报 `database is locked (code: 5)`。
    /// 修复后（busy_timeout=5s）：写冲突在 5s 内重试通过。
    #[tokio::test]
    async fn test_concurrent_writes_no_deadlock() {
        let url = temp_db_url();
        let pool = init_db(&url).await.expect("init_db 失败");

        // 先建父数据：library + file，让 tasks 外键有依附
        sqlx::query(
            "INSERT INTO libraries (name, protocol, base_path) VALUES ('lib', 'local', '/tmp')",
        )
        .execute(&pool)
        .await
        .expect("插入 library 失败");
        sqlx::query("INSERT INTO files (library_id, parent_path, filename, size, mtime) VALUES (1, '/', 'a.txt', 0, 0)")
            .execute(&pool)
            .await
            .expect("插入 file 失败");

        let pool = Arc::new(pool);
        let mut handles = Vec::new();

        // 8 个并发任务，每个连续做多次 INSERT + UPDATE（竞争写锁）
        for task_idx in 0..8 {
            let p = pool.clone();
            handles.push(tokio::spawn(async move {
                for i in 0..10 {
                    sqlx::query(
                        "INSERT INTO tasks (file_id, task_type, status) VALUES (1, 'thumb', 0)",
                    )
                    .execute(&*p)
                    .await
                    // 关键断言：任何错误不应是 database is locked / busy
                    .inspect_err(|e| {
                        let msg = e.to_string();
                        assert!(
                            !msg.contains("locked") && !msg.contains("busy"),
                            "task {} 迭代 {} 遇到锁错误: {}",
                            task_idx,
                            i,
                            msg
                        );
                    })?;
                }
                // 紧接着一批 UPDATE，加剧写锁竞争
                for i in 0..10 {
                    sqlx::query("UPDATE tasks SET status = 1 WHERE file_id = 1")
                        .execute(&*p)
                        .await
                        .inspect_err(|e| {
                            let msg = e.to_string();
                            assert!(
                                !msg.contains("locked") && !msg.contains("busy"),
                                "task {} update {} 遇到锁错误: {}",
                                task_idx,
                                i,
                                msg
                            );
                        })?;
                }
                Ok::<(), sqlx::Error>(())
            }));
        }

        let mut failures = 0;
        for h in handles {
            match h.await {
                Ok(Ok(())) => {}
                other => {
                    failures += 1;
                    eprintln!("并发任务失败: {:?}", other);
                }
            }
        }

        assert_eq!(failures, 0, "存在失败的并发任务");

        // 验证 INSERT 真的落库（8 * 10 = 80 行）
        let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM tasks")
            .fetch_one(&*pool)
            .await
            .expect("COUNT 查询失败");
        assert_eq!(count, 80, "tasks 行数应为 80，实际 {}", count);

        cleanup_db(&url);
    }

    /// 验证 `PRAGMA foreign_keys=ON` 对 pool **任意连接**生效：
    /// 从不同连接分别 DELETE 父行，子表应 CASCADE 删除。
    ///
    /// 修复前：只有执行手动 PRAGMA 的那一个连接 foreign_keys=ON，
    /// 其余连接 OFF → 通过其他连接的 DELETE 不 CASCADE，子表残留。
    /// 修复后：`SqliteConnectOptions::foreign_keys(true)` 对每个连接都设，
    /// 所有连接的 DELETE 都 CASCADE。
    #[tokio::test]
    async fn test_foreign_keys_cascade_on_all_connections() {
        let url = temp_db_url();
        let pool = init_db(&url).await.expect("init_db 失败");

        // 构造 3 组 (library, file, tag, file_tag) 数据
        for lib_id in 1..=3 {
            sqlx::query(
                "INSERT INTO libraries (name, protocol, base_path) VALUES (?, 'local', '/tmp')",
            )
            .bind(format!("lib{}", lib_id))
            .execute(&pool)
            .await
            .expect("插入 library 失败");
            sqlx::query("INSERT INTO files (library_id, parent_path, filename, size, mtime) VALUES (?, '/', ?, 0, 0)")
                .bind(lib_id)
                .bind(format!("file{}.txt", lib_id))
                .execute(&pool)
                .await
                .expect("插入 file 失败");
            sqlx::query("INSERT INTO tags (name, category) VALUES (?, 'user')")
                .bind(format!("tag{}", lib_id))
                .execute(&pool)
                .await
                .expect("插入 tag 失败");
            sqlx::query("INSERT INTO file_tags (file_id, tag_id) VALUES (?, ?)")
                .bind(lib_id)
                .bind(lib_id)
                .execute(&pool)
                .await
                .expect("插入 file_tag 失败");
        }

        // 关键：通过 pool 取多个不同连接分别 DELETE，验证 CASCADE 在每个连接都生效。
        // max_connections(5) + 3 次独立 acquire 可以确保至少命中非初始连接。
        for lib_id in 1..=3 {
            // 取一个连接（每次循环拿到的可能是不同物理连接）
            let mut conn = pool.acquire().await.expect("acquire 连接失败");

            // 先确认这个连接 foreign_keys 确实开着
            let fk_on: i64 = sqlx::query_scalar("PRAGMA foreign_keys")
                .fetch_one(&mut *conn)
                .await
                .expect("PRAGMA foreign_keys 查询失败");
            assert_eq!(
                fk_on, 1,
                "连接 {} 的 foreign_keys 未开启（实际 {}），per-connection PRAGMA 修复失效",
                lib_id, fk_on
            );

            // 通过这个连接 DELETE 父行
            sqlx::query("DELETE FROM libraries WHERE id = ?")
                .bind(lib_id)
                .execute(&mut *conn)
                .await
                .unwrap_or_else(|_| panic!("DELETE library {} 失败", lib_id));

            // 释放连接回 pool
            drop(conn);

            // 验证级联删除：library 的 files 和 file_tags 应该都没了
            let files_count: i64 =
                sqlx::query_scalar("SELECT COUNT(*) FROM files WHERE library_id = ?")
                    .bind(lib_id)
                    .fetch_one(&pool)
                    .await
                    .expect("COUNT files 失败");
            assert_eq!(
                files_count, 0,
                "lib {} 的 files 未被级联删除（foreign_keys 对该连接不生效）",
                lib_id
            );

            let ft_count: i64 =
                sqlx::query_scalar("SELECT COUNT(*) FROM file_tags WHERE file_id = ?")
                    .bind(lib_id)
                    .fetch_one(&pool)
                    .await
                    .expect("COUNT file_tags 失败");
            assert_eq!(
                ft_count, 0,
                "lib {} 的 file_tags 未被级联删除（foreign_keys 对该连接不生效）",
                lib_id
            );
        }

        // 终态：3 个 library 全删，3 个 file（连带 3 个 file_tag）也全没了
        let total_files: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM files")
            .fetch_one(&pool)
            .await
            .expect("COUNT 全部 files 失败");
        assert_eq!(total_files, 0, "残留 files: {}", total_files);

        let total_ft: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM file_tags")
            .fetch_one(&pool)
            .await
            .expect("COUNT 全部 file_tags 失败");
        assert_eq!(total_ft, 0, "残留 file_tags: {}", total_ft);

        cleanup_db(&url);
    }

    /// 验证连接配置三个 PRAGMA 确实生效（WAL + foreign_keys + busy_timeout）。
    #[tokio::test]
    async fn test_connection_pragmas_applied() {
        let url = temp_db_url();
        let pool = init_db(&url).await.expect("init_db 失败");

        let mut conn = pool.acquire().await.expect("acquire 失败");

        let fk: i64 = sqlx::query_scalar("PRAGMA foreign_keys")
            .fetch_one(&mut *conn)
            .await
            .expect("PRAGMA foreign_keys 失败");
        assert_eq!(fk, 1, "foreign_keys 应为 ON (1)，实际 {}", fk);

        let busy: i64 = sqlx::query_scalar("PRAGMA busy_timeout")
            .fetch_one(&mut *conn)
            .await
            .expect("PRAGMA busy_timeout 失败");
        assert_eq!(busy, 5000, "busy_timeout 应为 5000ms，实际 {}ms", busy);

        let journal: String = sqlx::query_scalar("PRAGMA journal_mode")
            .fetch_one(&mut *conn)
            .await
            .expect("PRAGMA journal_mode 失败");
        assert_eq!(
            journal.to_lowercase(),
            "wal",
            "journal_mode 应为 wal，实际 {}",
            journal
        );

        cleanup_db(&url);
    }
}
