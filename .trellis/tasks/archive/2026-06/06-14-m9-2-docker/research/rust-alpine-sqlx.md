# Research: Rust + Alpine musl + sqlx 0.8 (sqlite) Docker 多阶段构建

- **Query**: Rust 后端 + Alpine musl + sqlx 0.8 (sqlite) 在 Docker 多阶段构建中的兼容性与最佳实践
- **Scope**: external (主要) + internal (验证 Cargo.toml)
- **Date**: 2026-06-14

## TL;DR（≤400 字）

1. **sqlx 0.8 `features=["sqlite"]` 已经等于"bundled + 静态链接 SQLite"**——SQLite C 源码会被 `cc` crate 编进二进制，运行时镜像**不需要** `sqlite-libs`。要切到系统库才用 `sqlite-unbundled`（不推荐 Alpine）。TagFlow 当前 Cargo.toml 完美匹配 Alpine 静态方案。
2. **TLS 完全不需要**。sqlx 0.8 拆了 TLS feature：旧 `tls-rustls` 在 0.8 已删除，新名字是 `tls-rustls-ring-webpki` / `tls-rustls-aws-lc-rs`。TagFlow 只连本地 SQLite，无需 TLS——`runtime-tokio` + `tls-none`（默认）即可。
3. **argon2 0.5 + jsonwebtoken 9.2 + rand_core 0.6 + getrandom** 全是纯 Rust，musl 无需额外库。getrandom 在 musl 用 `getrandom()` syscall，无需 `/dev/urandom` 容器配置（Alpine 默认就有）。
4. **构建阶段绝对不能"slim 构建 + alpine 运行"混搭**：glibc 与 musl ABI 不兼容，二进制会启动即 `no such file or directory`（ld-linux 报错）。**构建基座与运行基座的 libc 必须一致**。两套合理方案：纯 alpine 全程，或纯 debian 全程。
5. **运行时 Alpine 必需 apk 包**：`ca-certificates`（HTTPS）、`ffmpeg`（缩略图）。SQLite 已静态链入，`sqlite-libs` **不要装**。Rust 二进制依赖 musl libc（alpine base 已含）+ 可能的 `libgcc`（alpine base 已含），不需要单独装。
6. **`RUSTFLAGS='-C target-feature=+crt-static'`** 在 musl target (`x86_64-unknown-linux-musl`) 下默认就开启。**不用手动指定**。但 alpine 镜像里 `cargo build` 默认 host triple 就是 musl，无需 `--target`。在 debian 上要 `rustup target add musl`+`--target`，不推荐。
7. **`rust:1.92-alpine` 已有 `ca-certificates`、`musl-dev`、`gcc`**——直接可编 libsqlite3-sys。

## Findings

### 1. sqlx 0.8 features 实际含义（权威来源）

**官方 README**（`launchbadge/sqlx/main/README.md`）原文：

> - `sqlite`: Add support for the self-contained SQLite database engine with **SQLite bundled and statically-linked**.
> - `sqlite-unbundled`: The same as above (`sqlite`), but **link SQLite from the system** instead of the bundled version.

——这意味着 `features=["sqlite"]` 已经是 bundled+static，**运行镜像无需安装 `sqlite-libs`/`libsqlite3`**。

源码层验证 `sqlx-sqlite/Cargo.toml`：

```toml
bundled = ["libsqlite3-sys/bundled"]
[dependencies.libsqlite3-sys]
version = ">=0.30.1, <0.38.0"
default-features = false
features = ["pkg-config", "vcpkg"]
```

`libsqlite3-sys 0.38` 的 `bundled` feature 拉入 `cc` crate 从源码编 SQLite（`rusqlite/rusqlite/master/libsqlite3-sys/Cargo.toml`）。

TLS feature 在 0.8 重命名（README 112-155 行）：

| 旧 (0.7) | 新 (0.8) |
|---|---|
| `tls-rustls` | `tls-rustls-ring-webpki` / `tls-rustls-ring-native-roots` / `tls-rustls-aws-lc-rs` |
| `tls-native-tls` | 保留同名 |
| 不需要 TLS | `tls-none`（默认，无需声明） |

**对 TagFlow 的影响**：当前 Cargo.toml 不写 TLS feature，等同 `tls-none`，正确无误。

链接：
- https://github.com/launchbadge/sqlx/blob/main/README.md
- https://github.com/launchbadge/sqlx/blob/main/sqlx-sqlite/Cargo.toml
- https://github.com/rusqlite/rusqlite/blob/master/libsqlite3-sys/Cargo.toml

### 2. argon2 / jsonwebtoken / rand_core 在 musl 下的行为

| crate | 版本（TagFlow） | 实现 | musl 注意 |
|---|---|---|---|
| `argon2` | 0.5（最新 0.5.5） | 纯 Rust（crates.io 描述确认） | 无 |
| `jsonwebtoken` | 9.2（最新 10.4） | 纯 Rust | 无 |
| `rand_core` 0.6 + `getrandom` | getrandom 0.2.x | Linux 用 `getrandom` syscall | 无需 `/dev/urandom` 配置 |

**getrandom README**（`rust-random/getrandom/master/README.md`）原文：

> | Linux, Android | `*‑linux‑*` | `getrandom` system call if available, otherwise `/dev/urandom` after successfully polling `/dev/random` |

——Linux musl 完整支持，无外部依赖。

**结论**：TagFlow 这些依赖在 musl 下零特殊处理。

链接：
- https://github.com/rust-random/getrandom/blob/master/README.md
- https://crates.io/crates/argon2/0.5.5
- https://crates.io/crates/jsonwebtoken

### 3. 构建基座选择：rust:alpine vs rust:slim（关键陷阱）

**官方 rust Docker 镜像 Dockerfile 模板**（`rust-lang/docker-rust/master/`）：

**`Dockerfile-alpine.template`**（基础镜像 `alpine:TAG`，含 `ca-certificates` + `musl-dev` + `gcc`）：

```dockerfile
FROM alpine:%%TAG%%
RUN apk add --no-cache ca-certificates musl-dev gcc
```

**`Dockerfile-slim.template`**（基础镜像 `debian:TAG-slim`，含 `ca-certificates` + `gcc` + `libc6-dev`）：

```dockerfile
FROM debian:%%TAG%%-slim
RUN apt-get install -y ca-certificates gcc libc6-dev wget
```

**核心规则**：构建阶段与运行阶段**必须同 libc**。

- `rust:slim`（glibc）编译出来的 ELF 用 `ld-linux-x86-64.so.2` + `libc.so.6` 动态链接。直接扔进 alpine（musl）运行，启动时会因找不到解释器报 `not found`/`no such file or directory`（内核 errno 是 ENOENT，错误信息具有误导性）。
- 反之亦然。

**TagFlow 推荐**：全 alpine 链路。理由：
1. 单一基座 libc 风险最低。
2. sqlx `bundled` SQLite 在 alpine + `musl-dev` + `gcc` 下原生编译，无需 cross-compile。
3. 最终镜像 ~80MB 可达成。
4. `rust:1.92-alpine` 已预装所有编译依赖。

链接：
- https://github.com/rust-lang/docker-rust/blob/master/Dockerfile-alpine.template
- https://github.com/rust-lang/docker-rust/blob/master/Dockerfile-slim.template
- https://hub.docker.com/_/rust

### 4. 运行时 Alpine apk 包清单

**必需**：

| 包 | 理由 |
|---|---|
| `ca-certificates` | HTTPS 根证书（虽然 TagFlow 本地不请求外部，但未来 WebDAV、HTTP 客户端等基础） |
| `ffmpeg` | 缩略图生成外部命令（CLAUDE.md 已明确） |

**不要装**：

| 包 | 原因 |
|---|---|
| `sqlite-libs` | sqlx 已 bundled，重复会冲突或浪费 |
| `libgcc` | alpine base 默认含，且 Rust 静态链入部分 |
| `libstdc++` | Rust 不依赖 libstdc++（用自身 stdlib） |
| `musl-dev` | 运行时不需要（仅构建期） |
| `gcc` | 运行时不需要（除非要 JIT，Rust 不需要） |

**Tini（推荐但可选）**：`tini` 作为 PID 1 处理信号——`apk add tini` + `ENTRYPOINT ["/sbin/tini","--"]`，让 SIGTERM 优雅关闭 axum。

### 5. RUSTFLAGS='-C target-feature=+crt-static' 是否必需

**答案：在 musl target 下默认开启，不用手写。**

`x86_64-unknown-linux-musl` target spec 里 `crt-static` 默认为 `true`（rustc 内置默认）。Rust std 文档明确：musl 默认 `crt-static-respected = yes` 且 `crt-static-default = true`。

**在 alpine 镜像里 `cargo build --release`**：host triple 就是 `x86_64-unknown-linux-musl`，自动静态链接，无需 `--target`、无需 RUSTFLAGS。

**在 debian 镜像里编译 musl**（不推荐）：需要 `rustup target add x86_64-unknown-linux-musl` + `--target` + 安装 musl-tools，繁琐且 linker 易错。**避免**。

### 6. Dockerfile 候选方案对比

#### 方案 A（推荐）：全 Alpine 链路

```dockerfile
# 前端构建
FROM node:20-alpine AS ui
WORKDIR /ui
COPY tagflow-ui/package*.json ./
RUN npm ci
COPY tagflow-ui/ ./
RUN npm run build

# 后端构建
FROM rust:1.92-alpine AS backend
WORKDIR /build
RUN apk add --no-cache musl-dev   # rust:alpine 已含，保险
COPY tagflow-core/ ./
COPY --from=ui /ui/dist ../tagflow-ui/dist
ENV SQLX_OFFLINE=true
RUN cargo build --release --bin tagflow-core

# 运行时
FROM alpine:3.20
RUN apk add --no-cache ca-certificates ffmpeg tini
RUN adduser -D -h /app tagflow
WORKDIR /app
COPY --from=backend /build/target/release/tagflow-core /usr/local/bin/
USER tagflow
ENTRYPOINT ["/sbin/tini","--"]
CMD ["tagflow-core"]
```

| 维度 | 评价 |
|---|---|
| 镜像体积 | ~70-90MB（目标 < 200MB 充裕） |
| 构建复杂度 | 最简，无需 cross-target |
| sqlx/sqlite | 静态链入，零运行时依赖 |
| 风险 | 国内某些 NAS 的 Alpine 仓库镜像慢 |

#### 方案 B：全 Debian slim 链路（备选）

```dockerfile
FROM node:20-slim AS ui
...
FROM rust:1.92-slim AS backend
COPY tagflow-core/ ./
RUN cargo build --release --bin tagflow-core
FROM debian:bookworm-slim
RUN apt-get update && apt-get install -y --no-install-recommends \
    ca-certificates ffmpeg && rm -rf /var/lib/apt/lists/*
...
```

| 维度 | 评价 |
|---|---|
| 镜像体积 | ~120-180MB（ffmpeg 在 debian 拖大） |
| 构建复杂度 | 同样简单 |
| sqlx/sqlite | 静态链入（bundled），与方案 A 一致 |
| 风险 | ffmpeg 体积大；非 root 用户管理略复杂 |
| 优点 | glibc 兼容性最广；NAS 上更稳 |

#### 方案 C（不推荐）：slim 构建 + alpine 运行

| 维度 | 评价 |
|---|---|
| 镜像体积 | — |
| 构建复杂度 | — |
| sqlx/sqlite | — |
| 风险 | **glibc/musl 不匹配，启动即崩**——避免 |

### 7. 其他注意点

- **WAL 模式下的卷持久化**：SQLite WAL 需要共享卷支持 `mmap` 和文件锁——Docker named volume / bind mount 都 OK，但 **NFS 共享卷锁不可靠**（部署文档要警示）。
- **非 root 用户**：`alpine` 默认无 `adduser` 命令？有——BusyBox `adduser -D -h /app tagflow` 即可。SQLite + WAL 要求运行用户对 `data/` 与 `cache/` 目录可写。
- **multi-arch（amd64 + arm64）**：alpine arm64 镜像官方支持。`docker buildx build --platform linux/amd64,linux/arm64` 在 alpine 上无需特殊处理（musl + gcc 在 arm64 都 OK）。sqlx `bundled` 的 C 代码 `cc` 编译在两个架构都原生支持。
- **`SQLX_OFFLINE=true`**：Docker 构建环境无 DB 连接做编译时 query 校验，**必须在 `cargo build` 前设置**（或先 `cargo sqlx prepare --check`）。当前项目 `tagflow-core/.sqlx/` 是否有离线数据需 implement 阶段确认。
- **时间戳**：`TAGFLOW_DB_PATH` 默认相对路径，Docker 必须改成绝对路径 `/app/data/tagflow.db`（PRD 已写明）。`TAGFLOW_CACHE_DIR` 同理 → `/app/cache`。

## Related Specs

- `tagflow-core/Cargo.toml` — sqlx 0.8 features 现状（无 TLS、无 `unbundled`，完美匹配方案 A）
- `tagflow-core/src/infra/db.rs` — `init_db` 初始化 WAL + foreign_keys，不影响 Docker 选型
- `tagflow-core/migrations/` — 3 个迁移文件，运行时自动跑（无需 Docker 镜像内 SQL）
- `.trellis/tasks/06-14-m9-2-docker/prd.md` — 任务上下文（M9-2 Docker 化）

## Caveats / Not Found

- **未直接验证**：sqlx 0.8 + Rust 1.92 + Alpine 3.20 的实际编译耗时（首次 `cargo build` 因 `bundled` SQLite C 源编译约增加 30-60s，后续 Docker layer 缓存命中后忽略不计）。
- **未直接验证**：alpine 3.20 与 3.21 之间 ffmpeg 包版本差异对缩略图生成的潜在影响（参考 M8 已验证的 ffmpeg 命令行兼容性）。
- **未深入**：cargo-chef 对依赖层缓存的价值（PRD Open Question Q2）。若需要可在 implement 阶段单独研究，初步判断：sqlx + opendal 编译耗时大，cargo-chef 收益明显。
- **未覆盖**：buildx 跨架构构建在 Apple Silicon 上的 QEMU 性能（PRD Q5）。arm64 主机跑 amd64 镜像约 5-10x 慢。
