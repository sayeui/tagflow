# CLAUDE.md

本文件为 Claude Code (claude.ai/code) 在此代码仓库中工作时提供指导。

## 项目概述

**TagFlow** 是一个轻量级、非侵入式、支持层级标签的多源资源管理系统。项目采用六边形架构（Hexagonal Architecture），使用 Rust（后端）和 Vue 3（计划中的前端）实现。

**当前状态：** 早期开发阶段（已完成 Milestone 1 - 数据库模型建立）

## 开发命令

### 构建与运行
```bash
cd tagflow-core
cargo build                    # 构建项目
cargo run                      # 运行应用（会初始化 SQLite 数据库）
cargo build --release          # 生产环境构建
```

### 测试与质量检查
```bash
cd tagflow-core
cargo test                     # 运行所有测试
cargo test -- --nocapture      # 运行测试并显示输出
cargo clippy                   # 代码检查
cargo fmt                      # 代码格式化
```

### 数据库操作
- 数据库文件：`tagflow-core/tagflow.db`（SQLite，启用 WAL 模式）
- 迁移脚本位置：`tagflow-core/migrations/`
- 应用启动时会通过 `sqlx::migrate!()` 自动执行迁移

### 依赖要求
- Rust 工具链：1.92.0+
- SQLx CLI（用于手动迁移）：`cargo install sqlx-cli --no-default-features --features sqlite`

## 架构设计

### 分层结构（六边形架构）

```
表现层      →  API 层    →  核心领域    →  基础设施层
(Vue 3)        (Axum)       (业务逻辑)      (存储/数据库)
```

### 模块组织

**`tagflow-core/src/`**
- `main.rs`：应用入口，初始化日志和数据库
- `models/`：领域模型（Library、Tag、FileEntry），映射到数据库架构
- `infra/`：基础设施适配器（数据库连接池，未来将包含 OpenDAL 存储）
- **计划中的模块：**
  - `engine/scanner/`：增量文件扫描与变更检测
  - `engine/tagger/`：层级标签生成（PathTagger、ExtensionTagger）
  - `api/`：Axum REST API 路由
  - `core/tag/`：标签层级管理与缓存

### 数据库架构

实现层级资源管理的四个核心表：
1. **`libraries`**：资源库定义（本地路径或 WebDAV 路径）
2. **`tags`**：层级标签树（通过 `parent_id` 自引用）
3. **`files`**：文件元数据，支持增量同步（基于哈希的移动检测）
4. **`file_tags`**：多对多关系表，记录标签来源（自动/手动）

**关键索引：**
- `idx_files_lookup (library_id, parent_path, filename)`：扫描时快速去重检测
- `idx_tags_parent (parent_id)`：高效标签树遍历

## 核心设计模式

### 标签层级系统
标签形成树形结构：
- 路径组件（`Projects/2024/Design`）自动创建嵌套标签：`Projects → 2024 → Design`
- 文件关联到叶子标签，但查询可递归包含子标签
- 标签类别：`path`（路径）、`type`（类型）、`user`（用户）、`time`（时间）

### 增量扫描算法
1. 递归遍历文件系统
2. 对每个文件，通过 `(library_id, parent_path, filename)` 查询数据库
3. 比较 `(size, mtime)` - 若未变化则标记为在线并跳过
4. 若有变化或为新文件，计算部分哈希（首尾 4KB）以检测文件移动
5. 对新增/修改的文件触发标签生成流水线

### 标签生成 Trait
```rust
pub trait Tagger {
    fn execute(&self, entry: &FileEntry) -> Vec<PendingTag>;
}
```
实现将包括：
- **PathTagger**：从目录层级提取嵌套标签
- **ExtensionTagger**：从文件扩展名创建类型标签
- **TimeTagger**：从修改时间戳生成年份/月份标签

## 开发指南

### 代码风格
- 使用 Rust 2024 版本特性
- 所有 I/O 操作使用 Tokio 运行时的 async/await
- 错误处理：应用错误使用 `anyhow::Result`，库错误使用 `thiserror`
- 使用 `tracing` crate 进行结构化日志（禁止使用 `println!`）

### 数据库约定
- 所有日期时间字段使用 `chrono::DateTime<Utc>`
- 通过 `PRAGMA foreign_keys = ON` 强制外键约束
- 不使用软删除 - 依赖 `ON DELETE CASCADE`
- 文件路径存储为库内相对路径（便于库迁移）

### 性能目标
- 后端内存占用：空闲时 <30MB，密集扫描时 <150MB
- API 响应时间：10 万+文件查询 <150ms
- 使用 SQLite 的 `WITH RECURSIVE` 进行高效标签树查询

## 后续里程碑

**Milestone 2（下一步）：** 增量扫描引擎 + OpenDAL 集成
**Milestone 3：** 层级标签引擎（TagManager 及 `get_or_create` 方法）
**Milestone 4：** Axum API + 虚拟滚动查询
**Milestone 5：** Vue 3 前端 + 虚拟滚动组件
**Milestone 6：** 异步任务流水线 + 缩略图生成
**Milestone 7：** Docker 部署（Alpine + 多阶段构建）

## 参考文档

详细规格说明位于 `doc/` 目录：
- `TagFlow 系统详细设计规格说明书.md`：完整系统设计
- `开发阶段/Milestone X.md`：各阶段具体实现计划
