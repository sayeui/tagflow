# CLAUDE.md

本文件为 Claude Code (claude.ai/code) 在此代码仓库中工作时提供指导。

## 项目概述

**TagFlow** 是一个轻量级、非侵入式、支持层级标签的多源资源管理系统。项目采用六边形架构（Hexagonal Architecture），使用 Rust（后端）和 Vue 3（前端）实现。

**当前状态：** 已完成 Milestone 1-7（数据库模型、扫描引擎、标签系统、API 层、前端基础架构、认证模块、存储管理）

## 开发命令

### 构建与运行
```bash
# 后端
cd tagflow-core
cargo build                    # 构建项目
cargo run                      # 运行应用（默认 INFO 日志）
RUST_LOG=debug cargo run       # 运行应用（DEBUG 日志，用于调试）
cargo build --release          # 生产环境构建

# 密码重置工具
cargo run --bin reset-password -- --help
cargo run --bin reset-password -- --new-password YOUR_PASSWORD
cargo run --bin reset-password -- --username admin --new-password YOUR_PASSWORD
```

### 测试与质量检查
```bash
cd tagflow-core
cargo test                     # 运行所有测试
cargo test -- --nocapture      # 运行测试并显示输出
cargo clippy                   # 代码检查
cargo fmt                      # 代码格式化
```

### 前端开发
```bash
cd tagflow-ui
npm install                    # 安装依赖
npm run dev                    # 启动开发服务器 (http://localhost:5173)
npm run build                  # 构建生产版本
```

### 数据库操作
- 数据库文件：`tagflow-core/tagflow.db`（SQLite，启用 WAL 模式）
- 迁移脚本位置：`tagflow-core/migrations/`
- 应用启动时会通过 `sqlx::migrate!()` 自动执行迁移
- 数据库文件不纳入 Git 管理

### 依赖要求
- Rust 工具链：1.92.0+
- Node.js：18+ (前端开发)
- SQLx CLI（用于手动迁移）：`cargo install sqlx-cli --no-default-features --features sqlite`

### 日志调试

日志级别通过环境变量控制：
```bash
# 默认 INFO 级别
cargo run

# DEBUG 级别（查看详细日志）
RUST_LOG=debug cargo run

# 只显示某个模块的 DEBUG 日志
RUST_LOG=tagflow_core::api::library=debug cargo run

# TRACE 级别（最详细）
RUST_LOG=trace cargo run
```

日志输出格式：
- ➡️ 请求开始
- ✅ 成功 (2xx)
- ⚠️ 客户端错误 (4xx)
- ❌ 服务器错误 (5xx)

## 架构设计

### 分层结构（六边形架构）

```
表现层 (Vue 3) → API 层 (Axum) → 核心领域 → 基础设施层
                                    ↓
                            (存储/数据库)
```

### 模块组织

**`tagflow-core/src/`**
- `main.rs`：应用入口，初始化日志和数据库
- `lib.rs`：库入口，供 bin 工具使用
- `models/`：领域模型
  - `db.rs`：数据库模型
  - `dto.rs`：API 数据传输对象
- `infra/`：基础设施适配器（数据库连接池）
- `core/`：核心领域逻辑
  - `auth.rs`：认证模块（密码哈希、JWT）
- `api/`：REST API 层
  - `auth.rs`：认证 API（登录、修改密码）
  - `tag.rs`：标签树 API
  - `file.rs`：文件检索 API
  - `library.rs`：资源库管理 API
- `bin/`：独立工具
  - `reset-password.rs`：密码重置工具

**`tagflow-ui/src/`**
- `views/`：页面组件
  - `Login.vue`：登录页面
  - `Home.vue`：主页（文件浏览）
  - `settings/Security.vue`：安全设置
  - `settings/Libraries.vue`：存储管理
- `components/`：Vue 组件
  - `TagItem.vue`：标签树递归组件
  - `FileGrid.vue`：虚拟滚动文件网格
  - `Toast.vue`：消息提示组件
- `stores/`：Pinia 状态管理
  - `auth.ts`：认证状态
  - `useResourceStore.ts`：资源状态
- `api/`：API 客户端
  - `http.ts`：Axios 封装
- `router/`：Vue Router 配置

### 数据库架构

实现层级资源管理的核心表：
1. **`users`**：用户认证信息（用户名、密码哈希）
2. **`libraries`**：资源库定义（本地路径或 WebDAV 路径）
3. **`tags`**：层级标签树（通过 `parent_id` 自引用）
4. **`files`**：文件元数据，支持增量同步（基于哈希的移动检测）
5. **`file_tags`**：多对多关系表，记录标签来源（自动/手动）

**关键索引：**
- `idx_files_lookup (library_id, parent_path, filename)`：扫描时快速去重检测
- `idx_tags_parent (parent_id)`：高效标签树遍历

### API 路由

**公开路由（无需认证）：**
- `POST /api/auth/login` - 用户登录

**受保护路由（需要认证）：**
- `GET /api/v1/tags/tree` - 获取标签树
- `GET /api/v1/files` - 获取文件列表
- `POST /api/auth/update-password` - 修改密码
- `GET /api/v1/libraries` - 获取资源库列表
- `POST /api/v1/libraries` - 创建资源库
- `POST /api/v1/libraries/test` - 测试连接
- `DELETE /api/v1/libraries/:id` - 删除资源库
- `POST /api/v1/libraries/:id/scan` - 触发扫描

## 核心设计模式

### 认证系统
- **密码哈希**：使用 Argon2 算法（最安全的密码哈希之一）
- **JWT 令牌**：24 小时有效期，存储在 localStorage
- **中间件**：自动验证 Bearer Token
- **默认管理员**：
  - 用户名：`admin`
  - 密码：`PhVENfYaWv`
  - 首次启动时自动创建

### 标签层级系统
标签形成树形结构：
- 路径组件（`Projects/2024/Design`）自动创建嵌套标签：`Projects → 2024 → Design`
- 文件关联到叶子标签，但查询可递归包含子标签
- 标签类别：`path`（路径）、`type`（类型）、`user`（用户）、`time`（时间）

### 存储管理
- 支持本地目录协议
- WebDAV 协议（计划中）
- 连接测试功能
- 动态添加/删除资源库

## 开发指南

### 代码风格
- 使用 Rust 2024 版本特性
- 所有 I/O 操作使用 Tokio 运行时的 async/await
- 错误处理：应用错误使用 `anyhow::Result`，库错误使用 `thiserror`
- 使用 `tracing` crate 进行结构化日志（禁止使用 `println!`）

### 日志规范
- `debug!()`：详细的调试信息（函数参数、中间状态）
- `info!()`：重要的业务操作（创建资源库、用户登录）
- `warn!()`：可预期的错误情况（无效输入、连接失败）
- `error!()`：需要关注的错误（数据库操作失败）

### 数据库约定
- 所有日期时间字段使用 `chrono::DateTime<Utc>`
- 通过 `PRAGMA foreign_keys = ON` 强制外键约束
- 不使用软删除 - 依赖 `ON DELETE CASCADE`
- 文件路径存储为库内相对路径（便于库迁移）

### 性能目标
- 后端内存占用：空闲时 <30MB，密集扫描时 <150MB
- API 响应时间：10 万+文件查询 <150ms
- 使用 SQLite 的 `WITH RECURSIVE` 进行高效标签树查询

## 项目结构

```
tagflow/
├── tagflow-core/              # Rust 后端
│   ├── src/
│   │   ├── main.rs           # 应用入口 & API 路由
│   │   ├── lib.rs            # 库入口（供 bin 工具使用）
│   │   ├── models/           # 领域模型
│   │   ├── infra/            # 基础设施层
│   │   ├── core/             # 核心领域逻辑
│   │   ├── api/              # REST API 层
│   │   └── bin/              # 独立工具
│   │       └── reset-password.rs
│   ├── migrations/           # SQL 迁移脚本
│   └── Cargo.toml
├── tagflow-ui/                # Vue 3 前端
│   ├── src/
│   │   ├── views/            # 页面组件
│   │   ├── components/       # Vue 组件
│   │   ├── stores/           # Pinia 状态管理
│   │   ├── api/              # API 客户端
│   │   └── router/           # Vue Router 配置
│   ├── package.json
│   └── vite.config.ts
├── doc/                      # 设计文档
│   ├── API文档.md
│   ├── TagFlow 系统详细设计规格说明书.md
│   └── 开发阶段/             # Milestone 详细计划
├── CLAUDE.md                 # 本文件
└── README.md
```

## 开发路线图

| 里程碑 | 内容 | 状态 |
|--------|------|------|
| **Milestone 1** | 项目初始化与数据库模型建立 | ✅ 完成 |
| **Milestone 2** | 增量扫描引擎 + OpenDAL 集成 | ✅ 完成 |
| **Milestone 3** | 层级标签引擎实现 | ✅ 完成 |
| **Milestone 4** | API 层与虚拟滚动查询实现 | ✅ 完成 |
| **Milestone 5** | Vue 3 前端 + 虚拟滚动组件 | ✅ 完成 |
| **Milestone 6** | 认证模块实现（JWT + Argon2） | ✅ 完成 |
| **Milestone 6-1** | 认证 UI 与安全设置 | ✅ 完成 |
| **Milestone 7** | 存储管理模块实现 | ✅ 完成 |
| **Milestone 8** | 异步任务流水线 + 缩略图生成 | ⏳ 待开始 |
| **Milestone 9** | 部署、容器化与产品化实现 | ⏳ 待开始 |

## 参考文档

详细规格说明位于 `doc/` 目录：
- `TagFlow 系统详细设计规格说明书.md`：完整系统设计
- `开发阶段/Milestone X.md`：各阶段具体实现计划
- `API文档.md`：API 接口文档
