# 更新项目说明文件以反映实际进展

## Goal

CLAUDE.md 与 README.md 最后更新于 2025-12-30（早于 Milestone 8 提交），文档声明的项目状态落后于实际代码。将说明文件同步到当前真实进展。

## What I already know（与文档的差异清单）

实际进展（来自 git log + 源码结构）：

* **Milestone 8 已完成**（commit 475414a）：异步任务流水线（`engine/worker.rs`、`tasks` 表）+ 缩略图生成（`infra/thumbnail.rs`）+ 缩略图 API
* **扫描流水线已打通**（0c17031）：`trigger_scan` 接入扫描引擎与缩略图任务
* **list_files 修复**（346589f）：返回真实 total 计数
* **Trellis 工作流已初始化**（7d43745）

文档落后点：

* CLAUDE.md：
  * "当前状态" 写到 M7，路线图 M8 标 ⏳ → 应为 ✅
  * 模块结构缺 `engine/`（scanner/tagger/worker）、`infra/thumbnail.rs`、`infra/storage/`
  * API 路由缺 `GET /api/v1/files/:id/thumbnail`
* README.md：
  * 路线图 M7、M8 均标 ⏳ 待开始 → 实际均已完成
  * 核心特性未提缩略图/异步任务

## Requirements

* 更新 CLAUDE.md：当前状态、模块组织、API 路由、路线图表格
* 更新 README.md：路线图表格、核心特性（如适用）
* 仅做事实同步，不重写文档结构

## Acceptance Criteria

* [x] CLAUDE.md 路线图 M7/M8 状态与 git 历史一致
* [x] CLAUDE.md 模块组织包含 engine/、infra/thumbnail.rs、infra/storage/
* [x] CLAUDE.md API 路由列表与 main.rs 实际路由一致（已逐条核对）
* [x] README.md 路线图 M7/M8 标 ✅
* [x] 不引入与代码不符的描述（FFmpeg 外部命令、./cache、tasks 表均已从源码核实）

## Out of Scope

* doc/ 下的设计规格文档与 Milestone 详细计划（除非用户要求）
* 文档结构重构、新增章节

## Decision (ADR-lite)

**Context**: 更新范围需确认
**Decision**: 用户确认仅更新 CLAUDE.md + README.md，不动 doc/ 设计文档
**Consequences**: doc/ 下里程碑文档状态可能仍滞后，留待后续任务

## Technical Notes

* 路由来源：tagflow-core/src/main.rs:59-93
* M8 提交内容：475414a（worker 195 行、thumbnail 159 行、tasks 表迁移、缩略图 API、FileGrid 缩略图显示）
