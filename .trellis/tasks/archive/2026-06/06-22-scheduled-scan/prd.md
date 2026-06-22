# 定时增量扫描（v0.2.0 自动同步）

## Goal

后台定时自动增量扫描所有资源库，替代当前手动 `POST /libraries/:id/scan` 触发，让文件增删改自动同步进库（缩略图任务照常入队 worker）。v0.2.0 Beta「多源接入与自动同步」第一块，为 WebDAV 接入铺路。前端展示「上次/下次扫描」让自动同步可见。

## What I already know（探查所得，2026-06-22）

### 扫描链路（决定如何复用）
- 扫描器入口：`Scanner::scan_library(&library) -> anyhow::Result<()>`（`engine/scanner/mod.rs:34`，async）。扫描时为媒体文件（白名单扩展名）入列缩略图任务，带防重。
- 手动触发 `trigger_scan`（`api/library.rs:302-357`）：`tokio::spawn` 后台执行 + API 立即返 202；扫描成功后 `UPDATE libraries SET last_scanned_at`。
- **409 并发防护**：内存锁 `SCANNING: LazyLock<Mutex<HashSet<i32>>>`（`library.rs:19`），按 library id 加锁，扫描完成释放。**进程重启丢失**。
- 这套"扫描单库"逻辑目前**内联在 trigger_scan 的 spawn 闭包里**——定时扫描与手动触发都要用，需抽成共享函数。

### 调度与 spawn 模式
- worker `start_task_worker`（`worker.rs:29`）无限循环 + `sleep(5s)`；`main.rs:89-96` 用 `tokio::spawn` 启动（pool/cache_dir clone 后 move）——scheduler 仿此模式。
- 配置模式：`infra/config.rs` env + 默认值回退（如 `TAGFLOW_DB_PATH`）。

### 数据库与前端
- `libraries` 表有 `last_scanned_at`、`config_json`，**无 scanning 状态字段**（纯内存锁）。
- GET /libraries 已返回 `last_scanned_at`（e2e 已验证推进）；Libraries.vue 已有「立即扫描」按钮。

## Assumptions（已通过探查验证）

- 复用 `Scanner::scan_library` + 409 内存锁；手动触发与 scheduler 共享同一把锁避免冲突。
- 新增后台 scheduler（仿 worker spawn 循环），main.rs 启动。
- 把 trigger_scan 的扫描逻辑抽成 engine 层共享函数，手动触发与定时调度共用，去重。

## 已定决策

- ✅ **Q1（调度模型）→ 全局单定时器**：一个 scheduler 每 N 秒遍历所有库，对每个调 `scan_library`，复用 409 锁跳过进行中的。所有库同频；新增/删除库下一轮重查 DB 自然感知。
- ✅ **Q3（间隔配置）→ 全局 env**：`TAGFLOW_SCAN_INTERVAL`（秒），默认 3600（1h），非法值 clamp 到最小 60s。
- ✅ **Q4（UI scope）→ 后端 + 前端展示「上次/下次扫描」**。
- ✅ **Q2（启动时机）→ 启动后立即首轮 + 之后按 interval**（服务起来数据即最新）。
- ✅ **Q5（e2e）→ 极短间隔 + 新增夹具文件 + 轮询入库**（证明无手动触发也扫进来）。

## Requirements

### 后端
- 新增 `engine::scheduler`：后台定时器，启动后立即首轮扫描所有库，之后每 `TAGFLOW_SCAN_INTERVAL` 秒一轮；每轮查所有 libraries，对每个调共享扫描函数（409 锁跳过进行中的）；单库失败记日志、不阻塞其他库。
- 抽取共享扫描函数（如 `engine::scanner::scan_library_job(pool, id) -> Result<ScanOutcome>`，封装锁 + scan_library + 更新 last_scanned_at）；`trigger_scan` 改调它，保持 202/409 语义不变。
- `infra/config.rs` 加 `TAGFLOW_SCAN_INTERVAL` 读取（默认 3600，clamp ≥60）。
- `main.rs` 用 `tokio::spawn` 启动 scheduler（仿 worker）。
- GET /libraries DTO 增加 `scan_interval_secs`（从 config 读），供前端算 next_scan_at。

### 前端
- Libraries.vue **已具备**手动「触发扫描」按钮（`:367`，调 `POST /libraries/:id/scan`，409 时提示"正在扫描中"）与「最后扫描」展示（`:361`）——**本任务保留不动**。
- 本任务前端仅**补充「预计下次扫描」**：next = last_scanned_at + scan_interval_secs（last 为空显示「等待首次扫描」）。与手动触发共享 409 锁，互不冲突。

### 测试
- e2e：globalSetup 注入极短 `TAGFLOW_SCAN_INTERVAL`（如 2s），测试往已 seed 的库目录新增夹具文件、**不调** `/scan`，轮询 GET /files 直到新文件入库，证明定时自动扫描生效。

## Acceptance Criteria

- [ ] 后台 scheduler 启动后立即首轮，之后按 `TAGFLOW_SCAN_INTERVAL` 周期扫描所有库。
- [ ] 文件增删改在下一轮自动同步进库（e2e 验证，无需手动触发）。
- [ ] 定时扫描与手动 `POST /scan` 共享 409 防护，同库不并发。
- [ ] `trigger_scan` 重构为调用共享扫描函数，202/409 行为不变（既有 library-scan e2e 仍绿）。
- [ ] 单库扫描失败不影响其他库与后续轮次（e2e 或单测覆盖）。
- [ ] `TAGFLOW_SCAN_INTERVAL` 默认 3600，非法值（0/负数）clamp 到 60。
- [ ] Libraries.vue 展示「上次/下次扫描」。
- [ ] 既有 10 个 e2e 用例 + 后端 `cargo test` 不回归。

## Definition of Done

- scheduler 实现 + main.rs 启动 + config 项。
- 共享扫描函数抽取，trigger_scan 与 scheduler 共用（无复制粘贴）。
- 前端展示上次/下次扫描。
- e2e 覆盖定时自动扫描 + 失败容错。
- 不破坏 `cargo test` / `npx playwright test`。
- 配置/运行方式记入 README + spec。

## Technical Approach

### 1. 共享扫描函数（去重的关键）
把 trigger_scan spawn 闭包里的逻辑抽到 engine 层：
```rust
// engine/scanner/mod.rs 或 engine/mod.rs
pub enum ScanOutcome { Performed, SkippedConcurrent, NotFound }
pub async fn scan_library_job(pool: &SqlitePool, id: i32) -> Result<ScanOutcome>
```
内部：try insert SCANNING 锁（失败→SkippedConcurrent）→ `Scanner::scan_library` → 成功则 UPDATE last_scanned_at → release lock。`trigger_scan` 改为 spawn 调它，按 outcome 返 202/409（NotFound 仍 404，保持现行为）；scheduler 直接 await 调它（scheduler 自身已在后台，无需再 spawn）。

### 2. scheduler（仿 worker）
```rust
// engine/scheduler.rs
pub async fn start_scan_scheduler(pool: SqlitePool) {
    let interval = Duration::from_secs(config::scan_interval_secs()); // 默认 3600, clamp 60
    loop {
        // 每轮：查所有 libraries，逐个 await scan_library_job（失败记日志继续）
        if let Ok(libs) = query_all_libraries(&pool).await {
            for lib in libs { let _ = scan_library_job(&pool, lib.id).await; }
        }
        sleep(interval).await;
    }
}
```
首轮立即执行（loop 体在前，sleep 在后）。main.rs：`tokio::spawn(start_scan_scheduler(pool))`，与 worker spawn 并列。

### 3. 前端展示（仅补「下次」）
手动「触发扫描」按钮 + 「最后扫描」展示**已存在**（保留不动）。GET /libraries DTO 加 `scan_interval_secs: i64`（config 值），Libraries.vue 据此算 `next = last_scanned_at + scan_interval_secs`（last 为空显示「等待首次扫描」），与现有手动入口并列展示。

### 4. e2e
globalSetup env 加 `TAGFLOW_SCAN_INTERVAL=2`；新 spec：往 `fixtures/library` 投一个新文件（如 `Photos/new_auto.jpg`，可复用现有图复制），不调 `/scan`，`expect.poll` 轮询 `GET /files` 直到该文件出现（超时 ~15s 覆盖 2s 间隔 + 扫描）。teardown 删掉新增文件保持夹具干净。

## Decision (ADR-lite)

- **Context**：需后台自动同步替代手动触发；现有 trigger_scan 的扫描逻辑内联、409 是内存锁。
- **Decision**：全局单定时器 + 全局 env 间隔；抽取共享 `scan_library_job` 让手动触发与 scheduler 共用同一把 409 锁；启动后立即首轮；前端展示上次/下次。
- **Consequences**：所有库同频（未来可演进为按 last_scanned_at 每库到点）；间隔进程级 env（无 per-lib UI 配置，MVP 不需要）；409 仍进程内（重启丢锁，但 scheduler 重启也会重新扫，无安全问题）。

## Implementation Plan (small PRs)

- **PR1：后端核心**。抽 `scan_library_job` 共享函数；重构 `trigger_scan` 调它（行为不变）；新增 `engine::scheduler` + `infra/config.rs` 的 `TAGFLOW_SCAN_INTERVAL`；`main.rs` spawn scheduler。后端单测（clamp、ScanOutcome）+ 既有 e2e 不回归。
- **PR2：前端展示**。GET /libraries DTO 加 `scan_interval_secs`；Libraries.vue 展示上次/下次扫描。
- **PR3：e2e + 文档**。定时自动扫描用例（极短间隔 + 新增夹具文件 + 轮询入库）+ 失败容错；README/spec 记录 env 与行为。

## Out of Scope (explicit)

- WebDAV 资源库（v0.2.0 第二块，独立任务）。
- 每库独立扫描间隔 / UI 配置间隔（Q1 选全局 env；未来可演进）。
- 扫描优先级 / 限流 / 错峰（v0.3.0+ 视需要）。
- 全文搜索 / 批量标签（v0.3.0+）。

## Research References

- 无外部 research：调度模型由 repo 现状（worker 循环 + trigger_scan 锁）决定，tokio interval 是标准方案。

## Technical Notes

- 关键文件：`engine/scanner/mod.rs`（scan_library）、`api/library.rs`（trigger_scan + SCANNING 锁 line 19）、`engine/worker.rs`（spawn 循环模式）、`main.rs:89-96`（worker spawn）、`infra/config.rs`（env 模式）、`migrations/202512260001_init.sql`（libraries schema）、`tagflow-e2e/globalSetup.ts`（env 注入）、`tagflow-e2e/lib/api.ts`（共享 API 辅助）。
- 409 内存锁进程重启丢失——scheduler 与手动触发同进程共享即安全；scheduler 重启会重新首轮扫描。
- `SCANNING` 锁目前是 `api/library.rs` 的私有项，抽共享函数时需把锁移到 engine 层（或 engine 持有锁，api 调 engine）。
