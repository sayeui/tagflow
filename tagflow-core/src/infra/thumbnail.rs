//! 缩略图生成模块
//!
//! 使用 FFmpeg 为图片和视频生成缩略图

use sqlx::{Row, SqlitePool};
use std::path::Path;
use std::process::Command;
use tracing::{debug, error, info, warn};

/// 视频扩展名白名单（必须与 `engine/scanner/mod.rs` 的 MEDIA_EXTENSIONS 中视频部分保持一致）
const VIDEO_EXTENSIONS: &[&str] = &["mp4", "mov", "m4v", "mkv", "avi", "webm"];

/// 根据扩展名判断是否为视频文件（大小写不敏感）
///
/// 静态图片只有 t=0 一帧，FFmpeg 加 `-ss` 会丢弃唯一帧导致生成失败，
/// 因此需要区分视频与图片以决定是否使用时间偏移
fn is_video_extension(path: &str) -> bool {
    Path::new(path)
        .extension()
        .and_then(|e| e.to_str())
        .map(|e| VIDEO_EXTENSIONS.contains(&e.to_lowercase().as_str()))
        .unwrap_or(false)
}

/// 拼接源文件完整路径
///
/// `base_path` 不含末尾斜杠（兼容含斜杠的情况），`parent_path` 为库内相对路径
/// （不含开头斜杠、含末尾斜杠，根目录文件为空字符串）
fn build_source_path(base_path: &str, parent_path: &str, filename: &str) -> String {
    format!(
        "{}/{}{}",
        base_path.trim_end_matches('/'),
        parent_path,
        filename
    )
}

/// 缩略图生成器
pub struct ThumbnailGenerator {
    cache_dir: String,
}

impl ThumbnailGenerator {
    /// 创建新的缩略图生成器
    ///
    /// # 参数
    /// - `cache_dir`: 缩略图缓存目录路径
    pub fn new(cache_dir: String) -> Self {
        // 确保缓存目录存在
        if let Err(e) = std::fs::create_dir_all(&cache_dir) {
            error!("无法创建缓存目录 {}: {}", cache_dir, e);
        }
        info!("缩略图生成器已初始化，缓存目录: {}", cache_dir);
        Self { cache_dir }
    }

    /// 为指定文件生成缩略图
    ///
    /// # 参数
    /// - `file_id`: 文件 ID
    /// - `pool`: 数据库连接池
    ///
    /// # 返回
    /// - `Ok(())`: 生成成功
    /// - `Err(anyhow::Error)`: 生成失败
    pub async fn generate_for_file(&self, file_id: i32, pool: &SqlitePool) -> anyhow::Result<()> {
        debug!("开始为文件 {} 生成缩略图", file_id);

        // 获取文件物理路径 (使用运行时检查)
        let row = sqlx::query(
            "SELECT f.parent_path, f.filename, l.base_path FROM files f
             JOIN libraries l ON f.library_id = l.id WHERE f.id = ?",
        )
        .bind(file_id)
        .fetch_one(pool)
        .await
        .map_err(|e| anyhow::anyhow!("查询文件失败: {}", e))?;

        let base_path: &str = row.try_get("base_path")?;
        let parent_path: &str = row.try_get("parent_path")?;
        let filename: &str = row.try_get("filename")?;

        // 构建完整路径
        let full_path = build_source_path(base_path, parent_path, filename);
        let output_path = format!("{}/{}.webp", self.cache_dir, file_id);

        // 检查源文件是否存在
        if !Path::new(&full_path).exists() {
            warn!("源文件不存在: {}", full_path);
            anyhow::bail!("源文件不存在: {}", full_path);
        }

        // 检查缩略图是否已存在（0 字节视为损坏，重新生成以自愈）
        let existing_valid = std::fs::metadata(&output_path)
            .map(|m| m.len() > 0)
            .unwrap_or(false);
        if existing_valid {
            debug!("缩略图已存在: {}", output_path);
            return Ok(());
        }

        // 调用 FFmpeg 生成缩略图
        let result = self.generate_thumbnail_ffmpeg(&full_path, &output_path);

        match result {
            Ok(_) => {
                // 防御性检查：FFmpeg 退出码为 0 但输出为空时视为失败
                let output_valid = std::fs::metadata(&output_path)
                    .map(|m| m.len() > 0)
                    .unwrap_or(false);
                if !output_valid {
                    let _ = std::fs::remove_file(&output_path);
                    error!("缩略图生成失败: FFmpeg 输出为空: {}", output_path);
                    anyhow::bail!("FFmpeg 输出为空: {}", output_path);
                }
                info!("缩略图生成成功: {} -> {}", full_path, output_path);
                Ok(())
            }
            Err(e) => {
                // 清理 FFmpeg 失败时遗留的 0 字节输出文件，避免污染缓存
                let _ = std::fs::remove_file(&output_path);
                error!("缩略图生成失败: {}", e);
                Err(e)
            }
        }
    }

    /// 使用 FFmpeg 生成缩略图
    ///
    /// # 参数
    /// - `input_path`: 输入文件路径
    /// - `output_path`: 输出缩略图路径
    fn generate_thumbnail_ffmpeg(&self, input_path: &str, output_path: &str) -> anyhow::Result<()> {
        debug!("调用 FFmpeg: {} -> {}", input_path, output_path);

        let mut cmd = Command::new("ffmpeg");
        cmd.args(["-y", "-i", input_path]);
        // 仅视频使用时间偏移取帧；静态图片只有 t=0 一帧，加 -ss 会导致无帧可输出
        if is_video_extension(input_path) {
            cmd.args(["-ss", "00:00:00.5"]);
        }
        cmd.args([
            "-vframes",
            "1", // 只提取一帧
            "-vf",
            "scale=256:256:force_original_aspect_ratio=decrease", // 缩放到 256x256
            "-q:v",
            "80", // WebP 质量 (0-100)
            output_path,
        ]);

        let output = cmd.output();

        match output {
            Ok(result) => {
                if result.status.success() {
                    debug!("FFmpeg 执行成功");
                    Ok(())
                } else {
                    let stderr = String::from_utf8_lossy(&result.stderr);
                    warn!("FFmpeg 执行失败: {}", stderr);
                    anyhow::bail!("FFmpeg 执行失败: {}", stderr)
                }
            }
            Err(e) => {
                // FFmpeg 可能未安装
                error!("无法执行 FFmpeg: {}", e);
                anyhow::bail!("无法执行 FFmpeg，请确保已安装: {}", e)
            }
        }
    }

    /// 获取缩略图文件路径
    ///
    /// # 参数
    /// - `file_id`: 文件 ID
    ///
    /// # 返回
    /// 缩略图文件路径
    pub fn get_thumbnail_path(&self, file_id: i32) -> String {
        format!("{}/{}.webp", self.cache_dir, file_id)
    }

    /// 检查缩略图是否存在
    ///
    /// # 参数
    /// - `file_id`: 文件 ID
    ///
    /// # 返回
    /// - `true`: 存在
    /// - `false`: 不存在
    pub fn thumbnail_exists(&self, file_id: i32) -> bool {
        let path = self.get_thumbnail_path(file_id);
        Path::new(&path).exists()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_thumbnail_path() {
        let generator = ThumbnailGenerator::new("./cache".to_string());
        assert_eq!(generator.get_thumbnail_path(123), "./cache/123.webp");
    }

    #[test]
    fn test_is_video_extension() {
        // 视频扩展名（含大小写混合）
        assert!(is_video_extension("/tmp/a.mp4"));
        assert!(is_video_extension("/tmp/b.MOV"));
        assert!(is_video_extension("clip.WebM"));
        assert!(is_video_extension("/tmp/dir/c.mkv"));
        // 图片扩展名不属于视频
        assert!(!is_video_extension("/tmp/a.jpg"));
        assert!(!is_video_extension("/tmp/a.PNG"));
        assert!(!is_video_extension("/tmp/a.webp"));
        // 无扩展名 / 非媒体
        assert!(!is_video_extension("/tmp/noext"));
        assert!(!is_video_extension("/tmp/a.txt"));
    }

    #[test]
    fn test_build_source_path() {
        // base_path 无末尾斜杠 + 嵌套目录
        assert_eq!(
            build_source_path("/tmp/data", "Projects/2024/", "poster.png"),
            "/tmp/data/Projects/2024/poster.png"
        );
        // base_path 含末尾斜杠
        assert_eq!(
            build_source_path("/tmp/data/", "Projects/2024/", "poster.png"),
            "/tmp/data/Projects/2024/poster.png"
        );
        // 根目录文件（parent_path 为空）
        assert_eq!(
            build_source_path("/tmp/data", "", "poster.png"),
            "/tmp/data/poster.png"
        );
    }
}
