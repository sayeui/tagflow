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
- [ ] `cargo fmt && cargo clippy && cargo test` clean?
- [ ] Change scope limited to the requirement — no drive-by refactors of unrelated features?
