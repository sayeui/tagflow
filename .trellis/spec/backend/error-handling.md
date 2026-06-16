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
- OpenDAL 0.50 的 `op.read()` / `op.read_with(path).range(..)` 返回 `opendal::Buffer`（非 `Vec<u8>`）——传给 `Body::from` 或 `String::from_utf8` 前必须 `.to_vec()`，否则 `From<Buffer>` 未实现导致编译失败。参考 `api/file.rs` `get_content`：`let bytes = op.read(&path).await?; Body::from(bytes.to_vec())`。

---

## 受保护资源 URL 与媒体鉴权（`?token=` 兜底）

浏览器原生媒体标签（`<img>` / `<video>` / `<audio>` / `<iframe src>`）发起的请求**不会**携带 `Authorization` 头。受 `auth_middleware` 保护的资源（缩略图、文件内容）若仅靠 header 鉴权会被一律 401，且 `<img @error>` 静默隐藏，表现为"资源加载不出来"——历史缩略图一直显示图标而非真缩略图的根因正是此。

**解法**：`auth_middleware`（`api/auth.rs`）在 `Authorization: Bearer` 之外，额外接受查询参数 `?token=<jwt>` 兜底（header 优先，query 次之）。前端用 `mediaUrl()` helper（`api/http.ts`）统一拼接 `?token=<jwt>` 供媒体 `src` 与下载使用。

**契约**：
- header 与 query 二选一即可，header 优先；
- query `token=` 空值不算（`query_token` 返回 `None` → 401）；
- 任何凭证缺失/无效一律 401，与无 header 凭证一致。

**边界**：JWT 进 URL 会出现在 server access log / 浏览器历史。自托管单用户 LAN 可接受；多用户或公网部署应改短时效签名 media token。

**测试**：`api::auth::query_token_extracts_first_non_empty` 覆盖 query 解析。前端配合见 frontend `quality-guidelines.md` 对应 common mistake。
