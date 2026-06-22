//! 定时扫描调度器
//!
//! 后台周期性扫描所有资源库，替代仅靠手动触发 `POST /libraries/:id/scan` 的模式，
//! 让文件增删改在下一轮自动同步进库（缩略图任务照常由扫描器入队 worker）。
//!
//! # 调度模型（ADR-lite，详见 prd.md「已定决策」）
//! - **全局单定时器**：一个 scheduler 每 `TAGFLOW_SCAN_INTERVAL` 秒遍历所有库；
//! - **首轮立即执行**：服务启动后立即跑一轮（loop 体在前，sleep 在后），数据即最新；
//! - **共享 409 锁**：每库调 [`crate::engine::scanner::scan_library_job`]，与
//!   手动触发共享同一把 `SCANNING` 锁，同库不会并发扫描；
//! - **失败容错**：单库扫描失败/查库失败记日志后继续下一库，不阻塞其他库与后续轮次；
//! - **间隔配置**：[`crate::infra::config::scan_interval_secs`]，缺省 3600s，clamp ≥60s。
//!
//! # 启动方式
//! `main.rs` 用 `tokio::spawn(start_scan_scheduler(pool))` 启动，与 worker spawn 并列。

use std::time::Duration;

use sqlx::SqlitePool;
use tokio::time::sleep;
use tracing::{debug, error, info, warn};

use crate::engine::scanner::{ScanOutcome, scan_library_job};
use crate::infra::config;
use crate::models::db::Library;

/// 启动定时扫描调度器（无限循环，应在独立 Tokio 任务中运行）。
///
/// 流程：立即首轮 → `sleep(interval)` → 循环。每轮：
/// 1. `SELECT * FROM libraries` 查所有库；
/// 2. 对每个库 `await scan_library_job`（共享 409 锁）；
/// 3. 按 [`ScanOutcome`] 记日志；Err 记 error 后继续下一个库。
///
/// 新增/删除库在下一轮重查 DB 时自然感知，无需通知 scheduler。
pub async fn start_scan_scheduler(pool: SqlitePool) {
    let interval_secs = config::scan_interval_secs();
    info!(
        "定时扫描调度器已启动，间隔 {} 秒（首轮立即执行）",
        interval_secs
    );

    loop {
        // 1. 查所有资源库
        match sqlx::query_as::<_, Library>("SELECT * FROM libraries ORDER BY id")
            .fetch_all(&pool)
            .await
        {
            Ok(libs) => {
                debug!("调度器本轮扫描 {} 个资源库", libs.len());
                for lib in libs {
                    // 2. 逐库执行扫描（共享 409 锁，与手动触发互斥）
                    //    scheduler 本身已在后台，无需再 spawn；逐个 await 即可。
                    match scan_library_job(&pool, lib.id).await {
                        Ok(ScanOutcome::Performed) => {
                            debug!("调度器扫描完成: id={}, name={}", lib.id, lib.name);
                        }
                        Ok(ScanOutcome::SkippedConcurrent) => {
                            debug!(
                                "调度器跳过（手动触发或其他任务正在扫描）: id={}, name={}",
                                lib.id, lib.name
                            );
                        }
                        Ok(ScanOutcome::NotFound) => {
                            // 查询列表与逐库扫描之间库被删除的竞态；非问题，记 warn 观测
                            warn!("调度器扫描时资源库已不存在（可能刚被删除）: id={}", lib.id);
                        }
                        Err(e) => {
                            // 单库失败不阻塞其他库与后续轮次
                            error!("调度器扫描失败: id={}, name={}, {}", lib.id, lib.name, e);
                        }
                    }
                }
            }
            Err(e) => {
                // 查库失败：记日志后等下一轮，绝不退出循环
                error!("调度器查询资源库列表失败: {}", e);
            }
        }

        // 3. 等待下一轮
        sleep(Duration::from_secs(interval_secs)).await;
    }
}
