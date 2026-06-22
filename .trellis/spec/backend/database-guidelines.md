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

### SQLite 连接配置契约（busy_timeout + foreign_keys per-connection）

`init_db`（`infra/db.rs`）初始化 pool 时**必须用 `SqliteConnectOptions`**，对 pool **每个连接**统一设 WAL + foreign_keys + busy_timeout。这是并发写不锁、CASCADE 可靠的根基。

**签名**（`SqliteConnectOptions` 链式 → `connect_with`）：
```rust
let options = SqliteConnectOptions::from_str(database_url)?
    .journal_mode(SqliteJournalMode::Wal)
    .foreign_keys(true)
    .busy_timeout(Duration::from_secs(5));
let pool = SqlitePoolOptions::new()
    .max_connections(5)
    .acquire_timeout(Duration::from_secs(3))
    .connect_with(options)
    .await?;
```

**为什么必须用 Options 而非手动 PRAGMA**：
- `PRAGMA foreign_keys` / `PRAGMA busy_timeout` 都是 **per-connection**：手动 `sqlx::query("PRAGMA ...")` 只对执行它的那一个连接生效，pool 其余连接仍是默认值。
- foreign_keys 默认 OFF → 多数连接 `ON DELETE CASCADE` 不强制（删库留孤儿 files/file_tags，历史 bug）。
- busy_timeout 默认 0 → SQLite 写串行下并发写（scheduler 扫描写 + worker 缩略图写 + 手动扫描写）碰写锁立即 `SQLITE_BUSY`（code 5）→ `database is locked`。
- `SqliteConnectOptions` 对 pool 每个新建连接都应用这些 PRAGMA，根治。

**`acquire_timeout` ≠ `busy_timeout`（不可互相替代）**：
- `acquire_timeout`（pool 选项）：从 pool **拿一个空闲连接**的等待超时。
- `busy_timeout`（SQLite PRAGMA）：拿到连接后，SQL 执行遇 **写锁**（另一连接在写）时的重试等待。
- pool 有空闲连接但 SQLite 写锁被占时，只有 `busy_timeout` 能等；反过来亦然。

**错误矩阵**：
| 场景 | 缺失配置 | 表现 |
|---|---|---|
| scheduler/worker/手动扫描并发写 | 无 busy_timeout | `database is locked` (code 5)，worker 更新任务状态失败、任务卡 Running |
| 删 library 期望级联删 files/file_tags | foreign_keys 只设一个连接 | 多数连接 CASCADE 不生效，孤儿数据残留 |

**回归测试**（`infra/db.rs` `#[cfg(test)]`）：
- `test_concurrent_writes_no_deadlock`：8 并发 tokio task 各 INSERT/UPDATE，断言全成功、无 `locked`/`busy`、落库行数正确。
- `test_foreign_keys_cascade_on_all_connections`：循环多次 `pool.acquire()` 命中**非初始连接**，DELETE library 验证 files/file_tags 级联删（真正证明 per-connection 修复，而非只测初始连接）。
- `test_connection_pragmas_applied`：读 PRAGMA 断言 `foreign_keys=1` / `busy_timeout=5000` / `journal_mode=wal` 生效。

#### Wrong（手动 PRAGMA — 两个隐藏 bug）
```rust
let pool = SqlitePoolOptions::new()
    .max_connections(5)
    .connect(database_url).await?;
sqlx::query("PRAGMA journal_mode = WAL;").execute(&pool).await?;   // OK：WAL 是 db 级，持久
sqlx::query("PRAGMA foreign_keys = ON;").execute(&pool).await?;    // ❌ per-connection，只一个连接生效
// ❌ 完全没设 busy_timeout → 并发写 database is locked
```

#### Correct（SqliteConnectOptions — 对所有连接）
```rust
let options = SqliteConnectOptions::from_str(database_url)?
    .journal_mode(SqliteJournalMode::Wal)
    .foreign_keys(true)                        // ✅ 每个连接都设
    .busy_timeout(Duration::from_secs(5));     // ✅ 每个连接都设
let pool = SqlitePoolOptions::new()
    .max_connections(5)
    .acquire_timeout(Duration::from_secs(3))
    .connect_with(options).await?;
```

参考实现：`tagflow-core/src/infra/db.rs::init_db`。

### 孤儿标签清理（删库 + 标签树过滤在线文件）

删库或文件离线（软删 status=0）后，标签树不得显示无效标签（孤儿 + 离线关联），删库后 `tags` 表不堆积孤儿。两机制配合：

**1. 标签树显示层过滤（`api/tag.rs::get_tag_tree`）**
- 只返回**有 status=1（在线）文件关联**的标签：`SELECT DISTINCT tag_id FROM file_tags ft JOIN files f ON ft.file_id = f.id WHERE f.status = 1` 得「在线 tag_id 集合」，`build_tree` 后递归剪枝。
- **按子树剪枝**：节点自身不在集合且所有子节点被剪则剪；父标签在子标签有在线文件时仍显示（树结构完整性）。
- 纯显示层、不改 tags 表。**同时解决**：删库孤儿（无 file_tags）+ 扫描删文件离线（file_tags 关联 status=0）的「标签显示但查询空」。
- **软删天然支持回归**：`mark_as_lost`（status=0）文件恢复（status→1）时，过滤重算、标签自动重现，无需额外处理。

**2. 删库孤儿清理（`api/library.rs::delete_library` + `cleanup_orphan_tag`）**
- `tags` 表无 `library_id` FK、不在删库 CASCADE 链 → 删库后 tags 节点残留为真孤儿，必须显式清理。
- `cleanup_orphan_tag`（`api/file.rs`，原 `cleanup_orphan_user_tag` 去除 user 限制）适用**所有类别**（path/ext/type/time/user），递归逻辑：`COUNT(file_tags)=0 且 COUNT(子节点)=0` 则删、向上递归。
- `delete_library` 流程：**删库前**查受影响 tag_ids（`SELECT DISTINCT tag_id FROM file_tags WHERE file_id IN (SELECT id FROM files WHERE library_id = ?)`）→ `DELETE library`（CASCADE 删 files/file_tags）→ **删库后**对每个 tag_id 调 `cleanup_orphan_tag`。清理失败 `error!` 记日志、不阻塞删库 204。

**跨库共享标签天然安全**：`#year:2026`、`Projects/` 被多库共用时，COUNT/EXISTS 判定保证「他库有 status=1 关联则保留/显示」，只删/隐藏真孤儿。

参考实现：`api/tag.rs::get_tag_tree`（过滤）、`api/library.rs::delete_library`（删库清理）、`api/file.rs::cleanup_orphan_tag`（泛化递归清理）。回归测试：`tag.rs` 标签树过滤 5 测、`library.rs` 删库清理 4 测、e2e `tag-tree-cleanup.spec.ts`（删库孤儿 + 软删隐藏/恢复）。

> **Warning**：`cleanup_orphan_tag` 泛化后，`remove_file_tag`（删 manual 关联）也会清 path/ext/type/time 空节点——这是期望行为（auto 标签删关联后变空也应清），非 bug；但**只清 COUNT=0 的真孤儿**，仍有其他文件关联的 auto 标签不动。
