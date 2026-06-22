# 孤儿标签清理（删库 + 扫描删文件离线标签）

## Goal

修复删库 / 删文件后无效标签残留（标签树显示但勾选查询为空），避免无效 tag 越来越多。两场景：删库（真孤儿 tags 无 file_tags）+ 扫描删文件（软删 status=0，标签关联离线文件）。v0.2.0 发版阻塞。

## What I already know（诊断，2026-06-22）

### 标签树与查询的现状（根因）
- `get_tag_tree`（`api/tag.rs`）：`SELECT * FROM tags` → **不过滤 file_tags 或 status**，返回所有标签节点（孤儿、离线关联的全显示）。
- 文件查询（`api/file.rs`）：全部过滤 `status = 1`（在线）——line 89/107/143/180/220/463/626/678。
- **这就是"标签显示但查询空"的根因**：标签树显示无在线文件关联的标签，查询过滤 status=1 返回空。

### 删库场景（真孤儿）
- `delete_library`（`api/library.rs`）：只 `DELETE libraries`，CASCADE 链删 files/file_tags/tasks，但 `tags` 表无 library_id FK、不在 CASCADE 链 → tags 节点（path/ext/type/time）残留为真孤儿（无 file_tags）。
- `cleanup_orphan_user_tag`（`file.rs:712`）：现有递归清理（COUNT(file_tags)=0 且 COUNT(子节点)=0 则删、向上递归），但**写死仅 user**、删库时不触发。

### 扫描删文件场景（软删，非孤儿）
- `scanner/mod.rs:310` `mark_as_lost`：物理消失的文件只置 `files.status=0`（软删），不真删 → file_tags 保留、tags 非孤儿但关联离线文件。
- 软删是**有意设计**（`restore_file` 恢复 + 基于哈希的移动检测），不能轻改。

### 标签树结构
- `tags` 表层级（parent_id 自引用 CASCADE），`get_tag_tree` 内存 `build_tree`。父标签即使无直接文件关联，子标签有时仍需显示父（剪枝要按子树判定）。

## Assumptions (temporary)

- 标签树过滤 status=1（方案 A）同时解决两场景的"显示"问题；删库另做真清理（泛化 cleanup_orphan_tag）解决 tags 表堆积。
- 软删语义保留（恢复/移动检测不动）。

## Open Questions

（全部已定，见已定决策）

## 已定决策

- ✅ **Q1（组合方案）→ 标签树过滤 + 删库清理**：`get_tag_tree` 过滤 status=1（孤儿 + 离线关联都不显示，同时解决两场景）+ 删库后泛化 `cleanup_orphan_tag` 真清理孤儿 tags（解决表堆积）。软删语义保留（恢复/移动检测不动）。

## Technical Approach

### 1. 标签树过滤在线文件（`api/tag.rs::get_tag_tree`）
- 查「有 status=1 文件关联的 tag_id 集合」：`SELECT DISTINCT tag_id FROM file_tags ft JOIN files f ON ft.file_id = f.id WHERE f.status = 1`。
- `build_tree` 后**递归剪枝**：节点若自身不在集合且所有子节点被剪，则剪（按子树判定，保证父标签在子标签有在线关联时仍显示）。
- 纯显示层过滤，不改 tags 表数据。

### 2. 删库孤儿清理（`api/library.rs::delete_library` + 泛化清理）
- 泛化 `cleanup_orphan_user_tag`（`file.rs:712`）为 `cleanup_orphan_tag`（去 user 限制，适用 path/ext/type/time/user），递归逻辑不变（COUNT(file_tags)=0 且 COUNT(子)=0 则删、向上）。
- `delete_library` 改造：**删库前**查受影响 tag_ids（`SELECT DISTINCT tag_id FROM file_tags WHERE file_id IN (SELECT id FROM files WHERE library_id=?)`），DELETE library（CASCADE 删 files/file_tags），**删库后**对每个 tag_id 调 `cleanup_orphan_tag`。
- 跨库共享标签天然安全（COUNT=0 才删，他库有 status=1 关联则保留）。

### 3. 软删语义保留
- `scanner mark_as_lost` 不动。标签树过滤使离线文件标签不显示，文件恢复（status→1）时标签自动回归。

## Decision (ADR-lite)

- **Context**：删库留孤儿 tags、扫描删文件留离线关联 tags，标签树 `SELECT * FROM tags` 全显示 → 无效标签堆积。
- **Decision**：标签树显示层过滤 status=1（解决两场景显示）+ 删库后真清理孤儿 tags（解决表堆积）；软删语义完整保留。
- **Consequences**：前端不再见无效标签；删库后 tags 表干净；扫描删文件标签隐藏但随文件恢复自动回归；tags 表可能有少量离线关联节点（不显示，可接受，未来可选加定期真删）。

## Implementation Plan (small PRs)

- **PR1：删库孤儿清理**——泛化 `cleanup_orphan_tag` + `delete_library` 改造 + 单测（删库后无孤儿、跨库共享保留、向上递归剪枝）。
- **PR2：标签树过滤**——`get_tag_tree` 内存剪枝 + 单测（孤儿/离线不显示、父标签保留、跨库共享显示）。
- **PR3：e2e + spec**——删库/扫描删文件标签树用例 + spec 契约（标签树只显示在线文件关联 + 删库孤儿清理）。

## Requirements (evolving)

- 标签树不显示无在线（status=1）文件关联的标签（孤儿 + 离线关联）。
- 删库后清理孤儿 tags（泛化 cleanup_orphan_tag，所有类别，递归剪枝）。

## Acceptance Criteria (evolving)

- [ ] 删库后标签树不再显示该库的 path/ext/type/time 标签。
- [ ] 扫描删文件后，离线文件的标签不再显示（软删文件可恢复时标签回归）。
- [ ] 跨库共享标签（#year:2026、Projects/ 被多库用）保留（仍有 status=1 关联）。
- [ ] 删库后 tags 表不堆积孤儿（真清理）。
- [ ] 软删语义保留（文件恢复时 status 回 1、标签重现）。
- [ ] e2e 覆盖：删库后标签树无残留 + 扫描删文件标签隐藏 + 跨库共享保留。

## Definition of Done

- 标签树过滤 + 删库清理实现 + 测试通过。
- `cargo test` / `clippy` / e2e 全绿。
- spec：记「标签树只显示在线文件关联的标签 + 删库孤儿清理」契约。

## Out of Scope (explicit)

- 软删改硬删（破坏恢复/移动检测）。
- 长期 status=0 定期真删（视 Q1，可能纳入或后续）。
- 标签计数 UI（当前 TagNode 不含 count）。

## Research References

- 无外部 research：方案由 repo 现状（get_tag_tree 不过滤 + cleanup_orphan_user_tag 可复用）决定。

## Technical Notes

- 关键文件：`api/tag.rs`（get_tag_tree + build_tree）、`api/library.rs`（delete_library）、`api/file.rs`（cleanup_orphan_user_tag line 712）、`engine/scanner/mod.rs`（mark_as_lost line 310）、`migrations/202512260001_init.sql`（tags/file_tags schema）。
- 标签树过滤实现：查「有 status=1 文件关联的 tag_id 集合」→ build_tree 后递归剪无在线关联的子树（或 SQL WITH RECURSIVE + EXISTS）。
- 删库清理：泛化 cleanup_orphan_user_tag 为 cleanup_orphan_tag（去 user 限制）；delete_library 删库前查受影响 tag_ids（`SELECT DISTINCT tag_id FROM file_tags WHERE file_id IN (SELECT id FROM files WHERE library_id=?)`），删库后逐个清理。
- 跨库共享标签天然安全（COUNT/EXISTS 判定，有他库 status=1 关联则保留）。
