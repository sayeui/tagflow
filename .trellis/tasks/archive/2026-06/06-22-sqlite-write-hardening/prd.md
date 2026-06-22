# 加强 SQLite 写并发（busy_timeout 15s + worker 重试）

## Goal

消除 NAS 真实部署（1700+ 文件库）扫描时偶发的 `database is locked`（worker:114 更新任务状态失败、任务卡 Running）。`fix-sqlite-locked` 的 `busy_timeout=5s` 在小库（e2e 5 图）够，但 1700 文件扫描密集写时偶发超时。发版阻塞。

## 根因（已诊断，2026-06-22）

- `fix-sqlite-locked` 设 `busy_timeout=5s`，e2e（5 图轻并发）验证通过、已部署 NAS（其他 fix 生效佐证修复版在跑）。
- NAS 1700+ 文件库扫描：scanner 连续 INSERT（每文件 `files` + `file_tags` + `tasks` ~3 写 × 1700 ≈ 5000+ 写），密集段写锁持续占用，worker 并发 `UPDATE task status` 偶发 5s 内抢不到锁 → `SQLITE_BUSY` 超时。
- scanner 自动提交（间歇锁），但密集 INSERT 间隙极短，5s busy_timeout retry 运气差时超时。
- **偶发**（非持续）说明 5s 在边界——加大窗口即解决。

## 修复

1. **`busy_timeout` 5s → 15s**（`infra/db.rs`）。1700 文件扫描窗口约 5-15s，15s 覆盖全程；worker 后台异步，等 15s 可接受。
2. **worker:114 遇 locked 重试兜底**：`UPDATE task status`（Ok/Err 两处）遇 `SQLITE_BUSY` 重试 3 次（退避，如 500ms/1s/2s），仍失败才 `error!`。即使偶发超时也能恢复，不让任务卡 Running。

## Requirements

- `infra/db.rs`：`busy_timeout(Duration::from_secs(5))` → `from_secs(15)`，启动 info 日志同步改。
- `worker.rs:104-133`：两处 UPDATE task status（Ok 分支 status=2、Err 分支 status=3）遇 `SQLITE_BUSY`（code 5 / "locked"）重试 3 次、退避；重试中 `debug!`、最终失败 `error!`。
- 单测/集成测：并发写压测（密集 INSERT + worker 并发 UPDATE，15s 内不 locked）+ worker 重试逻辑（mock/detect SQLITE_BUSY 重试成功）。

## Acceptance Criteria

- [ ] `busy_timeout=15s`（PRAGMA 验证 + 启动日志）。
- [ ] worker UPDATE 遇 locked 重试（单测覆盖重试逻辑与最终失败路径）。
- [ ] `cargo test` 全过（含新测，既有 78 不回归）+ clippy 干净。
- [ ] 真实复验（用户）：NAS 1700+ 文件库多次扫描，worker 日志无 `database is locked`（或重试后成功，任务不卡 Running）。

## Definition of Done

- `infra/db.rs` + `worker.rs` 改造 + 测试通过。
- `cargo test` / `clippy` 干净。
- spec：`backend/database-guidelines.md` 的「SQLite 连接配置契约」busy_timeout 值更新 5s→15s + 补 worker 重试约定。

## Technical Approach

- `busy_timeout` 改值即可（`SqliteConnectOptions::busy_timeout(Duration::from_secs(15))`）。
- worker 重试：抽一个 helper（`update_task_status_with_retry`），检测 sqlx Error 的 `DatabaseError` code == 5（SQLITE_BUSY）或 message 含 "locked"，是则 `tokio::time::sleep` 退避重试，3 次后放弃。两处 UPDATE（status=2/3）共用。
- 15s 理由：1700 文件扫描 ~5000 写，SQLite 毫秒/写 + IO，全程约 5-15s；15s busy_timeout 让 worker 等过整个扫描窗口。

## Decision (ADR-lite)

- **Context**：`fix-sqlite-locked` 的 5s busy_timeout 在 1700+ 文件真实库扫描偶发不够。
- **Decision**：`busy_timeout` 15s（覆盖扫描窗口）+ worker 重试兜底（3 次退避）。
- **Consequences**：偶发 locked 消除；worker 重试保证任务不卡 Running；15s 对后台异步可接受；未来超大库（万+）若仍偶发，可调 30s 或扫描分批让锁（YAGNI，不做）。

## Implementation Plan (small PRs)

- **PR1**：`infra/db.rs` busy_timeout 15s + `worker.rs` 重试 helper + 单测（重试逻辑 + 并发写压测）+ spec 值更新。`cargo test`/`clippy` 全过。
- **PR2**：用户 NAS 重新部署复验（多次扫描 1700+ 文件库，无 locked）。

## Out of Scope

- 架构级串行化（max_connections=1 / 写队列）——15s + 重试对当前负载够。
- 扫描分批 yield 让锁——复杂度增加，YAGNI。
- busy_timeout 可配（env）——当前固定 15s 够，需要时再加。

## Technical Notes

- 关键文件：`tagflow-core/src/infra/db.rs`（busy_timeout，`fix-sqlite-locked` 引入）、`tagflow-core/src/engine/worker.rs:104-133`（UPDATE task status Ok/Err 两处）。
- worker 重试检测 SQLITE_BUSY：sqlx Error `database_error().code()` == Some("5") 或 message 含 "locked"。
- 既有测试：`test_concurrent_writes_no_deadlock`（8 并发 task）保留，可加大并发数模拟更密集写。
