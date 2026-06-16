# 更新 README：去 AI 介绍 + 补进展/迭代计划 + 功能清单

## Goal

刷新 TagFlow 的 README，使其反映 v0.1.0 发版后的真实状态：移除所有 AI 工具/模型相关介绍，补充当前进展与后续迭代计划，并新增一份覆盖「已实现 / 进行中 / 计划」的系统功能清单。README 是项目对外门面，需准确、可维护。

## What I already know

### 需删除的 AI 介绍（共 3 处，已定位）
- L11：顶部徽章 `AI Developed - Claude Code | Gemini 3`
- L17–36：整个「🤖 AI 辅助开发声明」章节（AI 工具表 + 开发模式）
- L361–364：底部「Made with AI Assistance - Claude Code（GLM 4.7）& Gemini」+「🤖 Primarily AI-Generated Code」

### 当前真实进展（README 现有路线图表已过时，停在 M9-2）
README 路线图表只到 M9-2，但实际已完成（截至 v0.1.0，2026-06-16）：
- 自动标签三维度（ext/type/time）+ 多标签 AND 分面过滤
- 媒体预览抽屉（文本/MD/PDF/图/视频/音频 + 下载）+ `?token=` 鉴权兜底
- 技术债清理（密码门槛统一 ≥12B、路径存在性校验、死代码）
- v0.1.0 首次发版（GitHub Release，正式版）
- 手动标签（user tag）+ 递归过滤 + 自动清理空节点

### 进行中（并行 worktree，未提交）
- `06-16-file-view-enhancements`：卡片重叠修复、无限滚动加载、文件名搜索、卡片/列表视图切换

### 后续迭代计划（前轮已与用户讨论）
1. 收尾/合并 worktree（注意 file.rs 冲突）
2. 文件操作（重命名/移动/删除）——破坏性，需先定删除安全策略
3. 批量打标签（多选→批量加/移除）
4. WebDAV 资源库（OpenDAL `services-webdav` + AES 凭据加密）
5. 正文全文搜索（FTS5）/ EXIF·视频元数据（可选）

## Requirements (evolving)

- [ ] 删除 README 中全部 AI 工具/模型介绍（3 处）
- [ ] 补充当前进展（v0.1.0 发版 + M9-2 之后完成的项）
- [ ] 更新过时的路线图里程碑表
- [ ] 补充后续迭代计划
- [ ] 新增系统功能清单（已有 + 进行中 + 计划）

## Decision (ADR-lite)

**Context**: 功能清单与迭代计划需要呈现，README 自洽 vs 独立 ROADMAP 二选一。
**Decision**: README 自洽——功能特性分组 + 路线图 + 迭代计划全部整合进单个 README，单一来源。
**Consequences**: README 变长，但项目规模可控、维护负担低，避免文档分散。

### 迭代中追加决策

- **v0.1.0 改为 Pre-release Beta**：用户已将 GitHub Release 标记为 pre-release；README 路线图记为「v0.1.0 Beta」。
- **路线图用里程碑视角**：表行是里程碑节点（M1–M9-2 + vX.Y.Beta），不把每个功能/任务各列一行；功能完成项在功能清单体现。
- **文件操作（改名/移动/删除）暂不考虑**：破坏性操作暂缓，从后续迭代计划与功能清单移除。
- **下一个里程碑 v0.2.0 Beta = 多源接入与自动同步**：范围 WebDAV 资源库 + 定时增量扫描；目标闭环「接入 WebDAV → 自动同步 → 标签/浏览/预览可用」。批量标签 / 全文搜索 / 元数据留 v0.3.0+。
- **产品蓝图 gemini 链接暂时保留**。

## Technical Approach

- 删除 3 处 AI 介绍（L11 徽章 / L17–36 声明章节 / L361–364 底部标语）
- 更新路线图里程碑表：补 v0.1.0 发版 + M9-2 之后完成项（自动标签三维度、媒体预览、技术债清理、手动标签）
- 新增「功能特性」章节：按子系统分组，每项标 ✅已有 / 🔄进行中 / 📅计划
- 新增「后续迭代计划」小节：优先级列表
- 保留非 AI 的徽章（Rust/Vue/License）、贡献指南、联系方式、架构图、数据库设计等既有章节

## Acceptance Criteria (evolving)

- [ ] README 不再出现任何 AI 工具名/模型名/AI 开发声明
- [ ] 路线图表反映 v0.1.0 真实状态（含发版）
- [ ] 功能清单覆盖核心子系统，并区分已实现/进行中/计划
- [ ] 迭代计划可读、有优先级
- [ ] 内部链接（部署指南、doc/ 等）仍有效

## Definition of Done

- README 内容与代码/任务实际状态一致（交叉核对 API 路由、已完成任务）
- Markdown 渲染无破损（表格/链接/徽章）
- 一个清晰的 commit（纯文档，无代码改动）

## Out of Scope (explicit)

- 不改任何源代码（等 worktree 完成，不动其实现）
- 不改 doc/ 下其他文档、不改 CLAUDE.md
- 不引入新功能/新依赖

## Technical Notes

- 关键文件：`README.md`
- 参考素材：`memory/tagflow-roadmap-priorities.md`、worktree 任务 `06-16-file-view-enhancements/prd.md`、`doc/TagFlow 系统详细设计规格说明书.md` §3.x
- 当前 API 路由（核对用）：`tags/tree`、`files`(list/detail/thumbnail/content/tags CRUD)、`libraries`(CRUD/test/scan)、`auth`
