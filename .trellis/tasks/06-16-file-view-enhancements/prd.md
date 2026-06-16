# 文件视图样式与功能优化

## Goal

提升主页面文件浏览体验：修复虚拟滚动卡片重叠 bug、实现无限滚动加载、增加文件名搜索、支持卡片/列表两种视图切换。让用户能完整浏览、快速定位、按需切换展示形态。

## What I already know（代码现状分析）

### 1. 卡片上下重叠 — 根因已定位
`FileGrid.vue` 用 `vue-virtual-scroller` 的 `RecycleScroller`，把"每行 6 个卡片"当作一个虚拟 item，`item-size = 160`。
但实际单行 DOM 高度 = `p-3`(24) + `h-24` 缩略图(96) + `mb-2`(8) + 文件名(~16) + `mt-1` 文件大小(~20) ≈ **164px+，超过 160**。
RecycleScroller 要求 `item-size` 精确等于真实行高，否则虚拟定位错乱 → 相邻行重叠/裁切。行间也无额外间距（`gap-4` 只作用于列）。

### 2. 只加载前几个文件 — 根因已定位
后端 `list_files` 默认 `limit = 50`，前端 `useResourceStore.fetchFiles()` **从不传 page/limit**，故永远只拿第一页 50 条。
RecycleScroller 只虚拟渲染这 50 条 → 表现为"后面文件加载不出来"。**不是虚拟滚动的锅，是前端缺少分页/无限滚动逻辑**。后端 `FileResponse { items, total }` 已返回 total，分页判断就绪。

### 3. 文件名搜索 — 当前不支持
后端 `FileQuery` 无 keyword/q 字段；前端无搜索框，只能靠左侧标签树筛选。需后端加模糊匹配 + 前端加搜索框。

### 4. 视图切换 — 当前只有卡片
仅 `FileGrid.vue` 卡片视图。需新增列表视图组件 + 顶部切换 toggle + 视图偏好持久化。

## Requirements（evolving）

- [ ] **卡片间距修复**：消除虚拟滚动导致的上下行重叠/裁切，行高与 `item-size` 严格一致。
- [ ] **无限滚动加载**：滚到底部按 page 追加拉取，直到 `items.length >= total`；与标签筛选/搜索联动重置分页。
- [ ] **文件名搜索**：后端 `FileQuery` 增加关键词字段（`filename LIKE`，不区分大小写，中文友好），与标签筛选 AND 组合；前端 header 增搜索框，防抖触发。
- [ ] **卡片/列表视图切换**：新增列表视图（每行一个文件，展示名/大小/时间/路径等），header 切换 toggle，视图偏好持久化（localStorage）。

## Acceptance Criteria（evolving）

- [ ] 卡片视图：任意滚动位置无上下行重叠/裁切，缩略图、文件名、大小完整可见。
- [ ] 文件数 > 50 时，向下滚动能持续加载后续文件直到全部呈现。
- [ ] 输入文件名关键词后，列表仅显示文件名包含该关键词的文件（与已选标签 AND）。
- [ ] 切换"列表/卡片"后，当前文件集合保持不变，偏好刷新后保留。
- [ ] 搜索/标签切换/清空时，分页状态正确重置，不出现重复或遗漏。

## Definition of Done

- 前端 `npm run build` 通过、无 TS 错误。
- 后端 `cargo clippy` + `cargo test` 通过（新增搜索/分页相关单测）。
- 真实 e2e 验证：>50 文件库下滚动加载、搜索、视图切换均正常。
- 行为变更对应的前后端契约更新（如 FileQuery 新字段）在 prd/commit 说明。

## Out of Scope（explicit）

- 全文内容搜索（仅文件名级别，不索引文件正文）。
- 标签树/侧栏交互改动、文件详情抽屉改动。
- 高级筛选（按大小范围、时间范围、扩展名多选等）。
- 排序切换（当前固定 mtime DESC）。

## Technical Approach

### 卡片重叠修复
`RecycleScroller` 用绝对定位（transform）排布每个 item，**`item-size` 必须严格等于该 item 在垂直方向占位的总高度**（含行间距），否则后续 item 定位错乱 → 重叠/裁切。
→ 把"一行卡片"包成固定高度容器（如 `h-[168px]`），把行间距用容器内 `pb-*` 贡献进 `item-size`，并保证卡片内容不溢出该高度。两种视图各自的 `item-size` 独立设定。

### 无限滚动加载
- store 增加分页状态：`page`(从1起) / `pageSize` / `total` / `hasMore` / `loadingMore`。
- `fetchFiles()` = 重置 `page=1` 后拉取并**替换** `files`（搜索/标签切换时调用）。
- 新增 `fetchMore()` = `page+1` 后**追加** items，按 `items.length >= total` 置 `hasMore=false`。
- 滚动监听与视图解耦：两个视图组件都向 `RecycleScroller` 的 scroll 事件 emit `reach-bottom`，由 Home 统一触发 `fetchMore`（`hasMore && !loadingMore` 守卫，防重复）。

### 文件名搜索（仅文件名，AND 标签）
- 后端 `FileQuery` 增 `keyword: Option<String>`；3 个查询函数（all/recursive/direct）均追加 `AND LOWER(filename) LIKE ?`，模式 `'%'+kw+'%'`，并对 `%`/`_` 做 `ESCAPE` 转义防注入/误匹配。
- 前端 store 增 `keyword`，`fetchFiles` 透传；Home header 加 `<input>` + 300ms 防抖；输入变化时重置分页并重拉。

### 卡片/列表视图切换
- 新增 `FileList.vue`：精简单行，列 = 名称 / 大小 / 修改时间（lucide 文件图标 + 无缩略图）。
- Home header 加 `LayoutGrid` / `List` 图标 toggle；`viewMode` 存 localStorage，刷新保留。
- 两种视图共用 store.files 与无限滚动，切换时文件集合/分页不变。

## Implementation Plan（按功能拆 commit）

1. **后端文件名搜索契约** — `FileQuery.keyword` + 3 查询函数 `LIKE` 过滤 + `%`/`_` 转义 + 单测；前端 `fileApi.list` 透传 keyword。
2. **卡片重叠 bug 修复** — 行容器固定高度、`item-size` 严格对齐、间距纳入 item-size。
3. **列表视图 + 切换** — `FileList.vue` 精简单行 + header toggle + localStorage 持久化。
4. **无限滚动加载** — store 分页状态 + `fetchMore` + 滚动到底触发 + 搜索/标签联动重置。
5. **文件名搜索框** — header 输入框 + 防抖 + 联动重拉；e2e 验证。

## Decision (ADR-lite)

- **加载策略 = 无限滚动追加**：复用既有虚拟滚动 + 后端分页，首屏快、适配大库；放弃一次性全量的极简实现。
- **搜索范围 = 仅 filename**：语义清晰，路径筛选交由已有 path 标签；`parent_path` 匹配留作未来扩展。
- **列表视图 = 精简单行（名称/大小/时间）**：信息密度高、扫读快；缩略图/路径列未来按需加。
- **搜索与标签 = AND**：与现有"多标签 AND"语义一致。

## Technical Notes

- 前端关键文件：`tagflow-ui/src/components/FileGrid.vue`、`stores/useResourceStore.ts`、`views/Home.vue`、`api/http.ts`。
- 后端关键文件：`tagflow-core/src/api/file.rs`（`list_files` + 3 个查询函数）、`models/dto.rs`（`FileQuery`）。
- `RecycleScroller` 已是虚拟滚动组件，分页应在其滚动到底事件上触发下一页。
- SQLite `LIKE` 默认对 ASCII 不区分大小写，中文无大小写问题，直接 `LIKE '%kw%'` 即可（注意 `%`/`_` 转义）。
- 文件名/路径含中文，搜索需确保 UTF-8 正确透传（axum query 已 percent-decode）。
