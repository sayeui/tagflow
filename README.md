# TagFlow

<div align="center">

**轻量级、非侵入式、支持层级标签的多源资源管理系统**

[![Rust](https://img.shields.io/badge/rust-1.92.0%2B-orange.svg)](https://www.rust-lang.org/)
[![Vue](https://img.shields.io/badge/vue-3.5%2B-green.svg)](https://vuejs.org/)
[![License](https://img.shields.io/badge/license-MIT-blue.svg)](LICENSE)

</div>

---

## 项目简介

TagFlow 是一个基于六边形架构设计的本地文件资源管理工具，通过层级标签系统实现对多源文件（本地磁盘、WebDAV）的统一管理与检索。系统采用 Rust 后端 + Vue 3 前端的技术栈，提供高性能的增量扫描和灵活的标签分类能力。

[产品蓝图](https://gemini.google.com/share/c1e4d6b68b2c)

### 核心特性

- **层级标签系统** - 支持树形标签结构，路径自动转换为嵌套标签
- **增量同步扫描** - 基于哈希的差异化检测，高效识别文件移动与变更
- **多存储协议** - 通过 OpenDAL 支持本地文件系统和 WebDAV
- **非侵入式设计** - 不修改原始文件，所有元数据独立存储
- **高性能查询** - SQLite + 优化的索引设计，10万+文件查询 <150ms
- **虚拟滚动** - 前端支持大规模数据流畅渲染
- **异步缩略图** - 后台任务流水线 + FFmpeg 为图片/视频生成缩略图
- **用户认证** - JWT + Argon2 密码哈希，安全的登录与会话管理
- **密码管理** - 支持密码修改和重置工具

---

## 快速开始

### 环境要求

- **Rust**: 1.92.0 或更高版本
- **Node.js**: 18+ (前端开发)
- **SQLite**: 3.35+ (自动通过 SQLx 集成)
- **FFmpeg**: 缩略图生成需要 `ffmpeg` 在 PATH 中

### 快速开始（Docker，推荐生产部署）

```bash
git clone https://github.com/sayeui/tagflow.git
cd tagflow

cp .env.example .env
# 编辑 .env 设置 TAGFLOW_JWT_SECRET（≥ 32B）与 TAGFLOW_ADMIN_PASSWORD（≥ 12B）
# 建议：openssl rand -hex 32 生成 JWT_SECRET

# cache = 缩略图缓存（webp，可重建）；容器以 UID 1000 运行，host ./cache 须归 1000 可写
# （后端启动会自动建目录，但 bind mount 下 host 目录默认 root 属主，不 chown 会写不进、缩略图生成失败）
mkdir -p ./cache && chown -R 1000:1000 ./cache
docker compose up -d            # 首次构建约 5-15 分钟
# 浏览器访问 http://localhost:8080
```

完整部署流程（环境变量、卷规划、备份、重置密码、multi-arch 构建）见 [`doc/部署指南.md`](doc/部署指南.md)。

### 源码编译运行（开发模式）

```bash
# 启动后端 (终端 1)
cd tagflow-core
cargo run
# API 服务运行在 http://localhost:8080
# 首次启动需通过 TAGFLOW_JWT_SECRET / TAGFLOW_ADMIN_PASSWORD 环境变量提供凭据

# 启动前端 (终端 2)
cd tagflow-ui
npm install
npm run dev
# 前端开发服务器 http://localhost:5173，/api 请求自动代理到 :8080
```

> Release 构建（`cargo build --release`）产出的单二进制已通过 `rust-embed` 嵌入前端产物，可脱离 npm 独立运行。

### 运行时配置（环境变量）

| 变量 | 默认 | 说明 |
|------|------|------|
| `TAGFLOW_PORT` | `8080` | API 监听端口 |
| `TAGFLOW_JWT_SECRET` | debug 构建回退开发默认值 | JWT 签名密钥（HS256，要求 ≥ 32 字节）。release 构建缺失或长度不足将启动失败。更换密钥会使所有已签发 token 失效 |
| `TAGFLOW_ADMIN_PASSWORD` | debug 构建回退开发默认值 | 首次启动且 `users` 表为空时创建管理员用密码（要求 ≥ 12 字节）。release 构建缺失或长度不足将启动失败；已有用户的部署不触发该校验 |
| `TAGFLOW_DB_PATH` | `tagflow.db` | SQLite 数据库文件路径（自动创建，启用 WAL） |
| `TAGFLOW_CACHE_DIR` | `./cache` | 缩略图缓存目录（worker 启动时使用） |
| `TAGFLOW_SCAN_INTERVAL` | `3600` | 定时增量扫描间隔（秒）。后台 scheduler 启动后**立即首轮**扫描所有资源库，之后每 N 秒一轮；新增/删除文件在下一轮自动同步进库。低于 60 的值会被 clamp 回 60（避免高频扫描压满 IO）；非法值（0/负数/非数字）回退到 3600。与手动 `POST /libraries/:id/scan` 共享同一把 409 并发锁，同库不会并发扫描 |

> 生产部署完整清单（含 Docker 卷规划、备份、密钥生成）见 [`doc/部署指南.md`](doc/部署指南.md)。

---

## 架构设计

TagFlow 采用 **六边形架构**，将核心业务逻辑与基础设施解耦：

```
┌─────────────────────────────────────────────────────────────┐
│                    Presentation Layer                        │
│                      Vue 3 SPA                              │
│              (Virtual Scroller + UI Components)              │
└─────────────────────────────────────────────────────────────┘
                              ↕
┌─────────────────────────────────────────────────────────────┐
│                       API Layer                              │
│                    Axum REST API                             │
│              (JSON/Query Extractor + Validation)             │
└─────────────────────────────────────────────────────────────┘
                              ↕
┌─────────────────────────────────────────────────────────────┐
│                      Core Domain                             │
│  ┌──────────┐  ┌──────────────┐  ┌─────────────────────┐   │
│  │ Scanner  │  │ TaggerEngine │  │  TagManager         │   │
│  │ (增量同步) │  │ (标签生成)    │  │  (层级标签管理)     │   │
│  └──────────┘  └──────────────┘  └─────────────────────┘   │
└─────────────────────────────────────────────────────────────┘
                              ↕
┌─────────────────────────────────────────────────────────────┐
│                   Infrastructure                             │
│  ┌──────────┐  ┌──────────────┐  ┌─────────────────────┐   │
│  │ OpenDAL  │  │   SQLite     │  │  FFmpeg             │   │
│  │(存储适配器)│  │  (持久化)     │  │  (缩略图生成)       │   │
│  └──────────┘  └──────────────┘  └─────────────────────┘   │
└─────────────────────────────────────────────────────────────┘
```

---

## 项目结构

```
tagflow/
├── tagflow-core/              # Rust 后端
│   ├── src/
│   │   ├── main.rs           # 应用入口 & API 路由
│   │   ├── lib.rs            # 库入口（供 bin 工具使用）
│   │   ├── models/           # 领域模型 (Library, Tag, FileEntry)
│   │   │   ├── db.rs         # 数据库模型
│   │   │   └── dto.rs        # API 数据传输对象
│   │   ├── infra/            # 基础设施层
│   │   │   ├── db.rs         # 数据库连接池
│   │   │   ├── storage/      # OpenDAL 存储适配器
│   │   │   └── thumbnail.rs  # 缩略图生成器（FFmpeg）
│   │   ├── engine/           # 核心引擎
│   │   │   ├── scanner/      # 增量文件扫描
│   │   │   ├── tagger/       # 标签生成流水线
│   │   │   └── worker.rs     # 后台任务调度
│   │   ├── core/             # 核心领域逻辑
│   │   │   ├── tag/          # TagManager 标签层级管理
│   │   │   └── auth.rs       # 认证模块（密码哈希、JWT）
│   │   └── api/              # REST API 层
│   │       ├── tag.rs        # 标签树 API
│   │       ├── file.rs       # 文件检索 API（含缩略图）
│   │       ├── library.rs    # 资源库管理 API
│   │       └── auth.rs       # 认证 API（登录、修改密码）
│   ├── bin/
│   │   └── reset-password.rs # 密码重置工具
│   ├── migrations/           # SQL 迁移脚本
│   └── Cargo.toml
├── tagflow-ui/                # Vue 3 前端
│   ├── src/
│   │   ├── components/       # Vue 组件
│   │   │   ├── TagItem.vue   # 标签树递归组件
│   │   │   ├── FileGrid.vue  # 虚拟滚动文件网格
│   │   │   └── Toast.vue     # Toast 消息提示组件
│   │   ├── views/            # 页面组件
│   │   │   ├── Login.vue     # 登录页面
│   │   │   ├── Home.vue      # 主页
│   │   │   └── settings/     # 设置页
│   │   │       ├── Security.vue   # 安全设置
│   │   │       └── Libraries.vue  # 存储管理
│   │   ├── stores/           # Pinia 状态管理
│   │   │   ├── useResourceStore.ts
│   │   │   └── auth.ts       # 认证状态
│   │   ├── api/              # API 客户端
│   │   │   └── http.ts       # Axios 封装
│   │   ├── router/           # Vue Router 配置
│   │   ├── App.vue           # 主应用组件
│   │   └── main.ts           # 应用入口
│   ├── index.html
│   ├── package.json
│   └── vite.config.ts
├── doc/                      # 设计文档
│   ├── API文档.md
│   ├── TagFlow 系统详细设计规格说明书.md
│   └── 开发阶段/             # Milestone 详细计划
├── CLAUDE.md                 # AI 协作开发约定
└── README.md
```

---

## 数据库设计

### 核心表结构

| 表名 | 用途 |
|------|------|
| `users` | 用户认证信息（用户名、密码哈希） |
| `libraries` | 资源库定义（本地路径 / WebDAV 配置） |
| `tags` | 层级标签树（支持自引用的父子关系） |
| `files` | 文件元数据（路径、大小、哈希、状态） |
| `file_tags` | 文件-标签多对多关系（支持来源标记） |
| `tasks` | 异步任务队列（缩略图生成等后台任务） |

### 关键索引

- `idx_files_lookup (library_id, parent_path, filename)` - 扫描时快速去重
- `idx_tags_parent (parent_id)` - 标签树递归查询优化

---

## 开发命令

### 构建与测试

```bash
cd tagflow-core

cargo build                # 构建项目
cargo run                  # 运行应用
cargo build --release      # 生产环境构建

cargo test                 # 运行测试
cargo test -- --nocapture  # 显示测试输出
cargo clippy               # 代码检查
cargo fmt                  # 代码格式化
```

### 数据库操作

```bash
# 安装 SQLx CLI (可选，用于手动迁移)
cargo install sqlx-cli --no-default-features --features sqlite

# 数据库文件位置
tagflow-core/tagflow.db

# 密码重置工具
cargo run --bin reset-password -- --new-password YOUR_NEW_PASSWORD
cargo run --bin reset-password -- --username admin --new-password YOUR_NEW_PASSWORD
```

---

## 核心算法

### 增量扫描流程

1. 递归遍历文件系统
2. 通过 `(library_id, parent_path, filename)` 查询数据库
3. 比较 `(size, mtime)` - 未变化则标记在线并跳过
4. 计算部分哈希（首尾 4KB）检测文件移动
5. 触发标签生成流水线

### 标签继承

```
文件路径: Projects/2024/Design/logo.png
              ↓
PathTagger 解析
              ↓
标签层级: Projects → 2024 → Design
              ↓
文件关联至叶子标签 "Design"
```

---

## 功能清单

按子系统组织，标注当前状态：✅ 已实现 · 🔄 进行中 · 📅 计划中

| 子系统 | 能力 | 状态 |
|--------|------|------|
| **资源库管理** | 本地资源库 CRUD、连接测试、扫描触发（同库并发返回 409 防护） | ✅ |
|  | WebDAV 资源库（OpenDAL + 凭据加密） | 📅 |
| **文件扫描** | 增量扫描、基于哈希的文件移动检测、扫描时为媒体文件入列缩略图任务 | ✅ |
|  | 定时增量扫描（后台 scheduler 自动同步，`TAGFLOW_SCAN_INTERVAL` 可配） | ✅ |
| **标签系统** | 层级标签树、路径自动建标签、`path` / `type` / `ext` / `time` 四维自动标签 | ✅ |
|  | 多标签 AND 分面过滤、递归包含子标签子树 | ✅ |
|  | 手动用户标签（user tag）打标签 / 移除 / 空节点自动清理 | ✅ |
|  | 批量打标签（多选 → 批量加 / 移除） | 📅 |
| **浏览与预览** | 虚拟滚动卡片网格、文件详情、FFmpeg 缩略图 | ✅ |
|  | 媒体预览抽屉（文本 / Markdown / PDF / 图片 / 视频 / 音频 + 下载） | ✅ |
|  | 文件名搜索、卡片 / 列表视图切换、无限滚动加载、卡片重叠修复 | ✅ |
| **认证与安全** | JWT 会话、Argon2 密码哈希、密码修改 / 重置工具 | ✅ |
|  | 路径存在性校验、密码门槛前后端统一（≥ 12 字节）、媒体 `?token=` 鉴权兜底 | ✅ |
| **部署与运维** | rust-embed 单二进制、Docker / compose、`/api/health` 健康检查 | ✅ |

---

## 开发路线图

| 里程碑 | 内容 | 状态 |
|--------|------|------|
| **Milestone 1** | 项目初始化与数据库模型建立 | ✅ |
| **Milestone 2** | 增量扫描引擎 + OpenDAL 集成 | ✅ |
| **Milestone 3** | 层级标签引擎实现 | ✅ |
| **Milestone 4** | API 层与虚拟滚动查询实现 | ✅ |
| **Milestone 5** | Vue 3 前端 + 虚拟滚动组件 | ✅ |
| **Milestone 6** | 认证模块实现（JWT + Argon2） | ✅ |
| **Milestone 6-1** | 认证 UI 与安全设置 | ✅ |
| **Milestone 7** | 存储管理模块（动态资源库） | ✅ |
| **Milestone 8** | 异步任务流水线 + 缩略图生成 | ✅ |
| **Milestone 9-1** | rust-embed 嵌入前端 + DB/cache 路径配置化 | ✅ |
| **Milestone 9-2** | Docker 化（多阶段 Dockerfile + compose + 部署文档） | ✅ |
| **v0.1.0 Beta** | 首次预发布（Pre-release）：自动标签三维度 + 多标签分面过滤、媒体预览抽屉、安全加固 | ✅ |
| **v0.2.0** | 定时增量扫描（自动同步）+ 文件视图增强 + e2e 测试框架 + 稳定性修复 | ✅ |

> **v0.2.0 正式版**：定时增量扫描（后台自动同步）、文件视图增强（无限滚动 / 文件名搜索 / 列表视图）、手动用户标签、媒体预览抽屉、e2e 测试框架（14 用例），以及 SQLite 并发写 / 缩略图 / 孤儿标签等多项稳定性修复。WebDAV 资源库与批量打标签推迟到 v0.3。详见[功能清单](#功能清单)。

### 后续迭代计划

**v0.3 —— 多源接入（WebDAV）+ 批量操作**（下一个里程碑）

- **WebDAV 资源库** —— OpenDAL `services-webdav` + AES 凭据加密，接入 NAS / 云盘；接入后复用 v0.2.0 的定时扫描自动同步
- **批量打标签** —— 多选文件批量加 / 移除标签

> 目标闭环：接入 WebDAV 库 → 自动定时同步 → 标签 / 浏览 / 预览全部可用。

**更远的迭代（v0.4+）**

- 正文全文搜索 —— SQLite FTS5 索引文件内容（文件名搜索已在视图增强中提供）
- 元数据深化 —— EXIF / 视频元信息、排序切换等进阶能力

---

## 技术栈

### 后端

- **[Rust](https://www.rust-lang.org/)** 1.92.0+ - 系统编程语言
- **[Tokio](https://tokio.rs/)** - 异步运行时
- **[SQLx](https://github.com/launchbadge/sqlx)** - 编译时类型安全的 SQL
- **[Axum](https://github.com/tokio-rs/axum)** - Web 框架
- **[OpenDAL](https://opendal.apache.org/)** - 统一存储抽象层
- **[Tracing](https://github.com/tokio-rs/tracing)** - 结构化日志
- **[Argon2](https://github.com/RustCrypto/password-hashes)** - 密码哈希
- **[jsonwebtoken](https://github.com/Keats/jsonwebtoken)** - JWT 令牌管理

### 前端

- **[Vue 3](https://vuejs.org/)** - 渐进式框架
- **[TypeScript](https://www.typescriptlang.org/)** - 类型安全
- **[Vite](https://vitejs.dev/)** - 构建工具
- **[Pinia](https://pinia.vuejs.org/)** - 状态管理
- **[TailwindCSS](https://tailwindcss.com/)** - CSS 框架
- **[vue-virtual-scroller](https://github.com/Akryum/vue-virtual-scroller)** - 虚拟滚动组件

---

## 性能目标

| 指标 | 目标值 |
|------|--------|
| 后端空闲内存 | < 30MB |
| 后端扫描内存 | < 150MB |
| 10万文件查询 | < 150ms |
| 并发连接数 | 5+ (SQLite WAL) |

---

## 贡献指南

欢迎提交 Issue 和 Pull Request！

1. Fork 本仓库
2. 创建特性分支 (`git checkout -b feature/amazing-feature`)
3. 提交更改 (`git commit -m 'Add amazing feature'`)
4. 推送到分支 (`git push origin feature/amazing-feature`)
5. 提交 Pull Request

---

## 许可证

本项目采用 [MIT](LICENSE) 许可证。

---

## 联系方式

- 项目主页: [GitHub](https://github.com/sayeui/tagflow)
- 问题反馈: [Issues](https://github.com/sayeui/tagflow/issues)

---

<div align="center">

_轻量、非侵入、层级标签的多源资源管理_

</div>
