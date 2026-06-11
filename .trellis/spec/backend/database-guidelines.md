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
