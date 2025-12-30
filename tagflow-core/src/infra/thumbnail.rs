//! 缩略图生成模块
//!
//! 使用 FFmpeg 为图片和视频生成缩略图

use std::path::Path;
use std::process::Command;
use sqlx::{SqlitePool, Row};
use tracing::{debug, warn, error, info};

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
             JOIN libraries l ON f.library_id = l.id WHERE f.id = ?"
        )
        .bind(file_id)
        .fetch_one(pool)
        .await
        .map_err(|e| anyhow::anyhow!("查询文件失败: {}", e))?;

        let base_path: &str = row.try_get("base_path")?;
        let parent_path: &str = row.try_get("parent_path")?;
        let filename: &str = row.try_get("filename")?;

        // 构建完整路径
        let full_path = format!("{}{}{}", base_path, parent_path, filename);
        let output_path = format!("{}/{}.webp", self.cache_dir, file_id);

        // 检查源文件是否存在
        if !Path::new(&full_path).exists() {
            warn!("源文件不存在: {}", full_path);
            anyhow::bail!("源文件不存在: {}", full_path);
        }

        // 检查缩略图是否已存在
        if Path::new(&output_path).exists() {
            debug!("缩略图已存在: {}", output_path);
            return Ok(());
        }

        // 调用 FFmpeg 生成缩略图
        let result = self.generate_thumbnail_ffmpeg(&full_path, &output_path);

        match result {
            Ok(_) => {
                info!("缩略图生成成功: {} -> {}", full_path, output_path);
                Ok(())
            }
            Err(e) => {
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

        let output = Command::new("ffmpeg")
            .args([
                "-y",                              // 覆盖已存在的文件
                "-i", input_path,                  // 输入文件
                "-ss", "00:00:00.5",              // 视频: 从 0.5 秒开始提取帧
                "-vframes", "1",                   // 只提取一帧
                "-vf", "scale=256:256:force_original_aspect_ratio=decrease",  // 缩放到 256x256
                "-q:v", "80",                      // WebP 质量 (0-100)
                output_path
            ])
            .output();

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
}
