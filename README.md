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

---

## 快速开始

### 环境要求

- **Rust**: 1.92.0 或更高版本
- **Node.js**: 18+ (前端开发)
- **SQLite**: 3.35+ (自动通过 SQLx 集成)

### 安装运行

```bash
# 克隆项目
git clone https://github.com/yourusername/tagflow.git
cd tagflow

# 启动后端
cd tagflow-core
cargo build
cargo run

# 数据库将自动初始化为 tagflow.db (启用 WAL 模式)
```

### Docker 部署（计划中）

```bash
docker build -t tagflow .
docker run -p 8080:8080 -v ./data:/data tagflow
```

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
│   │   ├── main.rs           # 应用入口
│   │   ├── models/           # 领域模型 (Library, Tag, FileEntry)
│   │   ├── infra/            # 基础设施 (数据库连接池)
│   │   ├── engine/           # 核心引擎 (计划)
│   │   │   ├── scanner/      # 增量文件扫描
│   │   │   └── tagger/       # 标签生成流水线
│   │   ├── api/              # Axum REST API (计划)
│   │   └── core/             # 标签层级管理 (计划)
│   ├── migrations/           # SQL 迁移脚本
│   └── Cargo.toml
├── doc/                      # 设计文档
│   └── TagFlow 系统详细设计规格说明书.md
├── CLAUDE.md                 # Claude Code 工作指南
└── README.md
```

---

## 数据库设计

### 核心表结构

| 表名 | 用途 |
|------|------|
| `libraries` | 资源库定义（本地路径 / WebDAV 配置） |
| `tags` | 层级标签树（支持自引用的父子关系） |
| `files` | 文件元数据（路径、大小、哈希、状态） |
| `file_tags` | 文件-标签多对多关系（支持来源标记） |

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

## 开发路线图

| 里程碑 | 内容 | 状态 |
|--------|------|------|
| **Milestone 1** | 数据库模型建立 | ✅ 完成 |
| **Milestone 2** | 增量扫描引擎 + OpenDAL 集成 | 🚧 进行中 |
| **Milestone 3** | 层级标签引擎（TagManager） | ⏳ 待开始 |
| **Milestone 4** | Axum API + 虚拟滚动查询 | ⏳ 待开始 |
| **Milestone 5** | Vue 3 前端 + 虚拟滚动组件 | ⏳ 待开始 |
| **Milestone 6** | 异步任务流水线 + 缩略图生成 | ⏳ 待开始 |
| **Milestone 7** | Docker 部署（Alpine 多阶段构建） | ⏳ 待开始 |

---

## 技术栈

### 后端

- **[Rust](https://www.rust-lang.org/)** 1.92.0+ - 系统编程语言
- **[Tokio](https://tokio.rs/)** - 异步运行时
- **[SQLx](https://github.com/launchbadge/sqlx)** - 编译时类型安全的 SQL
- **[Axum](https://github.com/tokio-rs/axum)** - Web 框架（计划）
- **[OpenDAL](https://opendal.apache.org/)** - 统一存储抽象层（计划）
- **[Tracing](https://github.com/tokio-rs/tracing)** - 结构化日志

### 前端（计划）

- **[Vue 3](https://vuejs.org/)** - 渐进式框架
- **[TypeScript](https://www.typescriptlang.org/)** - 类型安全
- **[Vite](https://vitejs.dev/)** - 构建工具

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

**Made with ❤️ and Rust**

</div>
