# 媒体预览抽屉（文件详情 + 多类型预览 + 下载）

## Goal

点击文件网格里的文件 → 右侧抽屉带动画滑出，展示文件详情（元数据 + 标签）并按类型预览（文本/Markdown/PDF/图片/视频/音频），提供下载。顺手修复"文件卡片 cursor:pointer 但点击无反应"和"缩略图被 401 静默隐藏"两个 UX bug，让 523 本小说库真正可用。

## What I already know

**用户明确诉求**
- 预览支持 4 大类型：文本阅读器、图片、视频、音频播放
- 提供下载功能
- 详情面板用**抽屉**形式
- 要有**优雅的展开/收起动画**

**后端现状**
- `StorageManager::get_operator(library)` 返回 OpenDAL `Operator`（仅 local；webdav 预留）
- OpenDAL：`op.read(path)` 全量读、`op.read_with(path).range(start..end)` 区间读、`op.reader(path)` 返回可流式 `Reader`
- 文件路径 = `parent_path + filename`（相对 library root）
- `GET /api/v1/files/:id/thumbnail`（`api/file.rs:227`）是"流式返回二进制 + Cache-Control"参考模式，但它只接收 `Path(id)`、不查 DB、不解析 library——内容端点要更重，需 `State(pool)`
- `auth_middleware`（`api/auth.rs:110`）：**无 Bearer 一律 401**

**前端现状**
- `vue-virtual-scroller ^2.0.0-beta.8` 已是依赖（FileGrid 用 `RecycleScroller`）
- 纯 Tailwind，无动画库，无 `<Transition>` 先例
- `FileGrid.vue:67` 缩略图用裸 `<img src>`（不带 Authorization）→ **被 401，`@error` 隐藏，显示图标**
- 文件卡片有 `cursor-pointer` 但**无 @click**
- `FileItem`（dto.rs:20）字段：id/filename/extension/size/mtime/parent_path —— **无 tags、无 library_id**
- axios 实例（http.ts）请求拦截器自动附加 `Authorization: Bearer`，但**仅对 axios 发起的请求生效**

**关键约束（已验证）**
- 浏览器原生 `<img>/<video>/<audio src>` **不会**自动带 Authorization 头 → 受保护路由下的媒体 src 会 401
- blob URL（fetch 带 token → createObjectURL）方案：图片/音频 OK，但**视频大文件必须整文件下载完才能 seek**，体验不可接受
- 因此媒体 src 必须走"URL 携带凭证"路线

## Requirements (locked)

### 后端
1. **新增 `GET /api/v1/files/:id/content`** —— 内容端点
   - 解析 file_id → file 行（library_id/parent_path/filename/extension/size），不存在或 `status != 1` 返回 404
   - file_id 走 DB，**不接受路径参数**（防目录遍历）
   - 按扩展名分流：
     - **文本类**（txt/md/log/csv/json/xml/html/...）：`op.read()` 全量读字节 → 编码转码（见下）→ 返回 `text/plain; charset=utf-8`，**不走 Range**
     - **图片/视频/音频/PDF 类**：原始字节流，content-type 由 `mime_guess` 推断（PDF → `application/pdf`），**支持 Range**（解析 `Range: bytes=`，返回 206 + `Content-Range`/`Content-Length`/`Accept-Ranges: bytes`）
     - 其他/未知：`application/octet-stream`，支持 Range
   - `?download=1`：追加 `Content-Disposition: attachment; filename="<url-encoded filename>"`
2. **编码转码**（文本类专用）：新增 `encoding_rs` 依赖
   - 检测 BOM（UTF-8 BOM / UTF-16）→ 否则 `String::from_utf8` 严格校验 → 失败则 `encoding_rs::GBK.decode()` 兜底（覆盖中文 GBK/GB18030 小说）
   - 全程在后端转成 UTF-8 输出，前端拿到的就是干净字符串
3. **新增 `GET /api/v1/files/:id`** —— 文件详情端点
   - 返回完整元数据 + 该文件的标签列表（`[{id, name, category}]`，按 category 分组）
   - 用单独端点而非塞进 list，避免列表每行多一次 join
4. **auth_middleware 增强**：除 `Authorization: Bearer` 外，额外接受查询参数 `?token=<jwt>` 作为凭证（媒体 src 用，普通 axios 仍走 header）
5. 路由注册：`/api/v1/files/:id`、`/api/v1/files/:id/content` 挂到 protected_routes

### 前端
6. **新增 `FileDrawer.vue`**：右侧抽屉
   - 用 Vue `<Transition>` + CSS：translate-x(100%→0) + opacity + backdrop fade，cubic-bezier 缓动，约 280ms
   - 顶部：文件名 + 关闭按钮（X）
   - 元数据区：大小 / 修改时间 / 路径 / 扩展名 / 推断类型
   - 标签区：按 category 分组的 chip
   - 预览区：按类型切换组件（TextReader / MarkdownView / PdfFrame / 图片 / video / audio / 不支持提示）
   - 底部：下载按钮（带 token 的 content?download=1）
   - 关闭：点遮罩 / 点 X / 按 Esc
7. **媒体 URL helper**：`contentUrl(fileId, qs?)` → 自动拼 `?token=<jwt>`（从 auth store 取），供 `<img>/<video>/<audio>/<iframe src>` 与下载使用
8. **修复缩略图**：FileGrid 缩略图 src 改用 `contentUrl` 同款 token 拼接（缩略图路由用 `/thumbnail`）→ 缩略图真正显示
9. **FileGrid 卡片加 @click** → 设置 `selectedFileId` 打开抽屉
10. **useResourceStore**：新增 `selectedFileId: number | null` + `openFile(id)` / `closeFile()`
11. **文本/文档预览**（抽屉内）：fetch content（axios，走 header token）→ `response.text()`；按扩展名分流：
    - `.md`：`markdown-it`（配 `html: false` 禁原始 HTML 直通，免 XSS）渲染成 HTML，v-html + 排版样式
    - `.pdf`：`<iframe :src="contentUrl(id)">` 浏览器原生渲染（桌面 Chrome/Edge/Firefox 内置 PDF）
    - 其他文本（txt/log/csv/json/...）：按 `\n` 切行 → `DynamicScroller` 虚拟滚动，`white-space: pre-wrap`
    - 加载/转码失败显示提示

## Acceptance Criteria

- [ ] 点击文件卡片 → 右侧抽屉带 ~280ms 滑出动画
- [ ] 抽屉显示：文件名、大小、修改时间、路径、扩展名、类型、该文件所有标签（按 category 分组）
- [ ] 文本文件（含 8MB 大 txt / GBK 编码中文小说）：阅读器正确渲染，无乱码，滚动流畅
- [ ] Markdown 文件：渲染成 HTML（标题/列表/代码块），原始 HTML 不直通（安全）
- [ ] PDF 文件：抽屉内 iframe 预览（桌面浏览器原生渲染）
- [ ] 图片：显示原图，点击可全屏
- [ ] 视频：可播放，**可拖动进度条 seek**（验证 Range 206 生效）
- [ ] 音频：可播放
- [ ] 下载按钮：触发浏览器下载，文件名正确
- [ ] 关闭抽屉（遮罩 / X / Esc）：带收起动画
- [ ] 缩略图在网格中正常显示（不再被 401 隐藏）
- [ ] 非侵入：只读，不修改用户文件

## Definition of Done

- 后端 content 端点单测：Range 解析、content-type 推断、文本转码（UTF-8 直通 + GBK 兜底）、不存在的 file_id 404、download header
- 后端 auth_middleware `?token=` 分支单测
- `cargo clippy` + `cargo test` 通过；前端 `vue-tsc && vite build` 通过
- e2e（chrome-devtools）：文本（含 GBK）/md/pdf/图片/视频/音频各点开一次，验证预览 + 动画 + 缩略图显示 + 下载

## Technical Approach

**媒体鉴权（本任务最大决策）**：`auth_middleware` 增加 `?token=<jwt>` 兜底。理由：blob 方案破坏视频 seek；cookie 方案改动过大；签名短 token 增加复杂度。自托管单用户 LAN 场景下，JWT 进 URL 可接受（已在 localStorage，本来就有 XSS 暴露面），后续可硬化为短时效 media token。

**单一内容端点 + 类型分流**：一个 `get_content` handler 内按 extension 分文本/媒体/其他三路。文本转码在后端一次性完成（encoding_rs），前端免装编码库。

**文本/文档预览**：后端转码输出 UTF-8 `text/plain`；前端 axios 取文本（走 header token）→ 按扩展名分流：.md 走 markdown-it（html:false）渲染；.pdf 走 iframe 浏览器原生；其余文本按行切 → `DynamicScroller` 虚拟滚动（动态行高，适配换行）。8MB 文本几十万行，虚拟滚动保证 DOM 不爆。

**详情标签**：独立 `GET /files/:id`，join file_tags+tags，保持列表查询轻量。

**抽屉动画**：Vue 3 内置 `<Transition>` + Tailwind/CSS，零新依赖（markdown-it 除外）。遮罩 `v-enter-from opacity-0`，面板 `translate-x-full → translate-x-0`。

## Decision (ADR-lite)

**Context**：4 类媒体预览 + 抽屉，最大障碍是受保护路由与浏览器原生媒体 src 不兼容，且现有缩略图已因此静默失效。

**Decision**：
- D1 文本阅读器：全文阅读 + 后端 GBK→UTF-8 转码（encoding_rs）+ 前端 DynamicScroller 虚拟滚动
- D2 抽屉动画：Vue `<Transition>` + CSS，不加动画库
- D3 图片：native `<img>` object-contain，点击全屏；不做 zoom/pan
- D4 视频/音频：HTML5 原生 `<video controls>`/`<audio controls>`，Range seek
- D5 MVP 边界：纳入 Markdown 渲染（markdown-it, html:false）+ PDF（iframe 浏览器原生）；代码高亮 / 视频字幕 OUT
- D6（关键）媒体鉴权：auth_middleware 接受 `?token=<jwt>` 兜底，前端 contentUrl helper 统一拼接；顺带修复缩略图 401
- D7 文件详情：独立 `GET /files/:id` 返回元数据 + tags

**Consequences**：
- JWT 进入 URL（server log / 浏览器历史可见）——自托管 LAN 单用户可接受，记为已知妥协，未来可换短时效 media token
- 新增后端 `encoding_rs`（纯 Rust、零 unsafe）+ 前端 `markdown-it`（~30KB，html:false 配置免 XSS，无需 DOMPurify）
- 缩略图 bug 随 `?token=` 方案一并修复（Scope 内顺手）
- PDF 依赖浏览器原生能力，移动 Safari 会退化为下载（桌面访问为主，可接受）

## Out of Scope

- rename / move / delete（破坏性操作，未来单独任务）
- 手动标签编辑 UI（下一个任务，但详情面板要能"显示"标签）
- WebDAV 库的预览（当前 webdav 未启用）
- Office 文档（docx/xlsx/pptx）原生预览
- 代码语法高亮
- 视频字幕（.srt/.vtt）
- 图片 zoom/pan、视频播放器自定义皮肤
- 短时效 media token 硬化（记为未来改进）

## Technical Notes

**关键改动点**
- `tagflow-core/Cargo.toml`：加 `encoding_rs = "2"`
- `tagflow-core/src/api/file.rs`：新增 `get_content`、`get_file_detail`；Range 解析 + 类型分流 + 转码
- `tagflow-core/src/api/auth.rs`：`auth_middleware` 增加 `?token=` 解析分支
- `tagflow-core/src/models/dto.rs`：新增 `FileDetail`（含 tags）、`FileTagInfo`
- `tagflow-core/src/main.rs`：注册 `/api/v1/files/:id`、`/api/v1/files/:id/content`
- `tagflow-ui/package.json`：加 `markdown-it`
- `tagflow-ui/src/components/FileDrawer.vue`：新建（含文本/md/pdf 预览分支）
- `tagflow-ui/src/components/FileGrid.vue`：缩略图改 token 拼接，卡片加 @click
- `tagflow-ui/src/stores/useResourceStore.ts`：加 selectedFileId / openFile / closeFile
- `tagflow-ui/src/stores/auth.ts`：token 已暴露供 contentUrl 取用
- `tagflow-ui/src/api/http.ts`：加 `fileApi.detail(id)`、`contentUrl(id, opts)`（含 token 拼接）

**约束 / 坑**
- Range 请求：`Range: bytes=0-` / `bytes=1000-2000`，返回 206 + `Content-Range: bytes start-end/total`
- OpenDAL 大文件优先 `op.reader()` + `ReaderStream` 流式，避免整文件入内存；Range 用 `read_with().range()`
- `<video>` 可能发 HEAD —— 注意 axum 默认只注册 GET，必要时补 HEAD 路由
- rust-embed 不随 dist 变更自动重嵌，前端构建后 `touch src/api/static_files.rs` 再 `cargo build`
- 安全：file_id 走 DB 解析，filename 进 `Content-Disposition` 前必须 URL 编码 + 处理 CRLF
- markdown-it 必须配 `html: false`，否则 md 内嵌的原始 `<script>/<img onerror>` 会直通造成 XSS

**研究参考**
- `?token=` query 兜底是自托管 JWT 应用媒体加载的通行做法（Immich/Jellyfin 类项目类似权衡）
- encoding_rs 是 Rust 编码转码事实标准（Firefox 在用，零 unsafe）
- PDF iframe 方案依赖桌面浏览器内置 PDF 渲染（Chrome PDFium / Firefox pdf.js）

## Implementation Plan (small PRs)

- **PR1 后端**：content 端点（Range + 类型分流 + GBK 转码）+ detail 端点 + auth `?token=` 兜底 + 单测
- **PR2 前端**：FileDrawer（含 md/pdf/文本分支）+ contentUrl helper + FileGrid 接线（缩略图带 token、卡片 click）
- **PR3 验证**：vue-tsc/build + cargo clippy/test + chrome-devtools 全类型 e2e
