# Error Handling

> How errors are handled in this project.

---

## Overview

Two distinct layers with different error strategies:

1. **Domain / engine / infra code** (`core/`, `engine/`, `infra/`): returns `anyhow::Result<T>`. Errors are propagated with `?` and enriched with `anyhow!("...: {}", e)` context (see `core/auth.rs:46`, `infra/storage/mod.rs` using `anyhow::bail!`).
2. **API handlers** (`api/`): map errors directly to `axum::http::StatusCode`. There is **no global custom error enum / IntoResponse type** — handlers return one of:
   - `Result<Json<T>, StatusCode>` (e.g. `list_libraries` in `api/library.rs:66-82`)
   - bare `StatusCode` when there is no success body (e.g. `delete_library`, `update_password`)
   - `Json<T>` when failure is expressed in the payload (e.g. `test_library_connection` returns `TestConnectionResponse { reachable, message }`)

`thiserror` is in Cargo.toml but currently unused; new library-style error enums may use it, but match existing handler style first.

---

## Error Types

- `anyhow::Error` everywhere below the API layer; `main()` returns `anyhow::Result<()>`.
- HTTP errors are plain `StatusCode` values; an `ErrorResponse { error: String }` DTO exists in `api/auth.rs:32-35` but most endpoints return empty bodies on failure.
- DB layer helpers may return `Result<_, sqlx::Error>` when the caller wants to distinguish DB failures (see `engine/worker.rs:155-170`).

---

## Error Handling Patterns

Standard handler pattern — log, then map to status (from `api/library.rs`):

```rust
let libraries = sqlx::query_as::<_, Library>("SELECT * FROM libraries ORDER BY id")
    .fetch_all(&pool)
    .await
    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
```

Match pattern for optional rows (from `api/library.rs:265-275`):

```rust
match sqlx::query_as::<_, Library>("SELECT * FROM libraries WHERE id = ?")
    .bind(id).fetch_optional(&pool).await
{
    Ok(Some(lib)) => lib,
    Ok(None) => return StatusCode::NOT_FOUND,
    Err(_) => return StatusCode::INTERNAL_SERVER_ERROR,
};
```

The background worker **never panics or exits**: every fallible step logs via `error!`/`warn!` and `continue`s the loop after a 5s sleep; task failures are persisted to `tasks.error_msg` with status 3 (see `engine/worker.rs:116-130`).

Validation happens at the API boundary before any DB write — e.g. protocol whitelist and `validate_path_security` (path traversal rejection: no `..`, no `./`, absolute path required) in `api/library.rs:21-47,107-117`. Validation failures return `400 BAD_REQUEST` with a `warn!` log.

---

## API Error Responses

Status code conventions in use:

| Status | Meaning | Example |
|--------|---------|---------|
| 200/201/204 | success / created / deleted | `create_library` → 201, `delete_library` → 204 |
| 400 | validation failure | invalid protocol, unsafe path |
| 401 | missing/invalid JWT | `auth_middleware`, login failure |
| 403 | authenticated but rejected | wrong old password in `update_password` |
| 404 | resource not found | unknown library id, missing thumbnail |
| 500 | DB/internal failure | any `sqlx` error |
| 501 | feature not implemented yet | `trigger_scan` |

Error bodies are usually empty; do not invent new error envelope formats without discussion.

---

## Common Mistakes

- Do not `.unwrap()`/`.expect()` on request-path fallible operations; the only accepted `unwrap` is on infallible builders (e.g. `Response::builder()...body(body).unwrap()` in `api/file.rs:88-93`).
- Do not leak internal error strings to clients — log details server-side, return bare status codes.
- Do not let the worker loop propagate errors upward; handle-and-continue is required.
- `list_files` (`api/file.rs:54`) swallows DB errors with `unwrap_or_default()` — known tech debt; do not copy this pattern into new endpoints, prefer `Result<Json<T>, StatusCode>`.
