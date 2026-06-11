# Logging Guidelines

> How logging is done in this project.

---

## Overview

- Library: `tracing` + `tracing-subscriber` with `EnvFilter`. **`println!` is forbidden** in application code (CLI tools in `bin/` may print to stdout).
- Initialization lives in `main.rs:20-34`: default filter `"tagflow_core=info,axum=info"`, overridable via `RUST_LOG` (e.g. `RUST_LOG=debug cargo run`, or per-module `RUST_LOG=tagflow_core::api::library=debug`).
- Formatter shows target module + line number, hides thread ids/file paths.
- Log message text is written in **Chinese**, matching the rest of the codebase.

---

## Log Levels

Per project convention (also documented in CLAUDE.md):

- `debug!()` — detailed flow: function params, intermediate state, per-request traces (`debug!("获取资源库列表")`, `debug!("路径安全检查通过: {}", path)`).
- `info!()` — significant business operations: startup, library created, admin bootstrap, worker started (`info!("资源库创建成功: {}", payload.name)`).
- `warn!()` — expected failures: invalid input, 4xx responses, unreachable paths, unknown task types (`warn!("无效的协议类型: {}", payload.protocol)`).
- `error!()` — failures needing attention: DB errors, 5xx responses, task state update failures (`error!("更新任务状态失败: {}", e)`).

---

## Structured Logging

HTTP request logging is centralized in `request_logging_middleware` (`main.rs:135-175`), applied as a layer to every route group. It uses emoji markers and logs method, path, status, duration:

- `➡️` request start (debug; notes `authenticated` vs `public` based on Authorization header)
- `✅` 2xx (debug)
- `⚠️` 4xx (warn)
- `❌` 5xx (error)

Do not add per-handler request/response logging that duplicates the middleware; handlers log business events only.

Format values inline with `{}` placeholders, key=value style for ids: `debug!("获取到任务: id={}, file_id={}, type={}", id, file_id, task_type)` (`engine/worker.rs:73`).

---

## What to Log

- Startup milestones (DB ready, worker started, server address) at `info`.
- Every create/delete/scan business action with its key identifiers at `info`.
- Every validation rejection with the offending value at `warn`.
- Every DB failure at `error` (the handler then maps to 500).
- Worker lifecycle: task pickup (`debug`), task failure with message (`warn`), state-update failure (`error`).

---

## What NOT to Log

- Passwords, password hashes, JWT tokens, or Authorization header values — the middleware only logs *whether* an auth header is present, never its content.
- Exception: the one-time default-admin bootstrap prints the generated credentials to console (`main.rs:121-126`) so the operator can log in; do not extend this pattern elsewhere.
- Avoid logging full file contents or large payloads; log ids/paths/counts instead (`info!("返回 {} 个资源库", response.len())`).
