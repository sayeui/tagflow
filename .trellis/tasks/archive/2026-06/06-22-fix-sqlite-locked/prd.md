# 修复 SQLite database is locked（连接配置）

## Goal

修复扫描图片库时 worker 日志刷 `database is locked (code: 5)` ERROR。根因是 SQLite 连接配置缺陷（busy_timeout 未设 + foreign_keys per-connection 只设一个），本 session 的 scheduler 增加并发写把 busy_timeout 缺失暴露出来。这是 v0.2.0 发版阻塞 bug（P0）。

## 根因（已诊断，2026-06-22）

1. **busy_timeout 未设**：`infra/db.rs` 只 `PRAGMA journal_mode=WAL` + `foreign_keys=ON`，没设 SQLite 的 `busy_timeout`。SQLite 写串行，并发写（scheduler 扫描写 + worker 缩略图写 + 手动扫描写）碰写锁默认立即 `SQLITE_BUSY` → `database is locked`。pool 的 `acquire_timeout(3s)` 是「拿连接」超时，与写锁等待是两回事。
2. **foreign_keys 是 per-connection，只设了一个**：`PRAGMA foreign_keys=ON` 只对执行它的那一个连接生效，pool 其余 4 个连接 `foreign_keys=OFF` → **`ON DELETE CASCADE` 对多数连接不强制**（隐藏 bug，删库可能留 files/file_tags 等儿数据）。

## 修复

`infra/db.rs` 改用 `SqliteConnectOptions`——对 pool **每个连接**统一设 WAL + foreign_keys + busy_timeout（替代手动 PRAGMA，手动 PRAGMA 只对一个连接生效）：

```rust
let options = SqliteConnectOptions::from_str(database_url)?
    .journal_mode(SqliteJournalMode::Wal)
    .foreign_keys(true)
    .busy_timeout(Duration::from_secs(5));   // 写锁等待 5s 重试
let pool = SqlitePoolOptions::new()
    .max_connections(5)
    .acquire_timeout(Duration::from_secs(3))
    .connect_with(options)
    .await?;
```

一个改动同时修两个 bug，且对所有 pool 连接生效。

## Requirements

- `infra/db.rs` 用 `SqliteConnectOptions` 设 `busy_timeout(5s)` + `journal_mode(Wal)` + `foreign_keys(true)`，对 pool 每个连接生效。
- 移除手动 `PRAGMA journal_mode` / `PRAGMA foreign_keys`（由 Options 替代）。
- 保留 `max_connections(5)` + `acquire_timeout(3s)` + `sqlx::migrate!`。
- 加测试覆盖并发写不锁 + foreign_keys CASCADE 在任意连接生效（防回归）。

## Acceptance Criteria

- [ ] `infra/db.rs` 用 `SqliteConnectOptions`，busy_timeout/foreign_keys/WAL 对所有 pool 连接生效。
- [ ] 新测试：多个并发写任务（UPDATE tasks）在 busy_timeout 内不报 `database is locked`。
- [ ] 新测试：通过 pool 不同连接 `DELETE library`，验证 `files`/`file_tags` 级联删除（foreign_keys 对所有连接生效）。
- [ ] `cargo test` 全过（含现有 64+ 测试不回归）。
- [ ] 真实复验（用户）：NAS 重新部署扫描图片库，worker 日志无 `database is locked`。

## Testing

- 并发写不锁：spawn 多个并发 `UPDATE tasks` / `INSERT`，断言全部成功（无 SQLITE_BUSY）。
- foreign_keys CASCADE：用 pool 取两个不同连接，分别 DELETE 父行，验证子表级联删除（证明 foreign_keys 对所有连接生效，而非只一个）。

## Definition of Done

- `infra/db.rs` 改造 + 测试通过。
- `cargo test` / `cargo clippy` 干净。
- spec：`backend/database-guidelines.md` 加「SQLite 连接配置契约」（busy_timeout + foreign_keys 必须用 SqliteConnectOptions 对所有连接设，禁手动 PRAGMA）。
- 用户 NAS 复验扫描无报错。

## Technical Approach

- 唯一改动点：`tagflow-core/src/infra/db.rs` 的 `init_db`，PRAGMA 手动执行 → `SqliteConnectOptions` 链式配置 + `connect_with`。
- `busy_timeout(5s)`：SQLite 写锁等待，缓解 scheduler/worker/手动扫描并发写冲突。
- `foreign_keys(true)`：每个连接强制外键，修 CASCADE。
- `journal_mode(Wal)`：WAL 是 db 级（设一次持久），Options 也确保新连接正确。
- 测试放 `infra/db.rs` 的 `#[cfg(test)]` 或独立集成测试。

## Decision (ADR-lite)

- **Context**：SQLite 并发写 `database is locked` + foreign_keys per-connection CASCADE 不可靠，scheduler 暴露。
- **Decision**：用 `SqliteConnectOptions` 统一设连接级 PRAGMA（busy_timeout + foreign_keys + WAL），对所有 pool 连接生效；busy_timeout=5s 缓解写冲突。
- **Consequences**：解决锁报错 + CASCADE 可靠；5s busy_timeout 对单用户自部署负载足够，密集写仍可能偶发（架构级串行化 YAGNI，不做）；连接配置契约入 spec 防再犯。

## Implementation Plan (small PRs)

- **PR1**：`infra/db.rs` 改 `SqliteConnectOptions` + 移除手动 PRAGMA + 并发写测试 + foreign_keys CASCADE 测试。`cargo test`/`clippy` 全过。
- **PR2**：spec 入 `backend/database-guidelines.md`（连接配置契约）；用户 NAS 重新同步部署复验。

## Out of Scope

- 架构级并发串行化（max_connections=1 或写队列）——busy_timeout 对当前负载足够，YAGNI。
- WAL checkpoint / synchronous 调优。
- worker 任务状态卡 Running 的存量清理（修 busy_timeout 后新任务正常；存量 Running 任务不影响，可在后续清理）。

## Technical Notes

- 关键文件：`tagflow-core/src/infra/db.rs`（连接配置）、`engine/worker.rs:114`（报错点）、`engine/scheduler.rs`（并发写源）、`engine/scanner/`（写 files/file_tags）。
- `SqliteConnectOptions::busy_timeout` 对每个 pool 连接设 `PRAGMA busy_timeout`，写锁等待重试。
- `foreign_keys(true)` 对每个连接设 `PRAGMA foreign_keys=ON`，修 CASCADE。
- sqlx 0.8 的 `SqliteConnectOptions` 默认 busy_timeout 可能已有值，但显式设 5s 确保可靠。
