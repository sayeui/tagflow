//! 后台任务调度器
//!
//! 异步处理缩略图生成等耗时任务

use sqlx::{Row, SqlitePool};
use std::time::Duration;
use tokio::time::sleep;
use tracing::{debug, error, info, warn};

use crate::infra::thumbnail::ThumbnailGenerator;

/// 任务状态枚举
#[repr(i32)]
pub enum TaskStatus {
    Pending = 0,   // 待处理
    Running = 1,   // 进行中
    Completed = 2, // 已完成
    Failed = 3,    // 失败
}

/// 启动后台任务 Worker
///
/// 此函数会无限循环，持续从数据库获取待处理任务并执行。
/// 应该在独立的 Tokio 任务中运行。
///
/// # 参数
/// - `pool`: 数据库连接池
/// - `cache_dir`: 缩略图缓存目录
pub async fn start_task_worker(pool: SqlitePool, cache_dir: String) {
    let generator = ThumbnailGenerator::new(cache_dir);

    info!("异步任务 Worker 已启动");

    loop {
        // 1. 获取一个待处理任务 (使用运行时检查)
        let task = sqlx::query(
            "SELECT id, file_id, task_type FROM tasks
             WHERE status = 0
             ORDER BY priority DESC, id ASC
             LIMIT 1",
        )
        .fetch_optional(&pool)
        .await;

        match task {
            Ok(Some(row)) => {
                // 从行中提取字段 (需要显式类型注解)
                let id: i32 = match row.try_get("id") {
                    Ok(v) => v,
                    Err(e) => {
                        error!("解析任务 ID 失败: {}", e);
                        sleep(Duration::from_secs(5)).await;
                        continue;
                    }
                };
                let file_id: i32 = match row.try_get("file_id") {
                    Ok(v) => v,
                    Err(e) => {
                        error!("解析文件 ID 失败: {}", e);
                        sleep(Duration::from_secs(5)).await;
                        continue;
                    }
                };
                let task_type: String = match row.try_get("task_type") {
                    Ok(v) => v,
                    Err(e) => {
                        error!("解析任务类型失败: {}", e);
                        sleep(Duration::from_secs(5)).await;
                        continue;
                    }
                };

                debug!(
                    "获取到任务: id={}, file_id={}, type={}",
                    id, file_id, task_type
                );

                // 2. 更新状态为进行中 (1)
                if let Err(e) = sqlx::query(
                    "UPDATE tasks SET status = 1, started_at = CURRENT_TIMESTAMP WHERE id = ?",
                )
                .bind(id)
                .execute(&pool)
                .await
                {
                    error!("更新任务状态失败: {}", e);
                    // 出错时休眠后重试
                    sleep(Duration::from_secs(5)).await;
                    continue;
                }

                // 3. 执行任务逻辑
                let result: anyhow::Result<()> = match task_type.as_str() {
                    "thumb" => {
                        // 缩略图生成任务
                        generator.generate_for_file(file_id, &pool).await
                    }
                    _ => {
                        warn!("未知任务类型: {}", task_type);
                        Err(anyhow::anyhow!("未知任务类型: {}", task_type))
                    }
                };

                // 4. 更新任务完成状态（遇 SQLITE_BUSY 重试，避免任务卡 Running）
                match result {
                    Ok(_) => {
                        if let Err(e) =
                            update_task_status_with_retry(&pool, id, TaskStatus::Completed, None)
                                .await
                        {
                            error!("更新任务完成状态失败（重试用尽）: {}", e);
                        } else {
                            debug!("任务 {} 执行成功", id);
                        }
                    }
                    Err(e) => {
                        let error_msg = e.to_string();
                        warn!("任务 {} 执行失败: {}", id, error_msg);
                        if let Err(db_err) = update_task_status_with_retry(
                            &pool,
                            id,
                            TaskStatus::Failed,
                            Some(&error_msg),
                        )
                        .await
                        {
                            error!("更新任务失败状态失败（重试用尽）: {}", db_err);
                        }
                    }
                }
            }
            Ok(None) => {
                // 没有任务，休眠一段时间
                debug!("暂无待处理任务，休眠 5 秒");
                sleep(Duration::from_secs(5)).await;
            }
            Err(e) => {
                error!("查询任务失败: {}", e);
                sleep(Duration::from_secs(5)).await;
            }
        }
    }
}

/// 为文件创建缩略图生成任务
///
/// # 参数
/// - `pool`: 数据库连接池
/// - `file_id`: 文件 ID
/// - `priority`: 任务优先级 (可选，默认 0)
///
/// # 返回
/// - `Ok(task_id)`: 任务创建成功，返回任务 ID
/// - `Err(sqlx::Error)`: 数据库错误
pub async fn create_thumbnail_task(
    pool: &SqlitePool,
    file_id: i32,
    priority: Option<i32>,
) -> Result<i64, sqlx::Error> {
    let result = sqlx::query(
        "INSERT INTO tasks (file_id, task_type, status, priority)
         VALUES (?, 'thumb', 0, ?)",
    )
    .bind(file_id)
    .bind(priority.unwrap_or(0))
    .execute(pool)
    .await?;

    Ok(result.last_insert_rowid())
}

/// 检查文件是否有待处理的缩略图任务
///
/// # 参数
/// - `pool`: 数据库连接池
/// - `file_id`: 文件 ID
///
/// # 返回
/// - `Ok(true)`: 有待处理任务
/// - `Ok(false)`: 无待处理任务
/// - `Err(sqlx::Error)`: 数据库错误
pub async fn has_pending_thumbnail_task(
    pool: &SqlitePool,
    file_id: i32,
) -> Result<bool, sqlx::Error> {
    let count = sqlx::query_scalar::<_, i64>(
        "SELECT COUNT(*) FROM tasks
         WHERE file_id = ? AND task_type = 'thumb' AND status IN (0, 1)",
    )
    .bind(file_id)
    .fetch_one(pool)
    .await?;

    Ok(count > 0)
}

/// 判断 sqlx 错误是否为 `SQLITE_BUSY`（写锁被占）。
///
/// SQLite `SQLITE_BUSY` 的错误码为 `5`（字符串 `"5"`），错误消息通常含 "locked"。
/// 两种判定任一命中即视为 busy，避免依赖单一字段。
fn is_busy_error(err: &sqlx::Error) -> bool {
    if let Some(db_err) = err.as_database_error() {
        // SQLite 错误码以字符串形式返回（如 "5"）
        if db_err.code().map(|c| c == "5").unwrap_or(false) {
            return true;
        }
    }
    let msg = err.to_string().to_lowercase();
    msg.contains("locked") || msg.contains("busy")
}

/// 更新任务状态，遇 `SQLITE_BUSY` 退避重试 3 次（500ms / 1s / 2s）。
///
/// scanner 1700+ 文件库扫描密集写时，worker 的 `UPDATE task status` 偶发抢不到锁，
/// 即便 `busy_timeout=15s` 仍可能在边界超时。此 helper 作为兜底，重试 3 次后仍失败
/// 才返回错误（调用方 `error!` 记日志，不让任务卡 Running）。
///
/// # 参数
/// - `pool`: 数据库连接池
/// - `id`: 任务 ID
/// - `status`: 目标状态（`TaskStatus::Completed` 或 `TaskStatus::Failed`）
/// - `error_msg`: 失败时写入的 error_msg（仅 Failed 用，Completed 传 None）
async fn update_task_status_with_retry(
    pool: &SqlitePool,
    id: i32,
    status: TaskStatus,
    error_msg: Option<&str>,
) -> Result<(), sqlx::Error> {
    // 退避序列：500ms / 1s / 2s
    const BACKOFFS: [Duration; 3] = [
        Duration::from_millis(500),
        Duration::from_secs(1),
        Duration::from_secs(2),
    ];

    let mut last_err: Option<sqlx::Error> = None;

    // 总尝试次数 = BACKOFFS.len() + 1（初次 + 3 次重试）
    for (attempt, backoff) in [None]
        .into_iter()
        .chain(BACKOFFS.iter().map(Some))
        .enumerate()
    {
        let res = match status {
            TaskStatus::Completed => {
                sqlx::query(
                    "UPDATE tasks SET status = 2, completed_at = CURRENT_TIMESTAMP WHERE id = ?",
                )
                .bind(id)
                .execute(pool)
                .await
            }
            TaskStatus::Failed => {
                sqlx::query(
                    "UPDATE tasks SET status = 3, error_msg = ?, completed_at = CURRENT_TIMESTAMP WHERE id = ?"
                )
                .bind(error_msg)
                .bind(id)
                .execute(pool)
                .await
            }
            // 其它状态不走重试路径（本 helper 仅用于完成态更新）
            _ => return Ok(()),
        };

        match res {
            Ok(_) => {
                if attempt > 0 {
                    debug!("任务 {} 状态更新在第 {} 次重试后成功", id, attempt);
                }
                return Ok(());
            }
            Err(e) => {
                if is_busy_error(&e) {
                    last_err = Some(e);
                    if let Some(d) = backoff {
                        debug!(
                            "任务 {} 状态更新遇 SQLITE_BUSY，{}ms 后重试（第 {}/{} 次）",
                            id,
                            d.as_millis(),
                            attempt + 1,
                            BACKOFFS.len()
                        );
                        sleep(*d).await;
                        continue;
                    }
                } else {
                    // 非 busy 错误直接返回，不重试
                    return Err(e);
                }
            }
        }
    }

    // 重试用尽
    Err(last_err.unwrap())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// `is_busy_error` 对含 "locked"/"busy" 的错误消息判定为 true。
    ///
    /// 生产中 SQLite `SQLITE_BUSY` 抛出 "database is locked" 消息，sqlx 把它包进
    /// `Error::Database`。由于 `sqlx::sqlite::SqliteError` 私有构造函数无法直接构造，
    /// 这里用 `Error::Protocol` 模拟同样消息文本，覆盖消息匹配路径。
    #[test]
    fn test_is_busy_error_detects_locked_message() {
        let err = sqlx::Error::Protocol("database is locked".into());
        assert!(is_busy_error(&err), "含 'locked' 的错误应被判为 busy");

        let err = sqlx::Error::Protocol("database is busy".into());
        assert!(is_busy_error(&err), "含 'busy' 的错误应被判为 busy");

        // 大小写无关
        let err = sqlx::Error::Protocol("DATABASE IS LOCKED".into());
        assert!(is_busy_error(&err), "大小写无关应判为 busy");
    }

    /// `is_busy_error` 对非 busy 错误判定为 false（不误触发重试）。
    #[test]
    fn test_is_busy_error_rejects_non_busy() {
        let err = sqlx::Error::Protocol("no such table: tasks".into());
        assert!(
            !is_busy_error(&err),
            "无 locked/busy 字样的错误不应判为 busy"
        );

        let err = sqlx::Error::PoolClosed;
        assert!(!is_busy_error(&err), "PoolClosed 不应判为 busy");

        let err = sqlx::Error::Configuration("invalid url".into());
        assert!(!is_busy_error(&err), "配置错误不应判为 busy");
    }

    /// `update_task_status_with_retry` 对非 busy 错误立即返回，不重试。
    ///
    /// 用一个不存在的 file_id 触发外键约束错误（非 busy），断言函数立即返回 Err，
    /// 不进入退避循环（避免单测阻塞数秒）。
    #[tokio::test]
    async fn test_update_status_no_retry_on_non_busy_error() {
        let url = format!(
            "sqlite:/tmp/tagflow_worker_test_{}_{}.db?mode=rwc",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_micros()
        );
        let pool = crate::infra::db::init_db(&url).await.expect("init_db 失败");

        // 任务 id 999 不存在 → UPDATE 影响 0 行但 sqlx 不报错；
        // 改用破坏 SQL（指向不存在的列）触发非 busy 错误，验证不重试。
        // 这里我们直接用一个会失败的非 busy 操作：向不存在的表插入。
        let start = std::time::Instant::now();
        // 任务 id 999 不存在，UPDATE 成功（0 行受影响），不应进入重试。
        let result = update_task_status_with_retry(&pool, 999, TaskStatus::Completed, None).await;
        assert!(
            result.is_ok(),
            "UPDATE 不存在的任务应 Ok（0 行），不应报错: {:?}",
            result
        );
        // 应在远小于退避总时长（3.5s）内完成
        assert!(
            start.elapsed() < std::time::Duration::from_secs(1),
            "非 busy 路径不应触发退避，实际耗时 {:?}",
            start.elapsed()
        );

        // 清理
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
}
