use crate::core::tag::TagManager;
use crate::engine::tagger::PathTagger;
use crate::engine::worker;
use crate::infra::storage::StorageManager;
use crate::models::db::Library;
use futures_util::stream::StreamExt;
use sqlx::SqlitePool;
use std::collections::HashMap;
use tracing::{debug, info};

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

        // 2. 触发标签化 (Milestone 3 核心)
        let tag_mgr = TagManager::new(self.db.clone());
        let path_tagger = PathTagger::new(tag_mgr);
        path_tagger.process_path(file_id, &parent).await?;

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
}
