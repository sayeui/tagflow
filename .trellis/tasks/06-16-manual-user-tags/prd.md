# 手动标签 UI（user tag）

## Goal

让用户能给文件**手动打自定义标签**（`category='user'`），并像自动标签一样在侧栏勾选、做 AND 过滤。
把 TagFlow 从「纯自动索引器」推进到「可人工组织的资源库」——这是资源管理系统的核心诉求。

## Requirements（已确认）

1. 用户可在文件抽屉（`FileDrawer`）里为当前文件添加自定义标签。
2. **层级模型**：输入用「/」分隔的路径（如 `项目/TagFlow`），后端按段逐层创建/复用 `category='user'` 的标签节点，叶子挂到文件（`source='manual'`）。
3. 添加后侧栏「自定义」分区出现该标签（含层级），可勾选做 AND 递归过滤（现有 `query_files_by_tags_recursive` 天然生效）。
4. 抽屉里 user/manual 标签可点「×」移除当前文件关联；自动标签（`source='auto'`）受保护、不显示「×」、后端拒绝删除。
5. **移除即自动清理**：移除某 manual 关联后，若该 user 标签节点（及其因清理而变空的祖先节点）再无任何 `file_tags` 引用且无子节点，则从 `tags` 表删除，侧栏同步消失。不留空标签、无需独立管理 UI。

## Acceptance Criteria

- [ ] 后端 `POST /api/v1/files/:id/tags`（body `{path}`）：按「/」拆分逐层建/复用 user 节点 → 叶子挂 `source='manual'`；返回更新后的文件标签列表。
- [ ] 后端 `DELETE /api/v1/files/:id/tags/:tag_id`：仅删 `source='manual'` 关联（auto 返回 403/409）；删除后执行自动清理（叶子及空祖先 user 节点）。
- [ ] 校验：path 非空、按「/」拆分后过滤空段、单段名不含「/」、长度上限、trim；`UNIQUE(name,parent_id)` 冲突时复用既有节点而非报错。
- [ ] 前端抽屉：标签区有「+ 添加标签」输入框（回车提交）；user/manual 标签 chip 有「×」，auto 标签无「×」。
- [ ] 添加/移除后：抽屉 `fileDetail.tags` 局部刷新 + `fetchTags()` 刷新侧栏树（自动清理可见）。
- [ ] 侧栏勾选 user 标签能正确过滤出（含递归）挂了该标签的文件——过滤链路 e2e 验证。
- [ ] 自动标签不会被误删/误改。

## Decision (ADR-lite)

- **Context**：user 标签既要能人工组织、又要复用系统已有的层级标签 + 递归过滤能力；同时避免积累无用的「空标签」。
- **Decision**：
  1. 采用**层级模型**，输入「/」分隔路径自动建嵌套 user 节点（与 path/time 自动标签统一，schema/filter/侧栏全现成）。
  2. 删除采用**移除关联 + 无引用自动清理**（叶子向上递归清理空祖先），不设独立标签管理页。
  3. 创建交互为**输入即创建+挂载**（在抽屉内完成，不要求先去别的页面建档）。
- **Consequences**：交互最简、心智最轻；代价是后端需实现「/」逐层建节点 + 递归自动清理逻辑（核心复杂度集中在这两处）。未来若需标签重命名/合并/拖拽改层级，另开任务（见 Out of Scope）。

## Definition of Done (team quality bar)

- 后端新增写 API 有单测（内存库 + sqlx，参照 `api/file.rs` 测试范式：建表/插标签/插文件/link 辅助函数）。
- `cargo fmt && cargo clippy && cargo test` 全绿；前端 `npm run build` 通过、tsc 无错。
- spec 若有新约定（如「/」分隔建层级的校验规则、自动清理策略）入 `.trellis/spec/backend/`。
- **真实 e2e**：跑起前后端，对真实文件打「a/b」嵌套标签 → 侧栏出现层级 → 勾选过滤 → 移除 → 验证节点自动清理，全链路走通。

## Out of Scope (explicit)

- 批量给多文件打标签（多选文件 → 批量加 tag）。
- user 标签的**重命名 / 合并 / 拖拽改层级**（MVP 仅支持新建嵌套与删除；改父子关系另开任务）。
- WebDAV、文件重命名/移动/删除等其它路线图项。
- 扁平模式（已选层级，不再保留）。

## Technical Notes

### 后端
- 新增写 API 放 `api/tag.rs`（目前仅 `get_tag_tree`）或新建 `api/file_tag.rs`；遵循现有 `axum + sqlx + StatusCode` 风格（参考 `api/library.rs` 的写操作 + 校验 + 中文 error 日志范式）。
- **建层级**：`path.split('/')` → trim → 过滤空段；从 `category='user'` 根（`parent_id IS NULL`）开始，逐段 `SELECT id WHERE name=? AND parent_id=? AND category='user'`，不存在则 `INSERT`，得到叶子 id；`INSERT OR IGNORE INTO file_tags (file_id, tag_id, 'manual')`。
- **自动清理**：移除 `file_tags` 行后，对叶子 user 节点向上递归：`COUNT(*) FROM file_tags WHERE tag_id=? = 0` 且 `COUNT(*) FROM tags WHERE parent_id=? = 0` → `DELETE FROM tags WHERE id=?`（CASCADE 会带走其 file_tags，无子节点故无子树风险）；取 `parent_id` 继续向上，直到节点仍有引用或非 user 类别。
- 返回更新后的 `FileTagInfo` 列表（复用 `get_file_detail` 的标签查询）。

### 前端
- `http.ts`：新增 `fileApi.addTag(id, path)` / `fileApi.removeTag(id, tagId)`（资源归属 file，放 fileApi 合理）。
- `useResourceStore`：新增 `addTagToFile(path)` / `removeTagFromFile(tagId)` action；操作后局部更新 `fileDetail.tags` + `fetchTags()` 刷新侧栏（自动清理依赖树重建）。
- `FileDrawer.vue`：标签区追加「+ 添加标签」输入框（回车提交、空/非法输入禁用、失败 Toast）；user/manual chip 加 `×` 按钮（`source` 需从后端返回——见下）。
- ⚠️ **数据缺口**：当前 `FileDetail.tags` 的 `FileTagInfo` 只有 `{id,name,category}`，**缺 `source` 字段**，前端无法区分 auto/manual 以决定是否显示「×」。需在 `dto.rs` 的 `FileTagInfo` 加 `source` 字段 + `get_file_detail` 的查询带上 `source`。

## Implementation Plan

- **阶段 1 · 后端**：`FileTagInfo` 加 `source` → `POST /files/:id/tags` + `DELETE /files/:id/tags/:tag_id`（含校验 + 自动清理）→ 单测（建嵌套/复用/移除/auto 拒绝/自动清理父子链/保留仍有引用的节点）。
- **阶段 2 · 前端**：`http.ts` 写方法 → store actions → `FileDrawer` 添加输入框 + chip「×」（按 source 区分）。
- **阶段 3 · e2e**：跑前后端，嵌套标签添加→侧栏层级→勾选过滤→移除→自动清理，全链路验证；spec 收口。
