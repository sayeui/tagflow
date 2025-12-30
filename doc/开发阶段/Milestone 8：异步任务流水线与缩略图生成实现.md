进入 **Milestone 8：异步任务流水线与缩略图生成实现**。

在这一阶段，我们将实现系统的后台动力系统。缩略图生成是一个耗时且消耗 CPU 的操作，不能在扫描或 API 请求时同步进行。我们将建立一个基于数据库的任务队列，通过后台 Worker 异步生成缩略图，并将其存储在非侵入式的缓存目录中。

### 1. 实现后台任务调度器 (engine/worker.rs)

我们需要一个在后台循环运行的 Worker，它会不断检查 `tasks` 表，处理未完成的任务。

```rust
use sqlx::SqlitePool;
use std::time::Duration;
use tokio::time::sleep;
use crate::infra::thumbnail::ThumbnailGenerator;

pub async fn start_task_worker(pool: SqlitePool, cache_dir: String) {
    let generator = ThumbnailGenerator::new(cache_dir);
    
    tracing::info!("异步任务 Worker 已启动");

    loop {
        // 1. 获取一个待处理任务
        let task = sqlx::query!(
            "SELECT id, file_id, task_type FROM tasks WHERE status = 0 ORDER BY priority DESC, id ASC LIMIT 1"
        )
        .fetch_optional(&pool)
        .await;

        match task {
            Ok(Some(t)) => {
                // 2. 更新状态为进行中 (1)
                let _ = sqlx::query!("UPDATE tasks SET status = 1 WHERE id = ?", t.id)
                    .execute(&pool).await;

                // 3. 执行任务逻辑 (此处仅展示缩略图)
                if t.task_type == "thumb" {
                    match generator.generate_for_file(t.file_id, &pool).await {
                        Ok(_) => {
                            let _ = sqlx::query!("UPDATE tasks SET status = 2 WHERE id = ?", t.id)
                                .execute(&pool).await;
                        }
                        Err(e) => {
                            let _ = sqlx::query!(
                                "UPDATE tasks SET status = 3, error_msg = ? WHERE id = ?",
                                e.to_string(), t.id
                            ).execute(&pool).await;
                        }
                    }
                }
            }
            _ => {
                // 没有任务，休眠一段时间
                sleep(Duration::from_secs(5)).await;
            }
        }
    }
}
```

### 2. 实现缩略图生成逻辑 (infra/thumbnail.rs)

我们将使用 `image` 库处理图片，并调用系统 `ffmpeg` 处理视频（确保 Docker 镜像中包含 ffmpeg）。

```rust
use std::process::Command;
use sqlx::SqlitePool;
use std::path::Path;

pub struct ThumbnailGenerator {
    cache_dir: String,
}

impl ThumbnailGenerator {
    pub fn new(cache_dir: String) -> Self {
        // 确保缓存目录存在
        let _ = std::fs::create_dir_all(&cache_dir);
        Self { cache_dir }
    }

    pub async fn generate_for_file(&self, file_id: i32, pool: &SqlitePool) -> anyhow::Result<()> {
        // 获取文件物理路径
        let file = sqlx::query!(
            "SELECT f.parent_path, f.filename, l.base_path FROM files f 
             JOIN libraries l ON f.library_id = l.id WHERE f.id = ?",
            file_id
        ).fetch_one(pool).await?;

        let full_path = format!("{}{}{}", file.base_path, file.parent_path, file.filename);
        let output_path = format!("{}/{}.webp", self.cache_dir, file_id);

        if Path::new(&full_path).exists() {
            // 调用 ffmpeg 生成 256px 的缩略图 (兼容视频和图片)
            // 逻辑：提取第一帧并缩放
            let status = Command::new("ffmpeg")
                .args([
                    "-y",
                    "-i", &full_path,
                    "-ss", "00:00:01", // 视频跳过前1秒
                    "-vframes", "1",
                    "-vf", "scale=256:-1",
                    "-q:v", "80",      // WebP 质量
                    &output_path
                ])
                .status()?;

            if status.success() {
                return Ok(());
            }
        }
        anyhow::bail!("生成缩略图失败")
    }
}
```

### 3. 提供缩略图服务接口 (api/file.rs)

前端不能直接访问磁盘文件，我们需要一个 API 来流式传输缩略图。

```rust
use ax_fs::ServeFile; // 假设使用相关库或手动实现
use axum::response::IntoResponse;
use axum::extract::Path;

pub async fn get_thumbnail(
    Path(file_id): Path<i32>
) -> impl IntoResponse {
    let path = format!("./cache/{}.webp", file_id);
    if std::path::Path::new(&path).exists() {
        // 返回缩略图文件
        let file = tokio::fs::File::open(path).await.unwrap();
        let stream = tokio_util::io::ReaderStream::new(file);
        axum::body::Body::from_stream(stream).into_response()
    } else {
        // 返回占位符或 404
        axum::http::StatusCode::NOT_FOUND.into_response()
    }
}
```

### 4. 前端集成展示 (Vue 3)

修改 `FileGrid.vue`，将占位图标替换为真实的缩略图 URL。

```vue
<!-- components/FileGrid.vue 关键片段 -->
<template>
  <!-- ... 之前的代码 -->
  <div class="w-24 h-24 flex items-center justify-center bg-gray-50 rounded mb-2 overflow-hidden">
    <!-- 优先显示缩略图 -->
    <img 
      :src="`/api/v1/files/${file.id}/thumbnail`" 
      class="w-full h-full object-cover"
      @error="(e) => (e.target.style.display = 'none')" 
    />
    <!-- 备用图标 -->
    <FileText class="text-gray-400 w-12 h-12 absolute -z-10" />
  </div>
  <!-- ... -->
</template>
```

### 5. 在主程序中启动 Worker

在 `main.rs` 中启动后台线程。

```rust
#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let pool = infra::db::init_db("sqlite:tagflow.db?mode=rwc").await?;
    
    // 1. 启动任务 Worker
    let pool_for_worker = pool.clone();
    tokio::spawn(async move {
        engine::worker::start_task_worker(pool_for_worker, "./cache".to_string()).await;
    });

    // 2. 路由增加缩略图接口
    let app = Router::new()
        .route("/api/v1/files/:id/thumbnail", get(api::file::get_thumbnail))
        // ... 其他路由
        .with_state(pool);

    // ... 启动服务器
}
```

### 6. Milestone 8 交付点

1.  **异步生成：** 实现了任务队列，缩略图生成不会阻塞扫描引擎或 API 响应。
2.  **非侵入性：** 缩略图全部存储在指定的缓存目录，用户原始数据目录保持洁净。
3.  **多媒体支持：** 通过 FFmpeg 同时支持了图片预览和视频封面提取。
4.  **无缝体验：** 前端通过简单的 URL 即可加载缩略图，具备占位图回退机制。
