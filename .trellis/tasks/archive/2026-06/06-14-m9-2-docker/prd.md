# M9-2: Docker 化与部署文档

## Goal

在 M9-1 单二进制基础上完成产品化部署闭环：交付一份多阶段 Dockerfile 与 docker-compose.yml，使用户可以通过 `docker compose up -d` 一键启动 TagFlow（前端 + 后端 + FFmpeg 运行时），数据/缓存/资源库通过卷映射持久化。同时配套部署文档说明环境变量、卷、首次启动凭据、常见运维操作。

## What I already know

### M9-1 已交付（前置条件）

- `cargo build --release` 产出单二进制，前端 dist 已通过 `rust-embed` 嵌入
- 环境变量：`TAGFLOW_PORT`（默认 8080）、`TAGFLOW_JWT_SECRET`（release 强制 ≥32B）、`TAGFLOW_ADMIN_PASSWORD`（release 强制 ≥12B）、`TAGFLOW_ADMIN_USERNAME`、`TAGFLOW_DB_PATH`（默认 `tagflow.db`）、`TAGFLOW_CACHE_DIR`（默认 `./cache`）
- 缺省 `tagflow.db` 与 `./cache` 都是相对当前工作目录的路径
- `reset-password` CLI 工具与主程序共用 `infra::config::db_url()`

### 项目结构约束

- 单仓双子目录：`tagflow-core/`（Rust）+ `tagflow-ui/`（Vue 3 + Vite）
- Rust 工具链要求 1.92.0+（CLAUDE.md 明确）
- 前端构建依赖 Node 18+
- 缩略图生成依赖外部 `ffmpeg` 命令在 PATH 中
- 数据库使用 SQLite，启用 WAL 模式

### M9 文档原始方案（需要更新）

- 第 2 节给出三阶段 Dockerfile 框架：node:20-alpine（前端）→ rust:1.75-slim（后端）→ alpine:latest（运行时）
- 第 3 节给出 docker-compose.yml 框架，含 data/cache/library 卷映射 + TZ 设置
- **过时点**：`rust:1.75-slim` 已不满足 1.92.0+ 要求；运行时 `alpine:latest` 需重新考虑 SQLite/sqlx 兼容

## Assumptions (temporary)

- Dockerfile 放在仓库根目录（跨 tagflow-core 与 tagflow-ui 双包）
- 运行时容器以非 root 用户运行（安全基线）
- 镜像 tag 默认 `tagflow:latest`，可选 version tag
- 多 arch 支持（amd64 + arm64）以便 NAS 用户使用
- 部署文档放在 `doc/部署指南.md`（与现有 `doc/` 风格一致）

## Open Questions

- Q1: 运行时基座 Alpine vs Debian-slim（research 给出冲突建议，需用户拍板）
- 其余 Q2-Q6 已基于 research 收敛，见 Decision 节

## Requirements (evolving)

1. 仓库根目录新增 `.dockerignore`（排除 target/、node_modules/、.git/、tagflow.db 等）
2. 仓库根目录新增 `Dockerfile`（多阶段构建：前端 → 后端 → 运行时）
3. 仓库根目录新增 `docker-compose.yml`（数据/缓存/资源库卷映射、环境变量示例、重启策略）
4. 新增 `doc/部署指南.md`（最小可用：环境变量清单、卷规划、首次启动、常见运维操作）
5. 运行时镜像以非 root 用户运行
6. 镜像内 ffmpeg 可用
7. SQLite/缩略图缓存通过卷持久化

## Acceptance Criteria (evolving)

- [ ] `.dockerignore` 排除构建无关文件，避免上下文过大
- [ ] `docker build -t tagflow:latest .` 在干净环境一次构建成功
- [ ] 运行时镜像体积合理（目标 < 200MB，文档承诺 80MB 留余地）
- [ ] `docker compose up -d` 启动后，浏览器访问 `http://localhost:8080` 可见登录页
- [ ] 首次启动通过环境变量注入 admin 密码，不再 fail-fast
- [ ] 数据库与缓存目录持久化（容器删除后重建数据仍在）
- [ ] ffmpeg 缩略图生成可用（可手动触发扫描并验证 `cache/` 生成 `.webp`）
- [ ] 容器以非 root 用户运行（`docker exec <c> id` 输出非 0 UID）
- [ ] 部署文档列全环境变量、卷、首次启动步骤、reset-password 容器内用法
- [ ] e2e：`docker compose down -v && docker compose up -d --build` 全新环境可重建

## Definition of Done

- `docker build` + `docker compose up` 通过本地 e2e
- 部署文档自包含（用户无需读源码即可部署）
- README.md 顶层新增「快速开始（Docker）」一节，链到部署指南
- 缩略图生成在容器内验证可用（FFmpeg 命令在 PATH）

## Out of Scope

- Kubernetes manifests / Helm chart
- 镜像签名（cosign）/ SBOM
- CI/CD 自动构建推送镜像仓库（留给后续任务）
- TLS 终止 / 反向代理（Caddy/Nginx）配置
- 资源限制（mem_limit/cpu）调优（部署文档给出建议值即可，不在 compose 强制）
- WebDAV 资源库支持（M9 后的独立里程碑）

## Technical Notes

- M9 文档 `doc/开发阶段/Milestone 9：部署、容器化与产品化实现.md` 第 2-4 节给出框架（部分版本号需更新）
- 当前 Cargo.toml：sqlx 0.8 features = ["runtime-tokio", "sqlite", "chrono", "macros"]，无 `bundled`/`native-tls`，需确认 Alpine 下编译
- 数据库连接池初始化设置 `PRAGMA journal_mode = WAL` + `PRAGMA foreign_keys = ON`（infra/db.rs）
- 前端构建产物路径 `tagflow-ui/dist`，rust-embed 通过 `../tagflow-ui/dist/` 嵌入
- `TAGFLOW_DB_PATH` 默认相对路径，Docker 中应配置为绝对路径（如 `/app/data/tagflow.db`）

## Research References

- [`research/rust-alpine-sqlx.md`](research/rust-alpine-sqlx.md) — sqlx 0.8 sqlite 已 bundled+static，运行时无需 sqlite-libs；构建与运行必须同 libc
- [`research/rust-docker-cache.md`](research/rust-docker-cache.md) — cargo-chef + BuildKit cache mount 组合最优；项目 `src/lib.rs` 使手动 dummy src 方案易踩坑
- [`research/sqlite-persistence-multiarch.md`](research/sqlite-persistence-multiarch.md) — SQLite WAL 官方明确不支持网络 FS；multi-arch 建议 defer 到 M9-3

## Research-Derived Decisions（基于调研，待用户确认）

| 决策 | 选项 | 推荐 |
|------|------|------|
| 缓存策略 | cargo-chef + cache mount / 手动 dummy / 无 | **cargo-chef + cache mount**（研究 2） |
| data 持久化 | named volume / bind mount / 两者皆示例 | **named volume `tagflow_data`**（研究 3，规避 NFS/WAL 风险） |
| cache/library 持久化 | named volume / bind mount | **bind mount**（webp 可重建、library 只读源） |
| 非 root 用户 | UID 1000 + chown / entrypoint chown / userns-remap | **UID 1000 + 文档要求 chown**（最简单） |
| multi-arch | M9-2 实现 / defer 到 M9-3 | **defer 到 M9-3**（研究 3 强烈建议；QEMU 慢、cross 需独立验证） |
| 镜像分发 | 仅本地 build / GH Actions 推 GHCR | **仅本地 build**（CI 留给 M9-3） |
| 健康检查 endpoint | 新增 `/api/health` / 用 TCP probe | **新增 `/api/health`**（无依赖、利于编排） |
| 资源库挂载示例 | compose 示例 / 文档说明 | **compose 示例注释掉**（用户取消注释即用） |

## ADR-lite

**Context**：M9 文档原始方案（rust:1.75-slim + alpine:latest）版本号过时；Alpine 与 Debian-slim 在 sqlx+musl+ffmpeg 维度有不同取舍；multi-arch 增加构建复杂度。

**Decision（research 已收敛的，待用户确认补充 Q1）**：
- 缓存：cargo-chef + BuildKit `--mount=type=cache`
- 持久化：data named volume / cache+library bind mount
- 非 root UID 1000
- multi-arch defer 到 M9-3
- 仅本地 build（无 CI/CD）
- 新增 `/api/health` 端点
- compose 注释示例 library 挂载

**Consequences**：
- NAS 用户首次部署需 `chown -R 1000:1000 ./cache` 一步
- ARM NAS 用户（如 Synology DS423+）M9-2 内不可用，需等 M9-3
- NFS/SMB 共享 data 目录会触发 SQLite WAL 数据损坏（部署文档必须警告）
- 镜像构建首次较慢（cargo-chef cook 全量编依赖 + bundled SQLite C 源），后续命中缓存快
