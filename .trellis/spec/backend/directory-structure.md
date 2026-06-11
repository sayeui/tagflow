# Directory Structure

> How backend code is organized in this project.

---

## Overview

The backend is a single Rust crate `tagflow-core` following Hexagonal Architecture:

```
Presentation (Vue 3) → API layer (Axum) → Core domain → Infrastructure
                                              ↓
                                      (storage / database)
```

- **`api/`** — thin HTTP handlers (Axum). One file per resource. Handlers do request parsing, validation, and DB queries; they do NOT contain reusable domain algorithms.
- **`core/`** — pure domain logic (auth crypto, tag tree management). No HTTP types here.
- **`engine/`** — long-running engines: incremental scanner, path tagger, background task worker.
- **`infra/`** — adapters to the outside world: SQLite pool, OpenDAL storage, thumbnail generation.
- **`models/`** — `db.rs` (sqlx `FromRow` structs mirroring tables) and `dto.rs` (serde request/response types).
- **`bin/`** — standalone CLI tools that reuse the library via `lib.rs`.

---

## Directory Layout

```
tagflow-core/src/
├── main.rs              # Entry point: tracing init, DB init, admin bootstrap, router, middleware
├── lib.rs               # Library entry so bin/ tools can reuse modules
├── api/                 # REST handlers (one file per resource)
│   ├── mod.rs           # `pub mod tag; pub mod file; pub mod auth; pub mod library;`
│   ├── auth.rs          # login, update_password, auth_middleware
│   ├── file.rs          # list_files, get_thumbnail
│   ├── library.rs       # library CRUD + connection test + scan trigger
│   └── tag.rs           # tag tree endpoint
├── core/
│   ├── auth.rs          # Argon2 hashing, JWT create/decode (pure functions)
│   └── tag/mod.rs       # TagManager (hierarchical tag upsert, file-tag linking)
├── engine/
│   ├── scanner/mod.rs   # Scanner (incremental diff scan via OpenDAL)
│   ├── tagger/mod.rs    # PathTagger
│   └── worker.rs        # Background task worker loop (thumbnails)
├── infra/
│   ├── db.rs            # init_db: pool + PRAGMA + migrations
│   ├── storage/mod.rs   # StorageManager: protocol → OpenDAL Operator
│   └── thumbnail.rs     # ThumbnailGenerator
├── models/
│   ├── db.rs            # Library, Tag, FileEntry (derive FromRow)
│   └── dto.rs           # Request/Response DTOs + From<DbModel> impls
└── bin/
    └── reset-password.rs
```

---

## Module Organization

Adding a new API resource follows this recipe (see `library.rs` as the reference):

1. Add DTOs to `models/dto.rs` with a `// ========== X 相关 DTO ==========` section header, plus `impl From<DbModel> for XResponse` for mapping.
2. Create `src/api/<resource>.rs` with `pub async fn` handlers.
3. Register the module in `src/api/mod.rs`.
4. Wire routes in `main.rs`: public routes go in `auth_routes`, everything else in `protected_routes` (which is layered with `api::auth::auth_middleware` then `request_logging_middleware`) — see `tagflow-core/src/main.rs:56-74`.

Domain logic that is reused across handlers/engines lives in `core/` as a struct holding the pool (e.g. `TagManager::new(db)` in `core/tag/mod.rs`).

Background work is NOT spawned per-request: jobs are inserted into the `tasks` table and consumed by the worker loop started in `main.rs:50-52` (`engine::worker::start_task_worker`).

---

## Naming Conventions

- Files/modules: `snake_case`, one resource per file (`library.rs`, not `libraries_controller.rs`).
- Handlers: verb-first `snake_case` (`list_libraries`, `create_library`, `trigger_scan`).
- DTOs: `XxxRequest` / `XxxResponse` (e.g. `CreateLibraryRequest`, `TestConnectionResponse`).
- DB models: singular nouns matching the table (`Library`, `Tag`, `FileEntry`).
- Routes: `/api/v1/<plural-resource>` for business APIs; `/api/auth/*` for authentication (no version prefix).
- Doc comments and inline comments are written in Chinese; identifiers in English.

---

## Examples

- Well-organized API module: `tagflow-core/src/api/library.rs` (handler docs with route/request/response examples, validation, logging).
- Core domain struct pattern: `tagflow-core/src/core/tag/mod.rs` (`TagManager`).
- Infra adapter pattern: `tagflow-core/src/infra/storage/mod.rs` (`StorageManager::get_operator` matching on protocol).
