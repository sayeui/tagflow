# syntax=docker/dockerfile:1.7
#
# TagFlow 单二进制镜像：前端嵌入 + 后端静态链 + Alpine 运行时
#
# 构建命令（单架构）：
#   docker build -t tagflow:latest .
#
# 构建命令（多架构 amd64+arm64，需先启用 buildx 与 QEMU）：
#   docker run --privileged --rm tonistiigi/binfmt --install all   # 一次性
#   docker buildx create --use --name tagflow-builder
#   docker buildx build --platform linux/amd64,linux/arm64 \
#       -t tagflow:latest --load .
#
# 注意：amd64 host 用 QEMU 构建 arm64 预计 20-60 分钟（Rust release build）。
# M9-3 计划用 cross 工具替代，本地原生速度交叉编译。

# ============== Stage 1: Frontend build ==============
# 用 $BUILDPLATFORM 保证 builder 始终在 host 原生架构运行（QEMU 模拟会很慢）
FROM --platform=$BUILDPLATFORM node:20-alpine AS frontend
WORKDIR /ui
COPY tagflow-ui/package.json tagflow-ui/package-lock.json ./
RUN --mount=type=cache,target=/root/.npm \
    npm ci
COPY tagflow-ui/ .
RUN npm run build
# 产物：/ui/dist

# ============== Stage 2: Backend build (cargo-chef + BuildKit cache) ==============
FROM --platform=$BUILDPLATFORM rust:1.92-alpine AS chef
RUN apk add --no-cache musl-dev
RUN cargo install cargo-chef --locked
WORKDIR /app

# 2a. Planner：扫描 Cargo.toml/Cargo.lock 生成 recipe.json（不编译）
FROM chef AS planner
COPY tagflow-core/ ./
RUN cargo chef prepare --recipe-path recipe.json

# 2b. Builder：先 cook 依赖（layer 缓存关键），再编译业务代码
FROM chef AS builder
COPY --from=planner /app/recipe.json recipe.json
RUN --mount=type=cache,target=/usr/local/cargo/registry \
    --mount=type=cache,target=/app/target \
    cargo chef cook --release --recipe-path recipe.json
COPY tagflow-core/ ./
# rust-embed 在编译期读取 ../tagflow-ui/dist，必须先放置前端产物
COPY --from=frontend /ui/dist /tagflow-ui/dist
# 一次性构建全部 bin（tagflow-core 主程序 + reset-password CLI）
RUN --mount=type=cache,target=/usr/local/cargo/registry \
    --mount=type=cache,target=/app/target \
    cargo build --release --bins && \
    cp target/release/tagflow-core /tagflow-core && \
    cp target/release/reset-password /reset-password

# ============== Stage 3: Runtime ==============
# 用 $TARGETPLATFORM 输出目标架构镜像；multi-arch build 时此 layer 会按 arch 分别构建
FROM --platform=$TARGETPLATFORM alpine:3.20 AS runtime
RUN apk add --no-cache ca-certificates ffmpeg tini wget
# 非 root 用户 UID/GID 1000（部署文档要求宿主挂载目录 chown 1000:1000）
RUN addgroup -S -g 1000 tagflow && \
    adduser -S -D -H -u 1000 -G tagflow tagflow
WORKDIR /app
COPY --from=builder /tagflow-core /usr/local/bin/tagflow-core
COPY --from=builder /reset-password /usr/local/bin/reset-password
USER tagflow
# 容器内默认配置：DB 与 cache 用绝对路径，外部通过卷映射覆盖
ENV TAGFLOW_PORT=8080 \
    TAGFLOW_DB_PATH=/app/data/tagflow.db \
    TAGFLOW_CACHE_DIR=/app/cache
EXPOSE 8080
# HEALTHCHECK：每 30s 探测 /api/health；启动后 10s 才开始检查（给迁移与 worker 启动留时间）
HEALTHCHECK --interval=30s --timeout=5s --start-period=10s --retries=3 \
    CMD wget -qO- "http://127.0.0.1:${TAGFLOW_PORT}/api/health" || exit 1
# tini 作 PID 1：正确处理 SIGTERM，避免 axum 优雅退出被吞
ENTRYPOINT ["/sbin/tini", "--"]
CMD ["tagflow-core"]
