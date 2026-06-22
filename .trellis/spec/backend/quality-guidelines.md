# Quality Guidelines

> Code quality standards for backend development.

---

## Overview

- Rust edition **2024**, toolchain 1.92.0+ (`tagflow-core/Cargo.toml`).
- Quality gates: `cargo fmt`, `cargo clippy`, `cargo test` must pass before commit. No custom rustfmt/clippy config — defaults apply.
- Style target: lean and non-redundant. Comments/docs only when they add value; doc comments and comments are written in Chinese.
- Performance budgets (from system spec): idle RSS <30MB, scan RSS <150MB, file queries over 100k+ rows <150ms — prefer SQL (CTEs, indexes) over in-memory processing.

---

## Forbidden Patterns

- `println!`/`eprintln!` in application code — use `tracing` macros (CLI `bin/` tools excepted).
- `.unwrap()`/`.expect()` on request-path fallible operations (DB, IO, parsing user input).
- String-formatting values into SQL — always `.bind()` with `?` placeholders.
- Blocking IO inside async handlers — all IO is Tokio async (`tokio::fs::File`, OpenDAL, sqlx). The minor `std::fs::read_dir` permission probe in `test_library_connection` is existing debt; don't extend it.
- `sqlx::query!` compile-time macros — no offline metadata configured.
- Soft-delete columns — schema relies on `ON DELETE CASCADE`.
- Trusting client-supplied filesystem paths — must pass `validate_path_security` (`api/library.rs:21-47`).

---

## Required Patterns

- Handlers: `async fn` taking `State(pool): State<SqlitePool>` + extractors; return `Result<Json<T>, StatusCode>` or `StatusCode`.
- New protected routes must be registered under `protected_routes` in `main.rs` so `auth_middleware` and `request_logging_middleware` apply.
- DTO ↔ DB model conversion via `impl From<DbModel> for ResponseDto` in `models/dto.rs`.
- Long-running/heavy work goes through the `tasks` table + worker loop (`engine/worker.rs`), never inline in a request.
- Errors: `anyhow::Result` below the API layer, `?` propagation with context via `anyhow!("...: {}", e)`.
- Datetimes: `chrono::DateTime<Utc>` for DB timestamps; unix-seconds `i64` only for file mtimes.

---

## 后台任务与定时扫描契约

两种后台任务模式（都在 `main.rs` 用 `tokio::spawn` 启动，不阻塞请求）：

1. **tasks 表 worker**（`engine/worker.rs`）：轮询 `tasks` 表执行重任务（缩略图生成），空轮询 5s sleep。重/耗时工作必须走这条（见上方 Required Patterns）。
2. **定时 scheduler**（`engine/scheduler.rs`）：定时触发扫描，**不经过** `tasks` 表。

### 定时扫描 scheduler（`engine/scheduler.rs::start_scan_scheduler`）

- 无限循环，**首轮立即执行**（loop 体在前、`sleep(interval)` 在后），每轮 `SELECT * FROM libraries ORDER BY id` 逐库扫描。
- 间隔由 `infra::config::scan_interval_secs()` 读 `TAGFLOW_SCAN_INTERVAL`（秒，默认 3600，clamp ≥60）。
- **健壮性**：单库扫描失败 `error!` 后 continue 下一库；查库失败 `error!` 后仍 sleep 进下一轮——**永不退出循环**，无 `unwrap`/`expect`/panic 路径。

### 扫描去重契约（关键）

手动触发（`api/library.rs::trigger_scan`）与定时 scheduler **必须共用** engine 层的扫描实现，绝不能复制两份：

- `engine::scanner::scan_library_job(pool, id) -> Result<ScanOutcome>`（scheduler 用：try lock → scan → release）
- `engine::scanner::run_scan_with_lock_held(pool, id)`（trigger_scan 用：锁已持有、不释放，主体逻辑共享）
- `ScanOutcome { Performed, SkippedConcurrent, NotFound }`

**409 并发锁** `SCANNING: LazyLock<Mutex<HashSet<i32>>>` 位于 **engine 层**（`engine/scanner/mod.rs`），api 与 scheduler 共用同一把——同库不并发。**切勿在 api 层重新声明扫描锁**（早期版本锁在 `api/library.rs`，已迁移；再改时不要再搬回）。

**trigger_scan 同步 409 语义**：`try_acquire_scan_lock(id)` 在 HTTP 请求路径**同步**调用（不跨 await），调用方立即拿到 404/409/202。重构 trigger_scan 时必须保持这个同步语义——不要改成 spawn 后异步判断 409（调用方需要立即知道是否被拒）。

> **Warning：TAGFLOW_E2E_FAST_SCAN escape hatch**
>
> `scan_interval_secs()` 对 production clamp ≥60s（防高频扫描压满 IO）。`TAGFLOW_E2E_FAST_SCAN=1`（严格匹配字面量 `"1"`，大小写敏感）是**仅供 `tagflow-e2e` 自带 Playwright 套件**绕过 clamp 的逃生阀，让亚分钟级间隔（如 2s）生效以验证 scheduler。
>
> **production / 开发环境绝不应设置**；unset / `true` / `yes` / `0` 等其他值一律视为关闭。改 config clamp 逻辑时不得放宽这个边界。

---

## Testing Requirements

- Unit tests live inline as `#[cfg(test)] mod tests` at the bottom of the file under test. Real examples:
  - `core/auth.rs:142-161` — hash/verify roundtrip, JWT create/decode.
  - `api/auth.rs:246-266` — DTO serde roundtrips.
  - `infra/thumbnail.rs` — thumbnail generator tests.
- Test names: `test_<behavior>` (`test_password_hash_and_verify`, `test_jwt_create_and_decode`).
- Pure domain logic (core/, infra helpers) must have unit tests; HTTP handlers are currently tested via DTO-level tests only (no integration-test harness yet — don't invent one ad hoc, discuss first).
- Run: `cargo test` (use `-- --nocapture` to see output).

---

## Code Review Checklist

- [ ] New routes registered in the correct router group (public vs protected) in `main.rs`?
- [ ] All user input validated at the API boundary (protocol whitelist, path security)?
- [ ] All SQL parameters bound, indexes considered for new query shapes?
- [ ] Logging at correct levels, no secrets logged, Chinese messages?
- [ ] No `unwrap` on fallible request-path code; errors mapped to correct status codes (see error-handling.md table)?
- [ ] Heavy work enqueued as a task, not inline?
- [ ] Scan/trigger changes share `scan_library_job` between `trigger_scan` and scheduler (no duplicated scan logic); 409 lock stays in engine layer; `TAGFLOW_E2E_FAST_SCAN` clamp not loosened?
- [ ] `cargo fmt && cargo clippy && cargo test` clean?
- [ ] Change scope limited to the requirement — no drive-by refactors of unrelated features?
