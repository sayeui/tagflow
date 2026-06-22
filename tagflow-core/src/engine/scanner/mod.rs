use crate::core::tag::TagManager;
use crate::engine::tagger;
use crate::engine::worker;
use crate::infra::storage::StorageManager;
use crate::models::db::Library;
use futures_util::stream::StreamExt;
use sqlx::SqlitePool;
use std::collections::{HashMap, HashSet};
use std::sync::{LazyLock, Mutex};
use tracing::{debug, error, info};

/// 正在扫描的资源库 ID 集合（进程内并发防护）。
///
/// 历史位置：原本内联在 `api/library.rs::trigger_scan`，现收敛到 engine 层，
/// 让手动触发（`trigger_scan`）与定时调度（`engine::scheduler`）共享同一把锁，
/// 避免两套机制对同一资源库并发扫描（参见 code-reuse-thinking-guide.md
/// 「不对称机制产生同一输出」的反例预防）。
///
/// 注意：这是进程内锁，重启丢失；与重启重新首轮扫描的设计相容。
static SCANNING: LazyLock<Mutex<HashSet<i32>>> = LazyLock::new(|| Mutex::new(HashSet::new()));

/// `scan_library_job` 的执行结果，供调用方（手动触发、scheduler）做不同的响应映射。
#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub enum ScanOutcome {
    /// 成功完成了一次扫描
    Performed,
    /// 同库已有扫描进行中，本次跳过（手动触发对应 HTTP 409）
    SkippedConcurrent,
    /// 资源库不存在（手动触发对应 HTTP 404）
    NotFound,
}

/// 尝试同步获取某资源库的扫描锁。
///
/// **同步**返回（不跨 await），用于 `trigger_scan` 在 HTTP 请求路径上立即判定
/// 是否返 409——调用方无需进入后台 spawn 即可拒绝并发请求。
///
/// - 返回 `true`：锁被本调用获取，调用方有义务在结束后调用 [`release_scan_lock`]；
/// - 返回 `false`：同库扫描进行中，调用方应拒绝（409）。
///
/// 锁内部使用 `unwrap_or_else(|e| e.into_inner())` 恢复 poisoned mutex，
/// 避免某次扫描 panic 后整把锁永久失效（与 trigger_scan 历史行为一致）。
pub fn try_acquire_scan_lock(library_id: i32) -> bool {
    let mut scanning = SCANNING.lock().unwrap_or_else(|e| e.into_inner());
    scanning.insert(library_id)
}

/// 释放某资源库的扫描锁。
///
/// 仅在 [`try_acquire_scan_lock`] 返回 `true` 后调用；对未持有锁的 id 调用是空操作
/// （`HashSet::remove` 对不存在的元素返回 false，不报错）。
pub fn release_scan_lock(library_id: i32) {
    SCANNING
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .remove(&library_id);
}

/// 执行一次资源库增量扫描（共享入口）。
///
/// 这是定时调度（`engine::scheduler::start_scan_scheduler`）使用的扫描函数，
/// 封装：
/// 1. 同步尝试加 409 锁 → 失败返回 [`ScanOutcome::SkippedConcurrent`]；
/// 2. 锁持有期间调 [`run_scan_with_lock_held`] 执行实际扫描；
/// 3. **无论结果如何都释放锁**（保证不泄漏）。
///
/// 手动触发（`trigger_scan`）因需在 HTTP 请求路径上同步返 409，**不**调本函数，
/// 而是直接调 [`try_acquire_scan_lock`] 同步判定，再 spawn 后台任务调
/// [`run_scan_with_lock_held`]——两者共用同一份「查库 → 扫描 → 更新时间」实现。
pub async fn scan_library_job(pool: &SqlitePool, library_id: i32) -> anyhow::Result<ScanOutcome> {
    // 1. 同步 try lock（与 trigger_scan 共享同一把 SCANNING 锁）
    if !try_acquire_scan_lock(library_id) {
        return Ok(ScanOutcome::SkippedConcurrent);
    }

    // 2. 锁已持有，交由共享内部函数执行
    let outcome = run_scan_with_lock_held(pool, library_id).await;

    // 3. 无论 outcome 如何（包括 NotFound / Err），本函数负责释放锁
    release_scan_lock(library_id);
    outcome
}

/// 在调用方已持有扫描锁的前提下执行扫描主体（不 try lock，也不释放锁）。
///
/// **锁契约**：
/// - 调用前，调用方必须已通过 [`try_acquire_scan_lock`] 拿到 `library_id` 的锁；
/// - 调用后，**锁仍由调用方持有**，调用方负责在所有路径释放
///   （[`scan_library_job`] 与 `trigger_scan` 的 spawn 闭包都在 await 后调
///   [`release_scan_lock`]）。
///
/// 这样拆分让 trigger_scan 能在 HTTP 请求路径上同步拿到 409 信号，同时与
/// scheduler 共用同一份「查库 → 扫描 → 更新时间」的实现，杜绝复制粘贴
/// （code-reuse-thinking-guide.md 核心）。
pub async fn run_scan_with_lock_held(
    pool: &SqlitePool,
    library_id: i32,
) -> anyhow::Result<ScanOutcome> {
    // 1. 查库
    let library: Library =
        match sqlx::query_as::<_, Library>("SELECT * FROM libraries WHERE id = ?")
            .bind(library_id)
            .fetch_optional(pool)
            .await
        {
            Ok(Some(lib)) => lib,
            Ok(None) => return Ok(ScanOutcome::NotFound),
            Err(e) => return Err(anyhow::anyhow!("查询资源库失败: id={}, {}", library_id, e)),
        };

    // 2. 执行扫描
    let scanner = Scanner::new(pool.clone());
    let scan_result = scanner.scan_library(&library).await;

    // 3. 按结果更新 last_scanned_at / 记日志
    match scan_result {
        Ok(_) => {
            match sqlx::query(
                "UPDATE libraries SET last_scanned_at = CURRENT_TIMESTAMP WHERE id = ?",
            )
            .bind(library_id)
            .execute(pool)
            .await
            {
                Ok(_) => info!("资源库扫描成功: id={}, name={}", library_id, library.name),
                Err(e) => error!("更新扫描时间失败: id={}, {}", library_id, e),
            }
            Ok(ScanOutcome::Performed)
        }
        Err(e) => {
            error!(
                "资源库扫描失败: id={}, name={}, {}",
                library_id, library.name, e
            );
            Err(e)
        }
    }
}

/// 缩略图任务媒体扩展名白名单（仅控制缩略图入队，不限制扫描/打标/浏览）
const MEDIA_EXTENSIONS: &[&str] = &[
    // 图片
    "jpg", "jpeg", "png", "gif", "webp", "bmp", // 视频
    "mp4", "mov", "m4v", "mkv", "avi", "webm",
];

/// 判断扩展名是否在缩略图媒体白名单中（大小写不敏感）
fn is_media_extension(ext: &str) -> bool {
    let ext = ext.to_lowercase();
    MEDIA_EXTENSIONS.contains(&ext.as_str())
}

pub struct Scanner {
    db: SqlitePool,
}

impl Scanner {
    pub fn new(db: SqlitePool) -> Self {
        Self { db }
    }

    /// 执行扫描主逻辑
    pub async fn scan_library(&self, library: &Library) -> anyhow::Result<()> {
        info!("开始扫描资源库: {}", library.name);
        let op = StorageManager::get_operator(library)?;

        // 1. 获取数据库快照 (Path -> (Size, MTime, Status))
        let snapshot = self.get_db_snapshot(library.id).await?;
        let mut remote_paths = snapshot; // 用于追踪哪些文件还在

        // 2. 递归遍历物理文件
        let mut lister = op.lister_with("/").recursive(true).await?;

        while let Some(entry) = lister.next().await {
            let entry = entry?;
            let metadata = op.stat(entry.path()).await?;

            if metadata.is_dir() {
                continue;
            }

            let path = entry.path().to_string();
            let size = metadata.content_length() as i64;
            let mtime = metadata.last_modified().map(|t| t.timestamp()).unwrap_or(0);

            // 3. 差异对比
            if let Some((db_size, db_mtime, db_status)) = remote_paths.remove(&path) {
                if db_size != size || db_mtime != mtime {
                    // 文件已修改
                    self.update_file(library.id, &path, size, mtime).await?;
                } else if db_status == 0 {
                    // 内容未变更但曾标记丢失，恢复为正常
                    self.restore_file(library.id, &path).await?;
                }
                // status=1 且未变更，则什么都不做
            } else {
                // 新增文件
                self.insert_file(library.id, &path, size, mtime).await?;
            }
        }

        // 4. 清理阶段：remote_paths 中剩余的即为物理上已删除的文件
        for (deleted_path, _) in remote_paths {
            self.mark_as_lost(library.id, &deleted_path).await?;
        }

        info!("资源库 {} 扫描完成", library.name);
        Ok(())
    }

    // --- 数据库操作辅助函数 ---

    async fn get_db_snapshot(
        &self,
        lib_id: i32,
    ) -> anyhow::Result<HashMap<String, (i64, i64, i32)>> {
        let rows: Vec<(String, i64, i64, i32)> = sqlx::query_as(
            "SELECT parent_path || filename as path, size, mtime, status FROM files WHERE library_id = ?"
        )
        .bind(lib_id)
        .fetch_all(&self.db)
        .await?;

        Ok(rows
            .into_iter()
            .map(|(p, s, m, st)| (p, (s, m, st)))
            .collect())
    }

    async fn insert_file(
        &self,
        lib_id: i32,
        full_path: &str,
        size: i64,
        mtime: i64,
    ) -> anyhow::Result<()> {
        let (parent, filename) = self.split_path(full_path);
        let ext = filename.split('.').next_back().map(|s| s.to_lowercase());

        // 1. 插入文件记录
        let res = sqlx::query(
            "INSERT INTO files (library_id, parent_path, filename, extension, size, mtime) VALUES (?, ?, ?, ?, ?, ?)"
        )
        .bind(lib_id).bind(&parent).bind(&filename).bind(&ext).bind(size).bind(mtime)
        .execute(&self.db).await?;

        let file_id = res.last_insert_rowid() as i32;

        // 2. 触发标签化（多维度 tagger 流水线）
        let tag_mgr = TagManager::new(self.db.clone());
        tagger::run_all(&tag_mgr, file_id, &parent, ext.as_deref(), mtime).await?;

        // 3. 媒体文件入队缩略图任务
        self.maybe_enqueue_thumbnail(file_id, ext.as_deref())
            .await?;

        Ok(())
    }

    async fn update_file(
        &self,
        lib_id: i32,
        full_path: &str,
        size: i64,
        mtime: i64,
    ) -> anyhow::Result<()> {
        let (parent, filename) = self.split_path(full_path);
        sqlx::query(
            "UPDATE files SET size = ?, mtime = ?, status = 1 WHERE library_id = ? AND parent_path = ? AND filename = ?"
        )
        .bind(size).bind(mtime).bind(lib_id).bind(&parent).bind(&filename)
        .execute(&self.db).await?;

        // 内容已变更，重新入队缩略图任务
        let file_id: Option<i32> = sqlx::query_scalar(
            "SELECT id FROM files WHERE library_id = ? AND parent_path = ? AND filename = ?",
        )
        .bind(lib_id)
        .bind(&parent)
        .bind(&filename)
        .fetch_optional(&self.db)
        .await?;

        if let Some(file_id) = file_id {
            let ext = filename.split('.').next_back().map(|s| s.to_lowercase());
            self.maybe_enqueue_thumbnail(file_id, ext.as_deref())
                .await?;
        }
        Ok(())
    }

    /// 恢复曾标记丢失的文件
    async fn restore_file(&self, lib_id: i32, full_path: &str) -> anyhow::Result<()> {
        let (parent, filename) = self.split_path(full_path);
        sqlx::query(
            "UPDATE files SET status = 1 WHERE library_id = ? AND parent_path = ? AND filename = ?",
        )
        .bind(lib_id)
        .bind(&parent)
        .bind(&filename)
        .execute(&self.db)
        .await?;
        debug!("恢复丢失文件: library_id={}, path={}", lib_id, full_path);
        Ok(())
    }

    async fn mark_as_lost(&self, lib_id: i32, full_path: &str) -> anyhow::Result<()> {
        let (parent, filename) = self.split_path(full_path);
        sqlx::query(
            "UPDATE files SET status = 0 WHERE library_id = ? AND parent_path = ? AND filename = ?",
        )
        .bind(lib_id)
        .bind(parent)
        .bind(filename)
        .execute(&self.db)
        .await?;
        Ok(())
    }

    /// 媒体白名单内的文件入队缩略图任务（带防重检查）
    async fn maybe_enqueue_thumbnail(&self, file_id: i32, ext: Option<&str>) -> anyhow::Result<()> {
        let Some(ext) = ext else { return Ok(()) };
        if !is_media_extension(ext) {
            return Ok(());
        }

        if worker::has_pending_thumbnail_task(&self.db, file_id).await? {
            debug!("缩略图任务已存在，跳过入队: file_id={}", file_id);
            return Ok(());
        }

        worker::create_thumbnail_task(&self.db, file_id, None).await?;
        debug!("缩略图任务已入队: file_id={}", file_id);
        Ok(())
    }

    fn split_path(&self, full_path: &str) -> (String, String) {
        let path = std::path::Path::new(full_path);
        let parent = path
            .parent()
            .and_then(|p| p.to_str())
            .unwrap_or("")
            .to_string();
        let filename = path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("")
            .to_string();
        // 保证 parent 以 / 结尾或为空
        let parent = if parent.is_empty() {
            parent
        } else {
            format!("{}/", parent)
        };
        (parent, filename)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_media_extension_images() {
        for ext in ["jpg", "jpeg", "png", "gif", "webp", "bmp"] {
            assert!(is_media_extension(ext), "应识别图片扩展名: {}", ext);
        }
    }

    #[test]
    fn test_media_extension_videos() {
        for ext in ["mp4", "mov", "m4v", "mkv", "avi", "webm"] {
            assert!(is_media_extension(ext), "应识别视频扩展名: {}", ext);
        }
    }

    #[test]
    fn test_media_extension_rejects_non_media() {
        for ext in ["txt", "svg", "pdf", "heic", "rs", ""] {
            assert!(!is_media_extension(ext), "不应识别非白名单扩展名: {}", ext);
        }
    }

    #[test]
    fn test_media_extension_case_insensitive() {
        assert!(is_media_extension("JPG"));
        assert!(is_media_extension("Mp4"));
        assert!(is_media_extension("WebM"));
    }

    #[test]
    fn test_scan_lock_acquire_and_release() {
        // 用一个极不可能冲突的 id 做隔离，并在任何路径下都清理
        let test_id = i32::MAX - 9_999;
        // 前置清理：保证上一轮失败用例残留不影响本轮
        release_scan_lock(test_id);

        // 首次获取成功
        assert!(try_acquire_scan_lock(test_id));
        // 同 id 再次获取应失败（已持有）
        assert!(!try_acquire_scan_lock(test_id));
        // 释放后可再次获取
        release_scan_lock(test_id);
        assert!(try_acquire_scan_lock(test_id));
        release_scan_lock(test_id);

        // 对未持有的 id 调用 release 不应 panic
        release_scan_lock(test_id);
    }
}
