# 自动化测试框架 Playwright e2e

## Goal

为 TagFlow（Rust 后端 + Vue3 前端，rust-embed 嵌入式单服务）引入**自动化端到端测试**，把目前每个功能都靠手动浏览器验证（见 journal）的流程固化成可重复执行的契约。主力用 Playwright 覆盖关键用户流，使登录、文件浏览、搜索、视图切换、标签树、资源库增删/扫描等核心路径回归可被自动化捕获。

## What I already know（探查所得，2026-06-16）

### 后端可注入项（决定测试隔离零成本）
- `infra/config.rs`：`TAGFLOW_DB_PATH`（缺省 `tagflow.db`）、`TAGFLOW_CACHE_DIR`（缺省 `./cache`）已收敛为单一来源，`db_url()` 用 `sqlite:<path>?mode=rwc` 自动建库。
- `main.rs`：`TAGFLOW_PORT` 可覆盖（缺省 8080）；debug 构建下 `TAGFLOW_JWT_SECRET` / `TAGFLOW_ADMIN_PASSWORD` 缺失回退开发默认值（admin / `tagflow_dev_only_admin_pw`）。
- **结论**：测试只需注入临时 DB 路径 + 临时 cache 目录 + 测试端口 + 固定管理员密码即可完全隔离，**无需改动后端任何代码**。

### 异步机制（e2e 要等待的点）
- `worker.rs:137`：后台 worker 轮询 `tasks` 表间隔 **5 秒**；任务 Pending(0)→Running(1)→Completed(2)/Failed(3)。
- 扫描：`POST /libraries/:id/scan` 立即返回，缩略图任务入队异步处理；同库扫描进行中返回 409。
- 缩略图：`GET /files/:id/thumbnail` 未生成时返回 **404**（`file.rs:302`），缓存 `cache_dir/{id}.webp`；依赖外部 `ffmpeg` 在 PATH。

### 前端定位现状
- `Login.vue`：`id="username"` / `id="password"`，可直接定位 ✅
- `Home.vue` / `FileGrid.vue`：**缺少 `data-testid`**，文件项依赖索引/内容定位；虚拟滚动用 `RecycleScroller`（行高 176px，6 列）。

### 既定规范约束（引入测试须同步更新）
- `.trellis/spec/frontend/quality-guidelines.md:10,38-43`：明确"无前端测试框架，靠 `npm run build` 类型检查 + 手动验证"；并写明"If introducing a test framework, that's a team decision"。
- `.trellis/spec/backend/quality-guidelines.md:39-48`：后端有行内 `#[cfg(test)]` 单元测试规范。
- **无 e2e 规范、无 CI（无 `.github/workflows`、无 Makefile）。**

## Assumptions（已通过探查验证）

- 后端 `cargo run`（debug）经 Playwright `webServer` 启动即可：rust-embed 已嵌入前端产物，单进程访问完整 UI（`main.rs` fallback → `static_handler`）。✅
- env 注入临时 DB / cache / 端口 / 账号即可隔离，**无需改后端**。✅（`infra/config.rs` 提供 `TAGFLOW_DB_PATH` / `TAGFLOW_CACHE_DIR`；`main.rs` 提供 `TAGFLOW_PORT` / `TAGFLOW_ADMIN_PASSWORD` / `TAGFLOW_JWT_SECRET`）
- ffmpeg 在本机 PATH（项目硬依赖），缩略图用例可直接跑；缺失时用 skip 兜底。

## Open Questions（已全部收敛）

## 已定决策

- ✅ **Q1（范围）→ 档位 B「smoke + 核心交互」**：覆盖 登录 / 文件列表 / 文件名搜索 / 视图切换 / 标签树 / 资源库扫描触发 / 缩略图懒加载。Vitest 组件单测**拆为后续独立任务**。
- ✅ **Q2（夹具）→ 内置小图片样本**：`tagflow-e2e/fixtures/library/` 放几张小图（PNG/JPG），带嵌套目录驱动路径标签生成、带中英文文件名测搜索；体积小、自包含、CI 友好。视频样本 Out of Scope。
- ✅ **Q3（定位）→ 必须**给 `FileGrid.vue` / `Home.vue` 补 `data-testid`：虚拟滚动项只渲染可见 DOM，`data-testid` 是唯一稳健定位手段。
- ✅ **Q4（spec 同步）→ 是**：本任务一并更新 `frontend/quality-guidelines.md` 的 Testing Requirements 章节——把「无前端测试框架、靠类型检查 + 手动验证」的现状升级为「e2e 由独立 `tagflow-e2e/` 的 Playwright 覆盖关键流、组件须补 `data-testid` 供虚拟滚动定位」，并把 Common Mistakes 的缩略图坑纳入 e2e 注意事项。

## Requirements (evolving)

- 引入 Playwright，配置 `webServer` 自动拉起**隔离的后端进程**（注入临时 DB 路径 + 临时 cache 目录 + 测试端口 + 固定管理员密码）。
- e2e 覆盖以下关键流（档位 B）：
  - 登录（测试账号由环境变量注入，走 `Login.vue` 的 `id=username/password`）。
  - 文件列表渲染（虚拟滚动，定位补 `data-testid`）。
  - 文件名搜索过滤。
  - 视图切换（若前端具备）。
  - 标签树渲染。
  - 资源库扫描触发（`POST /libraries/:id/scan`）+ 轮询等待文件出现。
  - 缩略图懒加载（轮询 `/files/:id/thumbnail` 从 404 变 200）。
- 测试**不触碰**真实 `tagflow.db` / `./cache`；每次运行用临时目录，跑完清理。
- 给 `FileGrid.vue` / `Home.vue` 关键元素补 `data-testid`。
- 同步更新 `frontend/quality-guidelines.md` 测试章节（Q4 已确认：一并更新）。

## Acceptance Criteria (evolving)

- [ ] 一条命令跑通全部 e2e：从零启动隔离后端到所有用例通过。
- [ ] 登录流程 e2e 通过（测试账号由环境变量注入）。
- [ ] 文件列表渲染 e2e 通过（虚拟滚动经 `data-testid` 可定位）。
- [ ] 文件名搜索过滤 e2e 通过。
- [ ] 标签树渲染 e2e 通过。
- [ ] 资源库扫描触发后，轮询等待文件出现 e2e 通过。
- [ ] 缩略图懒加载 e2e 通过（404→200 轮询）。
- [ ] `FileGrid.vue` / `Home.vue` 已补 `data-testid`，不破坏现有渲染。
- [ ] 测试运行后真实 `tagflow.db` / `./cache` 未被污染。
- [ ] `frontend/quality-guidelines.md` 测试章节已更新（Q4 已确认）。

## Definition of Done

- e2e 套件本地一键运行通过，README/journal 记录运行方式。
- 测试夹具与隔离机制有文档说明（ffmpeg 依赖、临时目录策略）。
- 相关 spec 文档同步更新。
- 不破坏现有 `cargo test` 与 `npm run build`。

## Technical Approach

### 目录结构
- 独立 `tagflow-e2e/`（monorepo 顶层），自带 `package.json`（Playwright 依赖）、`playwright.config.ts`、`tests/`、`fixtures/`。理由：e2e 驱动完整 Rust 后端进程，职责与纯前端 `tagflow-ui/` 不同，独立目录更清晰，未来加性能/可访问性测试也在此扩展。

### 后端隔离启动（零后端改动）
- Playwright `webServer` 执行 `cargo run`（debug 构建开发态；首次编译后增量快），通过环境变量注入隔离参数：
  - `TAGFLOW_DB_PATH` → OS 临时目录下 `tagflow-e2e.db`
  - `TAGFLOW_CACHE_DIR` → OS 临时目录下 cache/
  - `TAGFLOW_PORT` → 固定测试端口（如 18080）
  - `TAGFLOW_ADMIN_PASSWORD` → 固定测试密码（≥12 字节），账号 admin
  - `TAGFLOW_JWT_SECRET` → 固定测试密钥（≥32 字节）
- `reuseExistingServer: !process.env.CI`：本地可复用已起的后端提速，CI 强制新起。

### 测试夹具（内置图片）
- `tagflow-e2e/fixtures/library/` 放 3~5 张小图（PNG/JPG，几 KB~几十 KB），含嵌套子目录（如 `Projects/2024/`、`Photos/`）驱动路径→标签生成；文件名含中文与英文以覆盖搜索。
- globalSetup：起隔离后端后，通过 API 预置一个指向 `fixtures/library` 绝对路径的本地资源库并触发扫描，等待文件入库。

### 定位策略
- `FileGrid.vue` 文件卡片补 `data-testid="file-card"`（可附文件名后缀）；`Home.vue` 搜索框、视图切换等控件补对应 `data-testid`。`Login.vue` 已有 `id=username/password`，直接复用。
- 虚拟滚动定位：用 `getByTestId` + Playwright 定位器自带自动重试，不依赖固定索引。

### 异步等待策略（三个陷阱）
- **缩略图**：轮询 `GET /files/:id/thumbnail`，期望从 404 转 200；超时 ~15s（覆盖 worker 5s 轮询 + ffmpeg 处理余量）。
- **扫描**：触发后轮询 `GET /files?...` 直到文件出现。
- **ffmpeg 依赖**：globalSetup 探测 `ffmpeg -version`，缺失则缩略图相关用例 **skip（带明确原因）**，其余照跑——避免环境缺 ffmpeg 时全盘失败。

### 跑法
- `npx playwright test` 一条命令：globalSetup 起隔离后端 + seed → 跑用例 → teardown 清理临时 DB/cache。

## Decision (ADR-lite)

- **Context**：前端零自动化测试，每个功能靠手动浏览器验证；需把关键流固化为可重复 e2e 契约。后端路径/端口/账号已 env 化，隔离成本为零。
- **Decision**：Playwright 做主力 e2e；独立 `tagflow-e2e/` 目录；通过 env 注入临时 DB/cache/端口实现隔离（不改后端）；内置小图片夹具；视频缩略图、Vitest、CI 均 Out of Scope。
- **Consequences**：覆盖三个异步点（虚拟滚动/扫描/缩略图）的真实链路，回归可自动化捕获；代价是缩略图用例强依赖 ffmpeg（用 skip 兜底）。Vitest 单测后移，纯逻辑暂无单测覆盖（已知缺口，后续任务补）。

## Implementation Plan (small PRs)

- **PR1：脚手架 + 隔离 + 登录 smoke**。建 `tagflow-e2e/`（package.json、playwright.config.ts、webServer 拉起隔离后端、globalSetup/teardown 临时目录、ffmpeg 探测）、内置图片夹具、登录 e2e。验证链路打通 + 隔离不污染真实数据。
- **PR2：文件列表 / 搜索 / 视图切换 / 标签树**。给 `FileGrid.vue`/`Home.vue` 补 `data-testid`，写对应用例。
- **PR3：资源库扫描 + 缩略图懒加载**。轮询等待用例（扫描后文件出现、缩略图 404→200），含 ffmpeg skip 兜底。
- **PR4：spec 同步 + 文档**。更新 `frontend/quality-guidelines.md` 测试章节；README/journal 记录运行方式与 ffmpeg/隔离说明。

## Out of Scope (explicit)

- Vitest 组件/逻辑单测——拆为后续独立任务。
- **视频样本的缩略图测试**——MVP 仅用图片样本覆盖缩略图链路（视频样本体积大，且 ffmpeg 对图片的处理足以验证 worker/缓存/懒加载整条路径）。
- CI 接入（无现有 CI 基础设施）。
- 覆盖率门禁。
- 手动标签（add/remove file tag）等更细的 CRUD 交互 e2e。

## Research References

- 无外部 research：Playwright 已由用户选定；数据隔离/启动方案完全由 repo inspection（`infra/config.rs`、`main.rs`）确定，未触发 research-first。如后续需要 Playwright Page Object / fixture 复用等最佳实践，再补 `research/*.md`。

## Technical Notes

- 后端启动入口：`tagflow-core/src/main.rs`；配置 `infra/config.rs`；DB 初始化 `infra/db.rs`（WAL + foreign_keys + `migrate!`）。
- worker 入口 `engine/worker.rs:137`（5s 轮询）；缩略图生成 `infra/thumbnail.rs`（外部 ffmpeg）。
- 前端关键组件：`views/Login.vue`、`views/Home.vue`、`components/FileGrid.vue`；状态 `stores/auth.ts`、`stores/useResourceStore.ts`；HTTP `api/http.ts`。
- 已知坑（来自 spec）：`display:none`+`loading=lazy` 不加载（FileGrid 用 opacity）；受保护媒体须 `mediaUrl()` 拼 `?token=<jwt>`。
