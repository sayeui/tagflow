# 类型/扩展名标签与多标签分面过滤

## Goal

补齐 TagFlow 自动标签引擎缺失的"类型/扩展名"维度，并在前端实现真正的多标签 AND 过滤，让分面搜索（Faceted Search）可用。当前部署（NAS / 523 个 txt 文件）侧栏只有 1 个 `#500+` 标签，左侧树几乎为空，用户"功能缺失感"主要来源于此。

## What I already know

**当前实现状态（已通过代码 + API + UI 三层核对）**
- `tagger/` 目录仅 `PathTagger` 一个文件，scanner 在 `insert_file` 处仅调用它
- `TagManager::ensure_path_tags` 硬编码写入 `category='path'`，**没有通用 ensure_tag 方法**
- `FileQuery` 只有 `tag_id: Option<i32>`（单值），`list_files` 用递归 CTE 查询子树文件
- 前端 `useResourceStore` 只有 `selectedTagId: number | null`，`fetchFiles(tagId?)` 单值传参
- `TagNode.category` 字段已存在（dto.rs:9），但前端 `TagItem.vue` 未按 category 分组渲染
- DB schema `tags` 表已有 `category` 列，枚举 `Path/Type/User/Time`（models/db.rs:6-19）
- 数据库实际状态：523 个 txt 文件，1 个 path 标签 `#500+`，无任何 type/ext/year 标签

**需求文档原文（doc/TagFlow需求调研.md §3.2.2）**
- 拓展名标签：`#ext:jpg`、`#ext:pdf`、`#ext:md`
- 宏类型标签：`#type:image` ← jpg/jpeg/png/gif/heic/webp；`#type:video` ← mp4/mkv/mov/avi；`#type:audio` ← mp3/flac/wav；`#type:code` ← js/py/go/rs/html/css；`#type:document` ← pdf/doc/docx/xls/ppt
- 需求未明示：`.txt` 属于哪一类？archive（zip/rar）？json/markdown？

## Decisions (confirmed)

**D1 — 类型分桶方案（A 方案）**：在原 5 桶（image/video/audio/code/document）基础上新增 `text` 桶。
- `#type:text` ← `.txt / .md / .log / .csv`
- 其余 5 桶按需求文档原映射，保持不变
- 未知扩展名（`.epub` / `.mobi` / 无扩展名）：**跳过，不生成 type 标签**（避免 `#type:other` 兜底桶膨胀）
- `.md` 暂不拆 `markdown` 子标签，扁平够用

**D2 — 标签存储格式**：name 存裸值，category 字段区分。
- DB：`name="text", category="type"`，`name="txt", category="ext"`
- 前端展示：拼前缀 `#type:text`、`#ext:txt`

**D3 — 多选 UX 模式**：复选框树（最小改动 MVP）。
- 左侧 `TagItem.vue` 每个叶子节点加 checkbox
- 顶层按 category 分组（type / ext / path / time 作为分组标题）
- store：`selectedTagIds: number[]`，checkbox 勾选/取消 → 数组增删 → 触发 AND 查询
- 顶栏 `当前查看:` 显示当前过滤上下文（如 `type:text ∧ ext:txt`）
- 分组标题节点（type/ext/path/time）本身不可勾选，仅作容器

**D4 — time 标签（B 方案）**：year + month 都做。
- `#year:2024`、`#month:2024-05`，从 `files.mtime`（Unix 秒）生成
- 层级嵌套：year 是 parent，month 挂在对应 year 下（复用 `ensure_path_tags` 式的层级逻辑）
- 无 mtime 或 mtime ≤ 0：跳过 time 标签

**D5 — 时区策略**：服务器本地时间（`TZ` 环境变量）。
- 部署环境 `TZ=Asia/Shanghai`（docker-compose.yml:32 已配置，注释已预留 time 标签用途）
- 未设置 TZ 时回退 UTC（`chrono::Local` 默认行为）
- 用 `chrono::DateTime<Local>` 从 mtime 转换，取 year/month

**D6 — 月份格式**：ISO `YYYY-MM`。
- `#month:2024-05`（不简写为 `2024-5`，保证字典序排序正确）

**D7 — 回填策略**：升级后自动一次性回填。
- 新增 meta 表 `app_meta(key, value)`，记录 `tagger_version`（当前 = 2，代表 type/ext/time tagger 全量）
- 启动时若版本落后，对 `files` 全表跑新 tagger（批事务，每 1000 条提交）
- 用户无感，无需手动重建库

## 边界与异常处理（Expansion Sweep）

- **无扩展名文件**（如 `README`）：跳过 ext + type 标签
- **大小写**：ext 已由 scanner 统一小写（`scanner/mod.rs:109`），tagger 直接用
- **路径组件与 year 重名**：`Projects/2024/x.txt` → path 标签 `2024` + time 标签 `2024`，category 不同互不冲突，AND 查询各自独立
- **mtime ≤ 0**：跳过 time 标签
- **跨年边界**（12/31 23:59 → 1/1 00:01）：由 D5 时区决定归属，行为可预测
- **回填性能**：523 文件无压力；为 10 万级文件预留批事务，不一次性 load 全表

## Assumptions (temporary)

- 类型/扩展名标签生成是 scanner 流水线的一部分（与 PathTagger 同步触发）
- 现有 523 txt 文件需要**重新扫描**才能补齐新标签（不做 lazy backfill）
- 多标签 AND 过滤语义：选了 `#type:text` + `#2024` → 同时满足两个条件的文件
- 标签树前端**按 category 分组**（path / type / ext / time 作为顶层分组节点）

## Open Questions

全部已收敛（见 D1–D7）

## Requirements (locked)

**后端**
- 新增 `extension_tagger`：从 `files.extension` 生成 `#ext:<ext>` 标签（category=ext）
- 新增 `type_tagger`：按 D1 映射表生成 `#type:<bucket>` 标签（category=type），未知扩展名跳过
- 新增 `time_tagger`：按 D5/D6 生成 `#year:<YYYY>`（category=time）和 `#month:<YYYY-MM>`（category=time，parent=对应 year）
- `TagManager` 抽出通用 `ensure_tag(name, category, parent_id) -> id`，`ensure_path_tags` 复用它
- scanner `insert_file` 在 PathTagger 后追加三个新 tagger 调用
- `FileQuery` 增加 `tag_ids: Option<Vec<i32>>`（保留 `tag_id` 单值兼容或废弃，二选一——倾向废弃，前端唯一调用方）
- `list_files` 支持 tag_ids 多值 AND（INTERSECT 或 `GROUP BY file_id HAVING COUNT = N`）
- 回填：`app_meta` 表 + 启动时版本检查 + 批量回填（D7）

**前端**
- `TagItem.vue` 改为支持 checkbox + 按 category 分组的递归树
- `useResourceStore`：`selectedTagId` → `selectedTagIds: number[]`，`fetchFiles(tagIds: number[])`
- `http.ts`：`fileApi.list` 支持 `tag_ids` 数组参数（axios paramsSerializer）
- `Home.vue`：顶栏 `当前查看:` 显示多标签拼接的过滤上下文（如 `type:text ∧ ext:txt`）

## Acceptance Criteria

- [ ] 扫描新库后，DB 出现 category ∈ {ext, type, time} 的标签记录
- [ ] 现有 523 txt 文件升级后自动回填，全部关联 `#ext:txt` + `#type:text` + 对应 `#year:` / `#month:`
- [ ] 左侧树按 type/ext/path/time 四组分组展示，叶子节点带 checkbox
- [ ] 勾选 `#type:text` + `#ext:txt` → 文件列表 = 同时含两标签的文件（AND）
- [ ] 取消勾选 → 实时重新过滤
- [ ] 顶栏正确显示当前过滤上下文
- [ ] tagger 单测覆盖：ext 提取、type 映射（含未知扩展名 skip）、year/month 生成（含 mtime≤0 skip）、时区转换
- [ ] list_files 多标签 AND 查询单测（含跨 category、含 0 结果）
- [ ] cargo clippy / cargo test / npm run build 全绿

## Definition of Done (team quality bar)

- 后端：新增 tagger 单测覆盖（ext 提取、type 映射、未知扩展名 fallback）
- 后端：list_files 多标签 AND 查询单测
- 前端：store 多选状态 + fetchFiles 多 tag_ids 参数
- 前端：TagItem 或新组件支持多选交互
- cargo clippy / cargo test / npm run build 全绿
- 部署文档更新（如配置项有变化）

## Out of Scope (explicit)

- 手动标签（user tag）—— P1，下一个任务
- 文件操作（重命名/删除/下载）—— P1
- 列表视图切换 —— P2
- EXIF / 视频元数据解析（需求 §3.2.3 进阶项）—— P2
- WebDAV 资源库 —— P3
- `.gitignore` 忽略规则 —— P3
- 全文搜索 —— 未在原需求核心范畴
- 标签数量统计显示（facet 后缀 `(523)`）—— 视前端工作量决定是否本轮顺手做

## Technical Approach

**type 映射表（D1 + 原需求 §3.2.2）**

| type | 扩展名 |
|------|--------|
| image | jpg jpeg png gif heic webp bmp svg |
| video | mp4 mkv mov avi webm flv |
| audio | mp3 flac wav aac ogg m4a |
| code | js ts py go rs java c cpp h cs rb php swift kt vue html css sql sh |
| document | pdf doc docx xls xlsx ppt pptx odt ods odp |
| text（新增） | txt md log csv |

未列出的扩展名 → 不生成 type 标签。

**多标签 AND 查询 SQL（核心）**

```sql
-- 选了 N 个 tag_id，要求文件同时关联全部
SELECT f.* FROM files f
JOIN file_tags ft ON f.id = ft.file_id
WHERE ft.tag_id IN (?, ?, ...)   -- N 个 tag
  AND f.status = 1
GROUP BY f.id
HAVING COUNT(DISTINCT ft.tag_id) = N
ORDER BY f.mtime DESC LIMIT ? OFFSET ?
```

注：递归子树展开（原 tag_id 单值时的 `WITH RECURSIVE sub_tags`）需在多值场景下对每个 tag_id 展开，再交并 —— 实现上先展开所有 tag_id（含子树）成扁平 id 集合，再套上面的 GROUP BY/HAVING。

**回填伪代码（D7）**

```
on_startup:
  version = app_meta.get("tagger_version") or 0
  if version < CURRENT_TAGGER_VERSION:
    backfill_all_files()   # 遍历 files，对每条跑 ext/type/time tagger
    app_meta.set("tagger_version", CURRENT_TAGGER_VERSION)
```

## Implementation Plan (4 PR)

- **PR1 后端 tagger 流水线**：`TagManager::ensure_tag` 泛化 + 3 个新 tagger + scanner 接线 + type 映射常量 + 单测
- **PR2 后端 API 多标签查询**：`FileQuery.tag_ids` + `list_files` AND 查询重写 + 递归展开兼容 + 单测
- **PR3 前端复选框树 + AND 过滤**：`TagItem.vue` checkbox/分组 + store 多选 + http paramsSerializer + Home 面包屑
- **PR4 回填 + 部署**：`app_meta` migration + 启动回填钩子 + Docker 重建部署验证

## Technical Notes

**核心改动点**（按文件）：
- `tagflow-core/src/core/tag/mod.rs`：抽出通用 `ensure_tag(name, category, parent_id) -> id` 方法，`ensure_path_tags` 改为它的特化
- `tagflow-core/src/engine/tagger/`：新增 `extension_tagger.rs`、`type_tagger.rs`（可能合并为一个 file_meta_tagger）
- `tagflow-core/src/engine/scanner/mod.rs:122`：在 PathTagger 后追加新 tagger 调用
- `tagflow-core/src/models/dto.rs:30`：`FileQuery` 增加 `tag_ids: Option<Vec<i32>>`，保留 `tag_id` 兼容（或直接换）
- `tagflow-core/src/api/file.rs:15`：list_files 支持 tag_ids 多值 AND 查询（SQL 改为 INTERSECT 或 HAVING COUNT）
- `tagflow-ui/src/stores/useResourceStore.ts`：`selectedTagId` → `selectedTagIds: number[]`
- `tagflow-ui/src/components/TagItem.vue`：递归渲染按 category 分组的多选树
- `tagflow-ui/src/views/Home.vue`：面包屑显示当前过滤上下文（如 `#type:document > #2024`）

**关键约束**：
- 标签 name 需带前缀（如 `type:document`、`ext:txt`）还是依赖 category 字段区分？需决策
- DB schema 是否需要为 `(name, category, parent_id)` 加唯一索引？（当前仅 `name` 上有 UNIQUE？需查 migration）
