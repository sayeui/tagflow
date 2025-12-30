//! 后台任务调度器
//!
//! 异步处理缩略图生成等耗时任务

use sqlx::{SqlitePool, Row};
use std::time::Duration;
use tokio::time::sleep;
use tracing::{debug, info, warn, error};

use crate::infra::thumbnail::ThumbnailGenerator;

/// 任务状态枚举
#[repr(i32)]
pub enum TaskStatus {
    Pending = 0,     // 待处理
    Running = 1,     // 进行中
    Completed = 2,   // 已完成
    Failed = 3,      // 失败
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
             LIMIT 1"
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

                debug!("获取到任务: id={}, file_id={}, type={}", id, file_id, task_type);

                // 2. 更新状态为进行中 (1)
                if let Err(e) = sqlx::query(
                    "UPDATE tasks SET status = 1, started_at = CURRENT_TIMESTAMP WHERE id = ?"
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

                // 4. 更新任务完成状态
                match result {
                    Ok(_) => {
                        if let Err(e) = sqlx::query(
                            "UPDATE tasks SET status = 2, completed_at = CURRENT_TIMESTAMP WHERE id = ?"
                        )
                        .bind(id)
                        .execute(&pool)
                        .await
                        {
                            error!("更新任务完成状态失败: {}", e);
                        } else {
                            debug!("任务 {} 执行成功", id);
                        }
                    }
                    Err(e) => {
                        let error_msg = e.to_string();
                        warn!("任务 {} 执行失败: {}", id, error_msg);
                        if let Err(db_err) = sqlx::query(
                            "UPDATE tasks SET status = 3, error_msg = ?, completed_at = CURRENT_TIMESTAMP WHERE id = ?"
                        )
                        .bind(&error_msg)
                        .bind(id)
                        .execute(&pool)
                        .await
                        {
                            error!("更新任务失败状态失败: {}", db_err);
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
         VALUES (?, 'thumb', 0, ?)"
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
         WHERE file_id = ? AND task_type = 'thumb' AND status IN (0, 1)"
    )
    .bind(file_id)
    .fetch_one(pool)
    .await?;

    Ok(count > 0)
}
