# M9-1: 配置外部化与 rust-embed 单二进制

## Goal

为 TagFlow 的"单二进制部署"产品卖点奠基：将前端 `tagflow-ui/dist` 通过 `rust-embed` 嵌入 Rust 二进制，并将当前硬编码的 `tagflow.db` 与 `./cache` 路径收敛为环境变量。完成本任务后，`cargo build --release` 产出的单一可执行文件即可独立提供服务，不再依赖外部静态服务器与默认工作目录。

## What I already know

### 现状（已盘点）

- **硬编码 1**：`main.rs:72` `let db_url = "sqlite:tagflow.db?mode=rwc";`
- **硬编码 2**：`main.rs:83` `start_task_worker(pool_for_worker, "./cache".to_string())`
- **硬编码 3**：`infra/thumbnail.rs:204` 测试用 `"./cache"`（测试范围，可保留或一并迁移）
- **已支持的环境变量**：`TAGFLOW_PORT`、`TAGFLOW_JWT_SECRET`、`TAGFLOW_ADMIN_PASSWORD`、`TAGFLOW_ADMIN_USERNAME`
- **Cargo.toml**：未引入 `rust-embed`、`mime_guess`、`tower-http`（按需）
- **前端产物**：`tagflow-ui/dist` 已存在构建输出，未嵌入二进制
- **路由结构**：`auth_routes` + `protected_routes` 两个子 Router 合并，未设置 `fallback`
- **ThumbnailGenerator**：构造时 `create_dir_all(cache_dir)`，失败仅 `error!` 不中断
- **数据库初始化**：`infra/db.rs::init_db(database_url: &str)` 已是参数化函数，只需在外部传值

### 已锁定（前置问答）

- 拆分粒度：方案 A（任务一配置化+嵌入；任务二 Docker+文档）
- 运行镜像基座：Alpine + ffmpeg（任务二）
- 仅支持 SQLite，不引入 Postgres/MySQL

## Assumptions (temporary)

- 环境变量命名延续 `TAGFLOW_*` 前缀：`TAGFLOW_DB_PATH`、`TAGFLOW_CACHE_DIR`
- `TAGFLOW_DB_PATH` 给出 SQLite 文件路径（如 `/var/lib/tagflow/tagflow.db`），代码内部拼接 `sqlite:{path}?mode=rwc`，自动创建父目录
- 缺省值沿用现状：`tagflow.db`（相对当前工作目录）、`./cache`
- 开发模式（`cargo run`）下不强制要求 `dist` 存在；release 模式要求 `dist` 已构建

## Open Questions

（已全部解决，见 Decision 节）

## Requirements (evolving)

1. 引入 `rust-embed`，将 `tagflow-ui/dist` 嵌入二进制
2. 实现 SPA fallback handler：非 `/api/*` 路径优先匹配静态资源，未命中返回 `index.html`
3. 新增 `TAGFLOW_DB_PATH` 环境变量（默认 `tagflow.db`），自动创建父目录
4. 新增 `TAGFLOW_CACHE_DIR` 环境变量（默认 `./cache`）
5. `main.rs` 启动信息打印当前 db/cache 实际路径，便于运维定位
6. `cargo build --release` 产出的二进制可独立运行并提供前端服务

## Acceptance Criteria (evolving)

- [ ] 启动时读取 `TAGFLOW_DB_PATH`/`TAGFLOW_CACHE_DIR`，缺省回退到现状值并 `info!` 打印
- [ ] `curl http://localhost:8080/` 返回前端 `index.html`
- [ ] `curl http://localhost:8080/api/auth/login`（POST）仍正常工作（API 路由优先）
- [ ] 任意前端路由（如 `/login`、`/settings/security`）刷新页面不返回 404（SPA fallback 生效）
- [ ] 静态资源 MIME 类型正确（`.js` → `application/javascript`、`.css` → `text/css`）
- [ ] e2e 验证：`cargo build --release && TAGFLOW_DB_PATH=/tmp/x.db TAGFLOW_CACHE_DIR=/tmp/cache ./target/release/tagflow-core` 启动后浏览器访问可用
- [ ] 缓存目录权限不足时启动明确报错（如选中 fail-fast）
- [ ] 既有单测全绿，新增配置加载单测覆盖默认值与覆盖值

## Decision (ADR-lite)

**Context**: rust-embed 是编译期宏，要求 folder 目录存在。同时项目当前已有 vite + proxy 开发流程。

**Decision**:
1. **开发模式不特殊处理**：保持现状（vite dev server + `/api` proxy）。rust-embed 嵌入的是上次 `npm run build` 的产物；开发人员清楚嵌入产物与 dev server 分离。release 部署前必须先 `npm run build`。
2. **环境变量命名**：`TAGFLOW_DB_PATH`（默认 `tagflow.db`）、`TAGFLOW_CACHE_DIR`（默认 `./cache`）。
3. **DB 父目录不自动创建**：保持现状行为，避免掩盖部署错误；运维负责挂载卷。
4. **缓存目录权限不足**：保持现状（warn + 后续缩略图任务失败），不做 fail-fast，留给任务二在 Docker 场景再评估。
5. **rust-embed folder**：`#[folder = "../tagflow-ui/dist/"]`（相对 `tagflow-core/Cargo.toml`）。
6. **thumbnail.rs 测试硬编码 `"./cache"` 保留**：测试范围，不影响生产路径。

**Consequences**:
- 开发人员首次开发前需 `cd tagflow-ui && npm run build` 一次（确保 dist 存在）
- 后续若 dist 漂移导致 rust-embed 嵌入过时产物，靠 CI 在 release 前 build 兜底（任务二 Dockerfile 三阶段构建会自动覆盖）
- 缓存权限错误不会在启动期暴露，运维需通过日志发现

## Definition of Done

- `cargo build --release` 通过，产物单一二进制（无 `dist` 文件夹依赖）
- `cargo test` 全绿
- `cargo clippy` 无 warning
- e2e：本地 release 二进制 + 临时目录跑通登录→主页→标签树→文件列表闭环
- README 或部署笔记补充新环境变量（最小记录，不写完整部署文档，部署文档留给任务二）

## Out of Scope

- Dockerfile / docker-compose.yml（任务二）
- 完整部署文档（任务二）
- Postgres/MySQL 等其他数据库后端
- YAML/TOML 配置文件支持（仅环境变量）
- build.rs 自动触发 `npm run build`（开发流程改动留给后续）
- 前端构建产物的版本/hash 管理

## Technical Notes

- M9 文档 `doc/开发阶段/Milestone 9：部署、容器化与产品化实现.md` 第 1 节给出 rust-embed 示例
- 现有 `infra/thumbnail.rs::ThumbnailGenerator::new` 接收 `cache_dir: String`，配置外部化只需在调用点传入新值
- `axum 0.7` 的 fallback API：`Router::fallback(handler)`
- `rust-embed 8.x` 的 `#[derive(RustEmbed)]` + `#[folder = "..."]`，运行时通过 `Asset::get(path)` 读取
- `mime_guess` crate 推断 Content-Type
- `tower-http` 是否需要：取决于是否额外需要压缩/缓存头，MVP 不引入
