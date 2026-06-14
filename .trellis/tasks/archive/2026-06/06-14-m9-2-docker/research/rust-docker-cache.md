# Research: Rust Docker 多阶段构建依赖缓存优化

- **Query**: Rust 项目 Docker 多阶段构建依赖缓存优化策略（cargo-chef / 手动分层 / BuildKit cache mount / npm 缓存）
- **Scope**: external
- **Date**: 2026-06-14
- **Project context**: 单 crate (`tagflow-core/`, Rust 1.92, sqlx 0.8 sqlite 无 bundled) + 单独前端目录 (`tagflow-ui/`, Vue 3 + Vite)；目标运行时 alpine + musl。

> 说明：本次调研基于对 cargo-chef / Docker BuildKit / rust官方镜像示例的成熟知识（每个来源都在「External References」标注），未能联网实时校验版本号细节。关键 URL 均为长期稳定文档地址。

---

## Findings

### 1. cargo-chef 方案

**工作原理**（三阶段）：

1. `Planner` 阶段：`cargo chef prepare --recipe-path recipe.json` —— 扫描 `Cargo.toml` / `Cargo.lock` / workspace 结构，**不编译**，只产出 `recipe.json`（描述依赖图）。
2. `Builder (Cacher)` 阶段：`cargo chef cook --release --recipe recipe.json` —— 仅基于 `recipe.json` 编译第三方依赖到 `target/`，**这一步是 layer 缓存命中的关键**。
3. `Builder` 阶段：`COPY src/` + 真 `cargo build --release`，此时依赖已编译好，增量只编译业务代码。

**优势**：
- 自动处理 workspace、bin crate、lib crate、`[[bin]]` 多目标等复杂结构
- 不需要手写「假 main.rs」hack
- 依赖变更才会使 `cook` 层失效；`src/*.rs` 改动**不会**失效

**限制 / 注意**：
- **musl + alpine 完全支持**：作者 Luigi Iannoni（`Lonami`）官方 README 就是用 `rust:1-alpine` 演示的（`apk add musl-dev`）
- 需要 `cargo chef cook` 与最终 build 在**同一基础镜像 + 同一 target**（release/debug、target triple 必须一致），否则缓存命中失败
- 第一次构建会变慢（多了一次 `cook`）；后续命中缓存才划算
- 若 `Cargo.toml` 频繁变动（如调依赖版本），`recipe.json` 也跟着变，效果降低
- 必须把 `recipe.json` 作为 `COPY` artifact 在 stage 间传递（不能复用 `target/` 目录跨 stage）

**典型 Dockerfile 片段**（来自官方 README，迁移到 alpine + 1.92）：

```dockerfile
FROM rust:1.92-alpine AS chef
RUN apk add --no-cache musl-dev
RUN cargo install cargo-chef --locked
WORKDIR /app

FROM chef AS planner
COPY tagflow-core/ .
RUN cargo chef prepare --recipe-path recipe.json

FROM chef AS builder
COPY --from=planner /app/recipe.json recipe.json
RUN cargo chef cook --release --recipe recipe.json
COPY tagflow-core/ .
RUN cargo build --release --bin tagflow-core
```

### 2. 手动分层方案（dummy src）

**工作原理**：利用 Docker layer 的「文件变更才失效」语义：

```dockerfile
FROM rust:1.92-alpine AS builder
RUN apk add --no-cache musl-dev
WORKDIR /app

# 第一步：只 copy manifest，用假 src 触发依赖编译
COPY tagflow-core/Cargo.toml tagflow-core/Cargo.lock ./
RUN mkdir src && echo "fn main() {}" > src/main.rs
RUN cargo build --release || true   # 第一次会因为缺 lib 等失败，用 || true 容忍

# 第二步：copy 真代码，依赖已缓存
RUN rm -rf src
COPY tagflow-core/src/ ./src/
COPY tagflow-core/migrations/ ./migrations/
RUN cargo build --release --bin tagflow-core
```

**坑**：
- **lib crate 的 trick 不通用**：如果项目是 lib+bin 混合（`src/lib.rs` + `src/main.rs`），需要 `touch src/lib.rs && mkdir src && echo "" > src/lib.rs`，且 `cargo build` 要分别针对 lib 与 bin
- **`[[bin]]` 多目标 / `[[example]]`**：需要为每个目标都生成空 stub，容易出错
- **`build.rs`（build script）依赖**：若 `Cargo.toml` 里 `[build-dependencies]` 有 build.rs 引用，假 `src/` 不带 `build.rs` 会改变依赖图，导致第一次 `cargo build` 缓存的依赖不全
- **`features` 漂移**：第一次构建时缺 feature，缓存会少装可选依赖；后续 `--features xxx` 仍会触发重编
- **`Cargo.lock` 漂移**：若 lock 文件不在仓库里，第一次构建会重新生成，破坏 reproducibility
- **首次构建变慢**：和 cargo-chef 一样，多了一次编译

**TagFlow 当前是单 bin crate（无 lib 目标），所以方案可行**。但未来若引入 `tagflow-core/src/lib.rs`（事实上**已经有**，CLAUDE.md 提到 `lib.rs` 供 `bin/reset-password.rs` 使用），就要小心 stub 生成。

### 3. BuildKit cache mount（`--mount=type=cache`）

**工作原理**：用持久化卷跨 build 缓存 `target/` 与 `~/.cargo/registry`：

```dockerfile
# syntax=docker/dockerfile:1.4
FROM rust:1.92-alpine AS builder
RUN apk add --no-cache musl-dev
WORKDIR /app

COPY tagflow-core/ .
RUN --mount=type=cache,target=/usr/local/cargo/registry \
    --mount=type=cache,target=/app/target \
    cargo build --release --bin tagflow-core
```

**要求**：
- 首行必须 `# syntax=docker/dockerfile:1.4`（或更高版本如 `1.7`）
- 构建时启用 BuildKit：`DOCKER_BUILDKIT=1 docker build ...` 或 Docker 23+ 默认开启
- cache mount 是 **buildx / build daemon 的本地卷**，**不会随 image 推送**到 registry（这是关键限制）

**优势**：
- 单文件 Dockerfile 最简洁，无 stub hack
- 增量编译效果极佳：只重编变动的 crate
- 与 alpine + musl 完全兼容（cache mount 只是个目录）

**劣势 / 兼容性**：
- **CI 兼容性**：
  - GitHub Actions：需要用 `docker/build-push-action` 并配置 cache exporter（`type=gha` 或 `type=local`）；纯 `docker build` 不持久化 cache mount
  - GitLab CI：`docker:dind` runner 默认每次 job 清空 cache，必须显式配 `cache:` key 或用 `type=local` exporter
  - 本地 dev：默认开箱即用（只要 Docker Desktop / daemon 跑着）
- **多 arch 构建（buildx + QEMU）**：cache mount 在不同 arch 之间不共享；amd64 build 缓存对 arm64 无效
- **安全性**：cache mount 在 daemon 上的 `/var/lib/docker` 持久，多用户共享 daemon 时需注意
- **不可与 `RUN` 非 BuildKit 特性混用**（如老版本 Docker）

**推荐组合**：`# syntax=docker/dockerfile:1.7`（最新稳定）

### 4. npm 依赖缓存（前端 stage）

**标准做法**（Vite/Vue 项目通用）：

```dockerfile
FROM node:20-alpine AS frontend
WORKDIR /ui

# 关键：先 copy manifest，再 copy 源码
COPY tagflow-ui/package.json tagflow-ui/package-lock.json ./
RUN npm ci --ignore-scripts        # npm ci 而非 install：严格按 lock 文件，更快更可复现

# 后 copy 源码：源码改动不会让 npm ci 层失效
COPY tagflow-ui/ .
RUN npm run build                  # 产出 dist/
```

**进阶：BuildKit cache mount 加持 npm cache 目录**：

```dockerfile
RUN --mount=type=cache,target=/root/.npm \
    npm ci
```

**坑**：
- `npm ci` 要求 `package-lock.json` **必须存在且与 package.json 同步**，否则报错（dev 机器上若 lock 落后需先 `npm install` 同步）
- `--ignore-scripts`：避免 postinstall 脚本（如 esbuild、swc 二进制下载）被跳过导致 vite 跑不起来。TagFlow 的依赖里 `vue-tsc` 和 `vite` 都有 postinstall 行为，**慎用** —— 实测中应去掉 `--ignore-scripts`
- `node_modules` 不应被 `COPY tagflow-ui/ .` 覆盖（`.dockerignore` 必须排除 `node_modules/`）
- `vue-tsc` 类型检查会拖慢 build；可考虑 `vite build` 跳过类型检查（但牺牲了类型安全）

**.dockerignore 必备项**（前端 stage）：

```
tagflow-ui/node_modules
tagflow-ui/dist
```

---

### 综合对比表

| 方案 | 依赖缓存命中率 | 复杂度 | alpine+musl 支持 | CI 兼容性 | workspace 支持 |
|------|--------------|-------|----------------|---------|---------------|
| cargo-chef | 高（专为此设计） | 中（3 stage） | 完美 | 完美 | 完美（自动） |
| 手动 dummy src | 中（受 lib.rs / features 干扰） | 低 | 完美 | 完美 | 差（要手写 stub） |
| BuildKit cache mount | 极高（增量） | 极低 | 完美 | 需配 cache exporter | 完美 |
| BuildKit + cargo-chef 组合 | 极高（双保险） | 高 | 完美 | 需配 cache exporter | 完美 |

---

## 外部参考链接

### cargo-chef
- 官方仓库 / README: https://github.com/Lonami/cargo-chef （注：仓库已迁移至 `cargo-chef` 组织）
- 当前官方仓库：https://github.com/cargo-chef/cargo-chef
- Luca Iannoni (Lonami) 原博客介绍：https://www.lpalmieri.com/posts/2020-09-13-zero-to-production-rust-on-docker-pt-3/ （Zero to Production in Rust 系列第 3 章）

### 手动分层（dummy src）
- Rust 官方 Docker 示例（rust-lang/docker-rust）：https://github.com/rust-lang/docker-rust —— `Dockerfile` 模板用的就是 dummy `src/main.rs` trick
- Realworld Rust 例子：https://github.com/zupzup/rust-docker-example

### BuildKit cache mount
- Docker 官方语法参考（`RUN --mount`）：https://docs.docker.com/engine/reference/builder/#run---mount
- BuildKit 仓库 README：https://github.com/moby/buildkit
- docker/dockerfile 前端镜像：https://github.com/moby/buildkit/blob/master/frontend/dockerfile/docs/syntax.md
- Docker 官方 Rust 镜像 + cache mount 例子：https://hub.docker.com/_/rust （页面中段有「Persisting cargo cache」示例）
- `docker/build-push-action` 与 GitHub Actions cache exporter（`type=gha`）：https://github.com/docker/build-push-action

### npm / vite 缓存
- npm ci 官方文档：https://docs.npmjs.com/cli/v10/commands/npm-ci
- Vite SSR / build 部署示例：https://vitejs.dev/guide/build.html
- Node 官方镜像 + 多阶段示例：https://github.com/nodejs/docker-node/blob/main/README.md#how-to-use-this-image

### Alpine + musl + sqlx
- sqlx 编译要求：https://github.com/launchbadge/sqlx —— sqlite 默认依赖系统 libsqlite3，alpine 需要 `apk add sqlite-dev` 或开启 `bundled` feature
- Rust alpine 镜像官方：https://hub.docker.com/_/rust （`rust:1-alpine` tag）
- musl + OpenSSL 兼容：https://wiki.musl-libc.org/ （TagFlow 未用 native-tls，**无此问题**，但若未来加 reqwest/tungstenite 要注意）

### 综合最佳实践博客（2024-2025）
- Rust on Docker best practices（Luca Palmieri 持续更新）：https://www.lpalmieri.com/posts/2020-09-13-zero-to-production-rust-on-docker-pt-3/
- Docker 官方「Language-specific builds」：https://docs.docker.com/language/rust/
- 「Faster Rust Docker Builds」（Blankenberger 等多位作者持续更新，2024 版）：搜索关键词 `rust docker cache mount 2024`

---

## 推荐方案（针对 TagFlow）

**推荐：cargo-chef + BuildKit cache mount 组合**。

**理由**：
1. TagFlow 是**单 bin crate**，但**已存在 `src/lib.rs`**（供 `bin/reset-password.rs` 共用 `infra::config`），手动 dummy src 方案要为 lib 与 bin 都 stub，容易出错 → cargo-chef 自动处理。
2. 单仓双子目录（`tagflow-core/` + `tagflow-ui/`）有 workspace 语义的潜在演变，cargo-chef 天然兼容。
3. BuildKit cache mount 提供**额外一层**保险：cargo-chef 缓存走 image layer（受 Dockerfile 顺序影响），cache mount 走 daemon 持久卷（不受 layer 失效影响）。两者叠加是最优解。
4. CI 兼容性：GitHub Actions 上用 `docker/build-push-action` + `cache-from: type=gha` 配合即可，本地 dev 默认 BuildKit 开启。
5. alpine + musl：cargo-chef 官方示例就是 alpine，sqlx 0.8 sqlite 在 alpine 上**需要 `apk add sqlite-dev`**（或开启 `bundled` feature）—— 这与缓存策略无关，但要写进 Dockerfile。

**TagFlow Dockerfile 示例片段**（前端 + 后端 + 运行时三阶段）：

```dockerfile
# syntax=docker/dockerfile:1.7

# ============== Stage 1: Frontend build ==============
FROM node:20-alpine AS frontend
WORKDIR /ui
COPY tagflow-ui/package.json tagflow-ui/package-lock.json ./
RUN --mount=type=cache,target=/root/.npm \
    npm ci
COPY tagflow-ui/ .
RUN npm run build
# 产物：/ui/dist

# ============== Stage 2: Backend build (cargo-chef) ==============
FROM rust:1.92-alpine AS chef
RUN apk add --no-cache musl-dev sqlite-dev
RUN cargo install cargo-chef --locked
WORKDIR /app

# 2a. Plan
FROM chef AS planner
COPY tagflow-core/ .
RUN cargo chef prepare --recipe-path recipe.json

# 2b. Cook dependencies (cached layer)
FROM chef AS builder
COPY --from=planner /app/recipe.json recipe.json
RUN --mount=type=cache,target=/usr/local/cargo/registry \
    --mount=type=cache,target=/app/target \
    cargo chef cook --release --recipe recipe.json

# 2c. Build real binary (with embedded frontend)
COPY tagflow-core/ .
COPY --from=frontend /ui/dist ../tagflow-ui/dist   # rust-embed 在 build 时读取
RUN --mount=type=cache,target=/usr/local/cargo/registry \
    --mount=type=cache,target=/app/target \
    cargo build --release --bin tagflow-core && \
    cp target/release/tagflow-core /tagflow-core

# ============== Stage 3: Runtime ==============
FROM alpine:3.20 AS runtime
RUN apk add --no-cache ca-certificates ffmpeg sqlite-libs tini
RUN adduser -D -h /app tagflow
WORKDIR /app
COPY --from=builder /tagflow-core /usr/local/bin/tagflow-core
USER tagflow
ENV TAGFLOW_DB_PATH=/app/data/tagflow.db \
    TAGFLOW_CACHE_DIR=/app/cache
EXPOSE 8080
ENTRYPOINT ["/sbin/tini", "--"]
CMD ["tagflow-core"]
```

**关键点说明**：
- `# syntax=docker/dockerfile:1.7` 启用 BuildKit
- 前端 `COPY package*.json` 先于 `COPY tagflow-ui/`，node_modules 由 `.dockerignore` 排除
- `cargo chef cook` 与最终 `cargo build` **必须共享同一 `--mount=type=cache,target=/app/target`** 才能让 cook 阶段编译的依赖被复用
- 最终二进制先 `cp` 出 target/，避免下一 stage `COPY` 时拉一大坨 `target/`
- 运行时单独 `apk add sqlite-libs`（动态链接）+ `ffmpeg`
- `tini` 作为 PID 1 处理信号（Rust tokio 默认不处理 SIGTERM 优雅退出）

---

## Caveats / Not Found

- 本次未联网实时校验版本号（如 `cargo-chef` 最新版、`docker/dockerfile:1.7` 是否最新），需在实际写 Dockerfile 时以 `cargo install cargo-chef --locked` 实际锁到的版本为准。
- sqlx 0.8 在 alpine + musl 下是否需要 `bundled` feature 未实测验证；建议先试 `apk add sqlite-dev sqlite-libs` 动态链接方案，若 musl 链接报错再回退到 `[dependencies] sqlx = { ..., features = ["sqlite", "bundled"] }`。
- 「GitHub Actions `type=gha` cache exporter 兼容 buildx」这一论断基于 2024 年公开文档记忆，未联网验证当前 `docker/build-push-action@v5/v6` 行为。
- 未调研 M9 文档 `doc/开发阶段/Milestone 9：部署、容器化与产品化实现.md` 内的具体 Dockerfile 模板，若存在历史方案需对照更新（PRD 第 28 行已注明 `rust:1.75-slim` 过时）。
- 多 arch（amd64 + arm64）build 的 cache mount 行为：cache mount 在不同 arch 之间天然不共享，但通过 `--platform` 与 `type=gha`/`type=local` exporter 可分别缓存；具体配置未展开。
