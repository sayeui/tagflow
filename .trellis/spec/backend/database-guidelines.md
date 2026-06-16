# Database Guidelines

> Database patterns and conventions for this project.

---

## Overview

- **Database**: SQLite, single file `tagflow-core/tagflow.db` (NOT committed to git).
- **Query library**: `sqlx` 0.8 (runtime-tokio, sqlite, chrono, macros features). No ORM.
- **Pool setup**: `infra::db::init_db` (`tagflow-core/src/infra/db.rs`) — `SqlitePoolOptions` with `max_connections(5)`, `acquire_timeout(3s)`, then forces `PRAGMA journal_mode = WAL;` and `PRAGMA foreign_keys = ON;`, then runs `sqlx::migrate!("./migrations")`.
- **State sharing**: the `SqlitePool` is the Axum router state (`.with_state(pool)` in `main.rs`); handlers receive it via `State(pool): State<SqlitePool>`.
- Connection URL uses `?mode=rwc` to auto-create the file: `"sqlite:tagflow.db?mode=rwc"`.

---

## Query Patterns

This codebase uses **runtime-checked queries**, not compile-time macros (`query!` is NOT used — no `DATABASE_URL`/offline metadata is configured):

- Typed row mapping: `sqlx::query_as::<_, Model>("SELECT ...")` where `Model` derives `sqlx::FromRow` (see `api/library.rs:71`, `models/db.rs`).
- Scalars: `sqlx::query_scalar::<_, i64>("SELECT COUNT(*) ...")` (see `engine/worker.rs:186`).
- Writes: `sqlx::query("INSERT/UPDATE/DELETE ...").bind(...).execute(&pool)`. Always `.bind()` parameters with `?` placeholders — never format values into SQL strings.
- Ad-hoc local row types are fine for narrow queries: define `#[derive(sqlx::FromRow)] struct UserRecord { password_hash: String }` inside the handler (see `api/auth.rs:64-67`).
- Existence/affected checks via `res.rows_affected() > 0` (see `delete_library` in `api/library.rs:152-167`).
- New row id via `result.last_insert_rowid()` (see `worker.rs:169`, `core/tag/mod.rs`).
- Recursive tag-tree queries use `WITH RECURSIVE` CTEs — reference implementation in `api/file.rs:24-38`:

```sql
WITH RECURSIVE sub_tags(id) AS (
    SELECT id FROM tags WHERE id = ?
    UNION ALL
    SELECT t.id FROM tags t JOIN sub_tags st ON t.parent_id = st.id
)
SELECT DISTINCT f.* FROM files f
JOIN file_tags ft ON f.id = ft.file_id
WHERE ft.tag_id IN (SELECT id FROM sub_tags)
```

- Idempotent link inserts use `INSERT OR IGNORE` (see `core/tag/mod.rs` `link_file_to_tag`).
- Pagination: `LIMIT ? OFFSET ?` computed from `page`/`limit` query params, defaults `limit=50`, `page=1`.

---

## Migrations

- Location: `tagflow-core/migrations/`, applied automatically on startup by `sqlx::migrate!("./migrations")`.
- Filename convention: `YYYYMMDDNNNN_description.sql` (date + 4-digit sequence), e.g.:
  - `202512260001_init.sql`
  - `202512290002_create_users.sql`
  - `202512300003_create_tasks.sql`
- Migrations are forward-only plain SQL; no down migrations.
- Manual runs (rarely needed): `cargo install sqlx-cli --no-default-features --features sqlite`.

---

## Naming Conventions

- Tables: plural `snake_case` (`users`, `libraries`, `tags`, `files`, `file_tags`, `tasks`).
- Columns: `snake_case`; FKs as `<entity>_id` (`library_id`, `parent_id`, `file_id`, `tag_id`).
- Indexes: `idx_<table>_<purpose>`, e.g. `idx_files_lookup (library_id, parent_path, filename)`, `idx_tags_parent (parent_id)`.
- Datetimes stored as SQLite timestamps, mapped to `chrono::DateTime<Utc>` in Rust (`models/db.rs`); file mtimes are plain `i64` unix seconds.
- Status fields are integer enums documented in code (e.g. `TaskStatus` in `engine/worker.rs:13-19`: 0=Pending, 1=Running, 2=Completed, 3=Failed).
- File paths are stored **relative to the library root** (`parent_path` + `filename`) so libraries can be relocated.

---

## Common Mistakes

- **No soft deletes** — rely on `ON DELETE CASCADE`. Do not add `deleted_at` columns.
- Do not forget `PRAGMA foreign_keys = ON` is set in `init_db`; tests creating their own pools must set it too, or FK constraints silently won't apply.
- Do not use `sqlx::query!`/`query_as!` macros — the project has no offline metadata; builds would fail without `DATABASE_URL`.
- Avoid per-request spawned heavy DB work; enqueue into the `tasks` table and let `engine/worker.rs` process it (poll loop, 5s sleep when idle).

### axum Query 不支持重复 key 成 Vec

axum 的 `Query<T>` 用 `serde_urlencoded`，**不能**把 `?tag_ids=1&tag_ids=2` 反序列化成 `tag_ids: Vec<i32>` —— 重复 key 会报 400「Failed to deserialize」；`tag_ids[]=1&tag_ids[]=2` 虽不报错但 key 不匹配，静默退化为空集。

数组型查询参数一律走**逗号分隔**：

```rust
#[serde(default, deserialize_with = "deserialize_csv_i32")]
pub tag_ids: Vec<i32>,

fn deserialize_csv_i32<'de, D: serde::Deserializer<'de>>(de: D) -> Result<Vec<i32>, D::Error> {
    let raw = Option::<String>::deserialize(de)?;
    match raw {
        None | Some(s) if raw.as_ref().map_or(true, |s| s.trim().is_empty()) => Ok(vec![]),
        Some(s) => s.split(',').map(|x| x.trim().parse().map_err(serde::de::Error::custom)).collect(),
    }
}
```

前端对应 `params.tag_ids.join(',')`（见 `tagflow-ui/src/api/http.ts` `fileApi.list`）。参考实现：`models/dto.rs` `FileQuery`、`api/file.rs` 多标签 AND 查询。

---

## 手动标签写操作（user category）

用户手动标签（`tags.category='user'`）的创建与清理约定（`api/file.rs` `add_file_tag` / `remove_file_tag`）：

- **层级路径**：请求体 `{ "path": "项目/TagFlow" }`，按 `/` 逐层 trim + 过滤空段，每段 `TagManager::ensure_tag(part, "user", parent)` 建/复用节点。复用其 SELECT-then-INSERT 是必需的——SQLite 把 NULL `parent_id` 视为 distinct，`UNIQUE(name, parent_id)` 拦不住多个根级同名标签，必须应用层先查后插。叶子 `INSERT OR IGNORE` 挂到文件（`source='manual'`）。
- **校验在 boundary**：`parse_tag_path` 纯函数过滤空段、限单段 ≤ 64 字符、禁控制字符；空/非法 → 400。
- **source 跨层**：`FileTagInfo` 带 `source`（`auto`/`manual`）端到端流转（DB → DTO → TS），前端据此决定 chip 是否显示「×」移除按钮。**删除仅允许 `manual`**；`auto` 关联 `DELETE` 返回 **403**（受保护，扫描器管理），关联不存在返回 **404**。
- **自动清理（best-effort）**：删除 manual 关联后，`cleanup_orphan_user_tag` 从叶子向上递归——当某 user 节点 `COUNT(file_tags)=0` 且 `COUNT(子节点)=0` 时删除，并对其 `parent_id` 继续判断，清空整条空链。**仅清理 `user` 类别**（path/type/time 由扫描器管理，不在此处理）；非关键路径，查询/删除失败记 error 后退出循环（`unwrap_or(1)` 保守视为「仍有引用」停止清理，非 panic）。

参考实现：`api/file.rs` `add_file_tag` / `remove_file_tag` / `cleanup_orphan_user_tag` / `ensure_user_tag_path` / `parse_tag_path`；单测覆盖建层级/复用/auto 拒绝/自动清理父子链/不碰 auto 类别。
