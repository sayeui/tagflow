# 打通扫描流水线：trigger_scan 接入扫描引擎与缩略图任务

## Goal

让用户在 UI 点击「扫描」后真正跑通 扫描→打标→缩略图→浏览 的产品核心闭环。当前 `POST /api/v1/libraries/:id/scan` 返回 501，而扫描引擎、标签引擎、任务表、缩略图 worker 均已就绪，只差接线。

## Requirements

1. **trigger_scan 真实实现**（`api/library.rs`）
   - 校验 library 存在（404）→ 获取扫描锁失败返回 409 → `tokio::spawn` 后台执行 `Scanner::scan_library` → 立即返回 202
   - 并发防护：进程内全局 `Mutex<HashSet<i32>>`（library_id），扫描结束（无论成败）释放
   - 扫描成功后更新 `libraries.last_scanned_at = CURRENT_TIMESTAMP`；失败只记日志不更新
2. **缩略图任务入队**（`engine/scanner/mod.rs`）
   - `insert_file` / `update_file` 中，对媒体扩展名白名单调用现有 `worker::create_thumbnail_task`
   - 白名单（保守，仅 ffmpeg 常规构建确定支持的格式）：图片 jpg/jpeg/png/gif/webp/bmp；视频 mp4/mov/m4v/mkv/avi/webm
   - 注意：扫描/打标/浏览**不限文件类型**，白名单只控制缩略图任务入队
   - 入队前用 `has_pending_thumbnail_task` 防重
3. **丢失文件恢复修正**（`engine/scanner/mod.rs`）
   - 差异对比命中且 size/mtime 一致时，若该文件此前 status=0（曾标记丢失），需恢复 status=1（实现上可在快照中带出 status，或对命中路径统一置 1）
4. **浏览过滤丢失文件**（`api/file.rs`）
   - `list_files` 的三条查询全部增加 `f.status = 1` 条件
5. **端口可配置**（`main.rs`，验证阶段经用户确认追加）
   - 监听端口支持 `TAGFLOW_PORT` 环境变量覆盖，默认 8080；无效值回退默认并 warn

## Acceptance Criteria

* [x] UI 点击扫描 → 返回 202 → 文件入库 → 标签树出现层级路径标签 → 缩略图陆续生成 → Home 可浏览（2026-06-12 API 级端到端验证通过）
* [x] 扫描进行中重复点击同库扫描返回 409，不产生并发双扫；不同库可并行
* [x] 扫描成功后 `last_scanned_at` 刷新并在 Libraries 页显示
* [x] 删除磁盘文件后重扫，该文件从浏览结果消失；恢复同一文件（size/mtime 不变）后重扫，文件重新出现（日志命中 restore 路径）
* [x] 非媒体文件（如 .txt）不产生 thumb 任务；同一文件不重复入队
* [x] 扫描失败（权限拒绝模拟）：error 日志记录、`last_scanned_at` 不更新、锁释放、可再次触发扫描

## Definition of Done

* `cargo fmt && cargo clippy && cargo test` 通过
* 单元测试：媒体扩展名判定、（可测的）锁获取/释放逻辑
* 真实目录手动端到端验证（含删除/恢复文件场景）

## Technical Approach

```
trigger_scan (202)
  └─ tokio::spawn
       ├─ Scanner::scan_library          # 已有：OpenDAL 遍历 + 差异对比
       │    ├─ insert_file → PathTagger  # 已有：层级打标
       │    │    └─ create_thumbnail_task(媒体白名单)   # 新增
       │    ├─ update_file → 同上入队 + status=1        # 修改
       │    └─ 命中未变更 → 恢复 status=1               # 修正
       ├─ UPDATE libraries.last_scanned_at             # 新增（成功时）
       └─ 释放 SCANNING 锁                              # 新增
worker（已有）轮询 tasks → ffmpeg 生成 ./cache/{id}.webp
list_files 增加 f.status = 1 过滤                       # 修改
```

锁实现：`std::sync::Mutex<HashSet<i32>>`（`once_cell`/`LazyLock` 静态），临界区仅插入/移除，无跨 await 持锁。

## Decision (ADR-lite)

**Context**: 扫描需异步执行，可选 handler 内 spawn 或扩展 tasks 表统一队列。
**Decision**: `tokio::spawn` + 进程内 HashSet 防重（用户选定）。缩略图扫描中逐文件入队；status 过滤纳入本任务。
**Consequences**: 无迁移、改动最小、与 main.rs spawn worker 风格一致；代价是扫描状态不持久化（进程重启丢失，但扫描增量幂等，重扫无害）。未来若需进度上报/断点续扫，再演进为持久化任务模型。

## Out of Scope

* WebDAV 协议支持
* 扫描进度百分比 / WebSocket/SSE 实时推送
* 定时自动扫描
* 丢失文件的 UI 特殊展示（置灰等）
* 缩略图清理（文件删除后清理 cache 中的 webp）

## Future Iterations (记录备查，非本任务)

* **缩略图格式扩充**：svg（需 librsvg 构建的 ffmpeg 或改用专门库）、heic/tiff（取决于 ffmpeg 编译选项）、RAW 照片（cr2/nef/arw 等）、flv/wmv/ts 视频；扩充时白名单应收敛为单一常量 + 单元测试，必要时启动期探测 ffmpeg 能力
* 白名单做成可配置项（配置文件/环境变量）

## Technical Notes

* 已检查文件：`api/library.rs`（trigger_scan 501 桩）、`engine/scanner/mod.rs`（完整差异扫描）、`engine/tagger/mod.rs`、`engine/worker.rs`（create_thumbnail_task / has_pending_thumbnail_task 已备）、`infra/thumbnail.rs`（ffmpeg 出 webp，图片视频通吃）、`migrations/202512300003_create_tasks.sql`（file_id NOT NULL，故未选方案 B）、`views/settings/Libraries.vue`（triggerScan 已接好，前端无需改动）
* 后端规范：`.trellis/spec/backend/`（错误处理 StatusCode 映射、worker 永不退出、tracing 中文日志、禁止 unwrap）
* 注意 `files.status` 语义：1=正常，0=丢失（scanner 现状如此）

## 验证阶段发现并修复的存量 Bug（infra/thumbnail.rs，M8 遗留）

1. **路径拼接缺分隔符**：`base_path + parent_path` 直接拼接产生 `/data` + `Projects/` = `/dataProjects/`；已抽出 `build_source_path` 并加单测
2. **静态图片缩略图必失败**：`-ss 00:00:00.5` 对单帧图片 seek 掉唯一帧；已改为仅视频扩展名（VIDEO_EXTENSIONS，与 scanner 白名单一致）加 seek
3. **失败残留 0 字节 webp 污染缓存**：重试时被「已存在」误判成功、API 返回 200 空图；已改为存在性检查要求非零字节（自愈再生）+ 失败时清理输出文件

## 验证阶段行为记录（非 bug，备查）

* OpenDAL Fs 服务对不存在的 base_path 会**自动创建目录**并以 0 文件「扫描成功」——与「非侵入式」定位略有张力，未来可在创建资源库时强制校验路径存在（test 接口已有该校验，create 接口没有）
* 前端 Libraries.vue 对 409 显示通用「启动扫描失败」，原 501 分支已成死代码——留作后续小改进
