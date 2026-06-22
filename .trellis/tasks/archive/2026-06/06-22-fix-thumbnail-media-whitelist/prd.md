# 修复非媒体文件缩略图 404（前端媒体白名单）

## Goal

修复前端对非媒体文件（文本/PDF/代码/归档等）请求缩略图导致 404 刷屏。前端 `FileGrid.vue` 只对媒体文件（与后端 `MEDIA_EXTENSIONS` 一致）渲染缩略图 `<img>`，非媒体直接显图标、不发请求。发版阻塞（体验 + 干净 console + 节省请求）。

## 根因（已诊断，2026-06-22）

- `FileGrid.vue:111-119` 对**所有文件**无条件渲染 `<img :src="fileApi.thumbnailUrl(file.id)">`。
- 后端只为 `MEDIA_EXTENSIONS`（`scanner/mod.rs:141`：`jpg/jpeg/png/gif/webp/bmp` + `mp4/mov/m4v/mkv/avi/webm`）入列缩略图任务。
- 非媒体文件永不生成缩略图 → `get_thumbnail` 读 cache 不存在返 404（`file.rs:302`）→ 前端 `@error` 设 opacity:0 显图标（功能正常），但浏览器已发请求 + console 刷 404。
- **附带不一致**：前端 `getFileIcon` 的 `imageExts` 含 `svg`，后端 `MEDIA_EXTENSIONS` 不含 svg → svg 会被请求缩略图但后端不生成（同样 404）。

## 修复

`FileGrid.vue` 加 `isMediaFile(ext)`（与后端 `MEDIA_EXTENSIONS` 完全一致），`<img>` 加 `v-if="isMediaFile(file.extension)"`，非媒体不渲染 img → 不发请求。

## Requirements

### 前端（`tagflow-ui/src/components/FileGrid.vue`）
- 加 `MEDIA_EXTENSIONS` 常量，**与后端 `scanner/mod.rs:141` 完全一致**：`jpg/jpeg/png/gif/webp/bmp + mp4/mov/m4v/mkv/avi/webm`（不含 svg）。
- 加 `isMediaFile(ext: string | null): boolean`。
- `<img>` 加 `v-if="isMediaFile(file.extension)"`，非媒体不渲染（不发 thumbnail 请求，只显图标）。
- `getFileIcon` 的 `imageExts` 是**图标分类**用途（svg 仍显 ImageIcon），与 `MEDIA_EXTENSIONS`（缩略图请求判断）**分离**，不强行合并（两者语义不同：图标可含 svg，缩略图不含）。

### e2e（`tagflow-e2e/`）
- `fixtures/library/` 加一个文本文件（如 `notes.txt`），作为非媒体夹具。
- 更新 `lib/env.ts` 的 `EXPECTED_FILE_COUNT`（5 → 6）。
- 新增/扩展用例：用 Playwright 拦截 thumbnail 网络请求（`page.on('request')` 或 `route`），加载首页，断言发起的 thumbnail 请求**只针对媒体文件 id**，不含 `notes.txt` 的 id。

## Acceptance Criteria

- [ ] `FileGrid.vue` 非媒体文件不渲染 thumbnail `<img>`（浏览器不发请求）。
- [ ] 前端 `MEDIA_EXTENSIONS` 与后端 `scanner MEDIA_EXTENSIONS` 完全一致。
- [ ] `npm run build`（vue-tsc + vite build）干净。
- [ ] e2e：非媒体文件不发起 thumbnail 请求（网络拦截断言）；既有 11 用例不回归（`EXPECTED_FILE_COUNT` 更新后 files.spec 仍绿）。
- [ ] 真实复验（用户）：NAS 扫描含文本文件的库，console 无文本文件 thumbnail 404。

## Testing

- e2e 网络拦截：`page.on('request', req => { if (req.url().includes('/thumbnail')) seen.add(...) })`，加载首页后断言 `seen` 不含非媒体文件 id（需要先拿非媒体文件的 id，通过 `GET /files` 找 `notes.txt` 的 id）。
- 更新 `files.spec.ts` 断言（文件数 5→6）。

## Definition of Done

- `FileGrid.vue` 改造 + `npm run build` 干净。
- e2e 非媒体不请求用例通过 + 既有用例不回归。
- spec：记「前端媒体白名单必须与后端 `MEDIA_EXTENSIONS` 一致」契约（防 svg 类不一致再犯）。
- 用户 NAS 复验 console 无文本文件 404。

## Technical Approach

1. **FileGrid.vue**：加 `MEDIA_EXTENSIONS`（与后端逐字一致）+ `isMediaFile`；`<img v-if="isMediaFile(...)">`。
2. **图标 vs 缩略图白名单分离**：`getFileIcon` 的 `imageExts`（含 svg，图标分类）与 `MEDIA_EXTENSIONS`（不含 svg，缩略图判断）各自独立，注释说明语义差异。
3. **e2e**：`fixtures/library/notes.txt`；`EXPECTED_FILE_COUNT=6`；新用例拦截 thumbnail 请求断言非媒体不发起。

## Decision (ADR-lite)

- **Context**：前端对所有文件请求缩略图，非媒体 404 刷屏；前端图标白名单（含 svg）与后端媒体白名单（不含 svg）不一致。
- **Decision**：前端按 `MEDIA_EXTENSIONS` 判断是否请求缩略图，非媒体不发；白名单与后端 `scanner` 逐字一致，绑定为 spec 契约。图标分类白名单（`getFileIcon`）独立保留。
- **Consequences**：消除非媒体 404 噪音 + 节省请求；媒体文件「生成中」的 404 仍存在（预期，@error 兜底，本次不消除）；前后端白名单绑定为契约，改后端 `MEDIA_EXTENSIONS` 必须同步前端。

## Implementation Plan (small PRs)

- **PR1**：`FileGrid.vue` 加 `isMediaFile` + `<img v-if>` + `MEDIA_EXTENSIONS`；`npm run build` 干净。
- **PR2**：e2e fixtures 加 `notes.txt` + `EXPECTED_FILE_COUNT=6` + 网络拦截用例；spec 记白名单一致性契约。

## Out of Scope

- 媒体文件「生成中」404 的消除（需后端 thumbnail API 返 202/占位，独立增强）。
- 缩略图 loading 占位 UI。

## Technical Notes

- 关键文件：`tagflow-ui/src/components/FileGrid.vue`、`tagflow-core/src/engine/scanner/mod.rs:141`（`MEDIA_EXTENSIONS`）、`tagflow-e2e/fixtures/library/`、`tagflow-e2e/tests/files.spec.ts`、`tagflow-e2e/lib/env.ts`（`EXPECTED_FILE_COUNT`）。
- 后端 `MEDIA_EXTENSIONS` 是 single source of truth，前端必须逐字同步（注释标明来源行号）。
