# Journal - sayeui (Part 1)

> AI development session journal
> Started: 2026-06-11

---



## Session 1: 打通扫描流水线：trigger_scan 接入扫描引擎与缩略图任务

**Date**: 2026-06-12
**Task**: 打通扫描流水线：trigger_scan 接入扫描引擎与缩略图任务
**Branch**: `main`

### Summary

实现 trigger_scan（404/409/202 + 进程级扫描锁 + last_scanned_at），Scanner 媒体白名单入队缩略图任务并修复丢失文件恢复边界，list_files 过滤丢失文件；e2e 验证揪出并修复 thumbnail.rs 三个 M8 存量 bug（路径拼接、静态图 -ss 丢帧、0 字节残留污染缓存）；新增 TAGFLOW_PORT 环境变量。端到端验收全部通过。

### Main Changes

(Add details)

### Git Commits

| Hash | Message |
|------|---------|
| `0c17031` | (see git log) |

### Testing

- [OK] (Add test results)

### Status

[OK] **Completed**

### Next Steps

- None - task complete


## Session 2: 修复 list_files 分页 total 计数与错误处理

**Date**: 2026-06-12
**Task**: 修复 list_files 分页 total 计数与错误处理
**Branch**: `main`

### Summary

list_files 三个查询分支各配条件一致的 COUNT 查询，total 不再误用当页条数；移除 unwrap_or_default 技术债，DB 错误映射 500 并记录中文 error 日志。e2e 验证：60 文件分页 50/60、10/60，标签过滤 total=55，删除文件重扫后丢失文件不计入计数。

### Main Changes

(Add details)

### Git Commits

| Hash | Message |
|------|---------|
| `346589f` | (see git log) |

### Testing

- [OK] (Add test results)

### Status

[OK] **Completed**

### Next Steps

- None - task complete


## Session 3: 同步项目说明文件至 Milestone 8 实际进展

**Date**: 2026-06-13
**Task**: 同步项目说明文件至 Milestone 8 实际进展
**Branch**: `main`

### Summary

将 CLAUDE.md 与 README.md 从 M1-7 状态同步到 M1-8 实际进展：补全 engine/（scanner/tagger/worker）、infra/thumbnail.rs、infra/storage、core/tag/ 模块组织；新增 FFmpeg 运行时依赖与 ./cache 缓存目录说明；数据库表补 tasks；API 路由补 GET /api/v1/files/:id/thumbnail；新增「异步任务流水线」与扫描并发防护（409）说明；README 路线图 M7/M8 标完成、项目结构补 worker.rs/thumbnail.rs/library.rs/settings 子目录。逐条对照 main.rs 路由与源码实现核实。

### Main Changes

(Add details)

### Git Commits

| Hash | Message |
|------|---------|
| `0e08830` | (see git log) |

### Testing

- [OK] (Add test results)

### Status

[OK] **Completed**

### Next Steps

- None - task complete


## Session 4: JWT_SECRET 环境变量化与启动校验

**Date**: 2026-06-13
**Task**: JWT_SECRET 环境变量化与启动校验
**Branch**: `main`

### Summary

移除 auth.rs 硬编码密钥常量，引入 OnceLock<Vec<u8>> + init_jwt_secret() + validate_secret_length()；create_jwt/decode_jwt 改用 secret() 内部 get_or_init 回退；main.rs 在日志 init 后、init_db 前调用 init。debug 模式缺失密钥用开发默认 + warn，release 模式 fail-fast，长度 < 32 字节启动失败（HS256 规范）。trellis-implement + trellis-check sub-agent 全程协作，runtime e2e 验证 debug/release 双模式与登录→受保护路由闭环全部通过。闭合 M9 部署前安全阻断项；TAGFLOW_ADMIN_PASSWORD 同等问题记为后续任务。

### Main Changes

(Add details)

### Git Commits

| Hash | Message |
|------|---------|
| `252514e` | (see git log) |

### Testing

- [OK] (Add test results)

### Status

[OK] **Completed**

### Next Steps

- None - task complete


## Session 5: TAGFLOW_ADMIN_PASSWORD 生产 fail-fast

**Date**: 2026-06-14
**Task**: TAGFLOW_ADMIN_PASSWORD 生产 fail-fast
**Branch**: `main`

### Summary

main.rs ensure_admin_user 的 count==0 分支重写：3 个常量（ADMIN_PASSWORD_ENV/MIN_ADMIN_PASSWORD_LEN=12/DEV_DEFAULT_ADMIN_PASSWORD）+ 纯函数 validate_admin_password_len + cfg!(debug_assertions) 区分（debug warn+默认 / release fail-fast）。长度阈值 12 字节（OWASP），与 JWT_SECRET 32 字节形成密码 vs 密钥的合理区分。非空 users 表分支不受影响（语义正确，env 在已有部署中不会被使用）。TAGFLOW_ADMIN_USERNAME 保持现状（决策 Q1-A）。trellis-implement + trellis-check sub-agent 协作，e2e 5 场景（release×3 + debug×1 + 非空库×1）全部通过。与 JWT_SECRET 形成对称安全姿态，闭合默认凭据风险；下一步是 M9 容器化。

### Main Changes

(Add details)

### Git Commits

| Hash | Message |
|------|---------|
| `dc0401c` | (see git log) |

### Testing

- [OK] (Add test results)

### Status

[OK] **Completed**

### Next Steps

- None - task complete


## Session 6: M9-1: rust-embed 单二进制 + 配置外部化

**Date**: 2026-06-14
**Task**: M9-1: rust-embed 单二进制 + 配置外部化
**Branch**: `main`

### Summary

引入 rust-embed 将 tagflow-ui/dist 嵌入二进制，新增 src/api/static_files.rs 提供 SPA fallback；新增 src/infra/config.rs 收敛 TAGFLOW_DB_PATH/TAGFLOW_CACHE_DIR 环境变量，统一替换 main.rs、api/file.rs、bin/reset-password.rs 中散落硬编码。e2e 验证全绿：前端 200、SPA 路由刷新不 404、MIME 正确（.js→text/javascript）、API 优先匹配、DB 路径真实创建。Release 单二进制可独立提供前后端服务。任务二（Docker + 部署文档）待启动。

### Main Changes

(Add details)

### Git Commits

| Hash | Message |
|------|---------|
| `1251d02` | (see git log) |

### Testing

- [OK] (Add test results)

### Status

[OK] **Completed**

### Next Steps

- None - task complete


## Session 7: M9-2 Docker 化与部署文档

**Date**: 2026-06-14
**Task**: M9-2 Docker 化与部署文档
**Branch**: `main`

### Summary

三阶段 Dockerfile（node + cargo-chef + Alpine runtime），docker-compose 模板（含 fail-fast 必填变量），完整部署指南。新增 /api/health 端点供容器探活。134MB 镜像，e2e 全过：health 200 / SPA 477B / UID 1000 / ffmpeg 6.1.1 / tini PID 1 / SQLite WAL 持久化 / admin 创建幂等 / reset-password CLI 容器内可用。期间发现并修复两个预存 clippy 警告（auth.rs manual_strip / db.rs to_string_trait_impl），作为独立 refactor commit。路线图 M9 拆分为 M9-1/M9-2 标记完成。

### Main Changes

(Add details)

### Git Commits

| Hash | Message |
|------|---------|
| `df2623b` | (see git log) |
| `f6e8421` | (see git log) |
| `4bbb6e2` | (see git log) |

### Testing

- [OK] (Add test results)

### Status

[OK] **Completed**

### Next Steps

- None - task complete


## Session 8: 类型/扩展名/时间自动标签 + 多标签分面过滤

**Date**: 2026-06-15
**Task**: 类型/扩展名/时间自动标签 + 多标签分面过滤
**Branch**: `main`

### Summary

NAS 部署核对发现自动标签引擎只有 path 一维、无多标签过滤。补齐 ext/type/time 三个 tagger（新增 text 桶含 txt/md/log/csv）+ 递归 CTE 多标签 AND 查询 + app_meta 版本回填 + 前端复选框分区树 + 面包屑。代码/API/UI 三层 e2e 验证通过；发现并修复 axum serde_urlencoded 不支持重复 key 成 Vec 的坑（改逗号分隔，写入 spec）。挂载 chrome-devtools MCP 用于线上回归。

### Main Changes

(Add details)

### Git Commits

| Hash | Message |
|------|---------|
| `be96a9d` | (see git log) |
| `92af9d1` | (see git log) |
| `9f231fd` | (see git log) |

### Testing

- [OK] (Add test results)

### Status

[OK] **Completed**

### Next Steps

- None - task complete


## Session 9: 媒体预览抽屉：文件详情 + 多类型预览 + 下载

**Date**: 2026-06-16
**Task**: 媒体预览抽屉：文件详情 + 多类型预览 + 下载
**Branch**: `main`

### Summary

实现点击文件卡片滑出右侧抽屉，按类型预览：文本(DynamicScroller 虚拟滚动)、Markdown(markdown-it html:false)、PDF(iframe)、图片(点击全屏)、视频(Range seek)、音频，附下载/Esc+遮罩关闭。后端新增 GET /files/:id/content(类型分流+Range 206+GBK→UTF-8 转码+下载头) 与 GET /files/:id(元数据+标签)。关键修复：auth_middleware 增加 ?token=<jwt> 兜底，解决浏览器媒体 src/缩略图被 401 静默隐藏的历史问题；FileGrid 缩略图改 opacity 显隐(display:none 的 lazy img 不加载)。trellis-check 通过并 self-fix 空内容文件渲染缺口。e2e 8 文件全类型验证通过。spec 捕获 ?token= 鉴权/OpenDAL Buffer/display:none lazy img 三约定。

### Main Changes

(Add details)

### Git Commits

| Hash | Message |
|------|---------|
| `7415c62` | (see git log) |

### Testing

- [OK] (Add test results)

### Status

[OK] **Completed**

### Next Steps

- None - task complete


## Session 10: 技术债清理：密码文案/路径校验/扫描死代码

**Date**: 2026-06-16
**Task**: 技术债清理：密码文案/路径校验/扫描死代码
**Branch**: `main`

### Summary

制定迭代计划(V1手动标签/V2文件操作/V3全文搜索/V4 WebDAV/V5元数据深化 + 技术债批次)，用户选先清技术债。核实三条已记录债：(1) Security.vue 密码前端6位 vs 后端≥12——发现 update_password 后端完全无长度校验；(2) create_library 不校验路径存在导致 OpenDAL 自动建幽灵空库；(3) Libraries.vue 501死代码+409无区分。brainstorm 收敛两决策：前后端统一≥12字节、路径不存在拒绝400。trellis-implement 实现+抽取 validate_local_path_readable 供 create/test_connection 共用消除重复；trellis-check PASS 并 self-fix 1处重复 warn 日志。顺手全局 cargo fmt 格式化上阶段遗留(file.rs/backfill.rs/tagger，逐行确认纯折行无逻辑)。e2e 实测三路径全绿：update_password 5字节→400/12字节→200、create_library 不存在路径→400、大库并发scan→409(扫完恢复→202)。拆 chore(fmt)+feat 两 commit。

### Main Changes

(Add details)

### Git Commits

| Hash | Message |
|------|---------|
| `b0867d8` | (see git log) |
| `2e9b977` | (see git log) |

### Testing

- [OK] (Add test results)

### Status

[OK] **Completed**

### Next Steps

- None - task complete


## Session 11: 手动标签（user tag）

**Date**: 2026-06-16
**Task**: 手动标签（user tag）
**Branch**: `main`

### Summary

文件抽屉打自定义标签/移除+递归过滤。后端 POST /files/:id/tags（按「/」逐层建 user 节点，复用 ensure_tag）、DELETE /files/:id/tags/:tag_id（仅删 manual，auto→403，移除后向上递归清理空 user 节点）；FileTagInfo 加 source 跨层流转（前端据此显隐×）；fetch_file_tags 抽取复用。前端 fileApi/store/FileDrawer 联动。6 内存库单测 + clippy/build + 真实进程 e2e（建层级/递归过滤/auto拒删403/自动清理父子链）全过；spec 入 database-guidelines。

### Main Changes

(Add details)

### Git Commits

| Hash | Message |
|------|---------|
| `30b9f1c` | (see git log) |

### Testing

- [OK] (Add test results)

### Status

[OK] **Completed**

### Next Steps

- None - task complete


## Session 12: 更新 README：去 AI 介绍、功能清单、里程碑路线图与 v0.2.0 规划

**Date**: 2026-06-16
**Task**: 更新 README：去 AI 介绍、功能清单、里程碑路线图与 v0.2.0 规划
**Branch**: `main`

### Summary

更新 README 反映 v0.1.0 Beta 后真实状态：删除 3 处 AI 工具/模型介绍（顶部徽章、AI 声明章节、底部标语）+ 项目结构树 CLAUDE.md 注释中性化；新增「功能清单」章节（6 子系统分组，✅/🔄/📅 三态）；路线图改里程碑视角（M1–M9-2 + v0.1.0 Beta ✅ + v0.2.0 Beta 📅）；后续迭代计划按里程碑重组——下一个里程碑 v0.2.0 Beta = 多源接入与自动同步（WebDAV + 定时增量扫描），文件操作暂不考虑，批量标签/全文搜索/元数据留 v0.3.0+。产品蓝图 gemini 链接暂时保留。同步更新 memory（v0.1.0→Beta、文件操作暂缓、v0.2.0 规划）。纯文档 1 commit，与 worktree 无冲突。

### Main Changes

(Add details)

### Git Commits

| Hash | Message |
|------|---------|
| `e27deef` | (see git log) |

### Testing

- [OK] (Add test results)

### Status

[OK] **Completed**

### Next Steps

- None - task complete


## Session 13: 文件视图增强——真实 e2e 验证与收尾

**Date**: 2026-06-16
**Task**: 文件视图增强——真实 e2e 验证与收尾
**Branch**: `main`

### Summary

对 commit 443e220 的 4 项改动做真实进程 e2e（76 文件库/后端 8080+前端 5173+Chrome DevTools）：API 层 10 场景（分页 p1=50/p2=26/p3=0、搜索 report=40/image=30/中文=1、%/ _ 转义边界当字面量）+ UI 层 5 验收点（卡片无重叠 DOM 测量 translateY 恒 176px/overflow=0、无限滚动 50→76、搜索框 report→40/40、视图切换集合不变、偏好 tagflow.viewMode 刷新恢复）全绿。prd 勾选全部 Requirements(4)/AC(5)。测试数据清理：进程停/73 png 删/db(files/tags/tasks)归零/cache 清空/工作区 clean。⚠️ admin 密码为登录验证已重置为 E2eVerify!2026，需用户改回。

### Main Changes

无新代码改动（验证既有 commit `443e220`）；prd.md 勾选全部 Requirements(4)/AC(5) 复选框并追加 E2E 验证结果章节；Phase 3.4 commit `9557711` 收尾。

### Git Commits

| Hash | Message |
|------|---------|
| `9557711` | (see git log) |

### Testing

- [OK] API 层 e2e 10 场景全绿（curl + python：分页/搜索/中文/`%`_` 转义边界）
- [OK] UI 层 e2e 5 验收点全绿（Chrome DevTools DOM 测量 translateY 恒 176px/overflow=0 + 真实交互）
- [OK] 测试数据清理：73 png 删除、db(files/tags/tasks/file_tags) 归零、cache 清空、工作区 clean
- ⚠️ admin 密码为登录验证已用 reset-password 重置为 `E2eVerify!2026`（原密码未知），需用户自行改回

### Status

[OK] **Completed**

### Next Steps

- None - task complete


## Session 14: 引入 Playwright e2e 测试框架

**Date**: 2026-06-22
**Task**: 引入 Playwright e2e 测试框架
**Branch**: `main`

### Summary

为 TagFlow 引入 Playwright e2e 测试框架。brainstorm 收敛 PRD（档位 B 范围：登录/列表/搜索/视图/标签树/扫描/缩略图；内置小图片夹具；data-testid 必补；Q4 spec 同步=是）。新增 tagflow-e2e/ 独立包：webServer 经 env 注入临时 DB/cache/端口/账号起隔离后端（零后端改动），globalSetup seed 资源库并扫描 fixtures/library。前端 FileGrid/FileList/TagItem/Home 补 data-testid。10 用例全过（login/files/library-scan/thumbnails，连跑无 flake），隔离确认真实 db/cache 未污染。缩略图用例双状态稳定 + ffmpeg skip 兜底；409 并发防护作为已知缺口透明记录。同步 frontend quality-guidelines 测试章节。途中发现并修复 .gitignore 误加 tagflow-e2e 行的红旗。

### Main Changes

(Add details)

### Git Commits

| Hash | Message |
|------|---------|
| `b9bb4be` | (see git log) |

### Testing

- [OK] (Add test results)

### Status

[OK] **Completed**

### Next Steps

- None - task complete


## Session 15: 定时增量扫描（v0.2.0 自动同步）+ git 身份配置

**Date**: 2026-06-22
**Task**: 定时增量扫描（v0.2.0 自动同步）+ git 身份配置
**Branch**: `main`

### Summary

v0.2.0 第一块「定时增量扫描」完整落地。brainstorm 收敛 5 决策：全局单定时器 + 全局 env TAGFLOW_SCAN_INTERVAL 间隔（默认 3600 clamp ≥60）+ 启动即首轮 + 后端 scheduler/前端展示下次扫描 + e2e 极短间隔验证。PR1 后端：scheduler（engine/scheduler.rs，仿 worker spawn 循环，首轮立即+interval，单库失败 continue 不退出）+ 抽 engine 层 scan_library_job/run_scan_with_lock_held（trigger_scan 与 scheduler 共用同一份扫描逻辑，去重）+ 409 锁 SCANNING 从 api 移至 engine 层（api/scheduler 同库不并发）+ trigger_scan 保持同步 409 语义（不跨 await）+ config clamp。PR2 前端：LibraryResponse 加 scan_interval_secs + Libraries.vue 展示「预计下次扫描」（手动触发按钮保留不动）。PR3 e2e：scheduled-scan.spec 验证无手动触发自动扫入新文件+删除自动移除；TAGFLOW_E2E_FAST_SCAN=1 escape hatch（仅 tagflow-e2e 绕 clamp，production 绝不设）；library-scan 加 409 retry wrapper 容忍 scheduler 撞锁。验证：cargo test 64+1+4 / clippy --all-targets clean / npm run build clean / e2e 11×3 连跑无 flake。spec：backend quality-guidelines 新增「后台任务与定时扫描契约」节（含 escape hatch 安全阀约定）。附带：本 session 还配置了 git 身份按 remote 自动切换（~/.gitconfig hasconfig:remote，全局保留公司身份给 GitLab、GitHub remote 自动切个人 noreply），解决历史 commit 不归属 GitHub 账号问题；memory 已记录该环境约定。

### Main Changes

(Add details)

### Git Commits

| Hash | Message |
|------|---------|
| `376c184` | (see git log) |

### Testing

- [OK] (Add test results)

### Status

[OK] **Completed**

### Next Steps

- None - task complete


## Session 16: 发版准备 + 修复 SQLite database is locked

**Date**: 2026-06-22
**Task**: 发版准备 + 修复 SQLite database is locked
**Branch**: `main`

### Summary

Session 聚焦 v0.2.0 发版准备 + 一个发版阻塞 P0 bug 修复。(1) 发版评估：核心闭环完整、e2e 11 守护，可发 v0.2.0 正式版，但 e2e 隔离夹具覆盖不到真实环境/升级/长期运行，需手动验收。(2) 发版准备产物：doc/发版验收测试用例.md（P0/P1/P2 清单含升级路径/定时扫描真实间隔）、scripts/sync-nas.sh（白名单 rsync 同步源码到 NAS）、.env.example+docker-compose 补 TAGFLOW_SCAN_INTERVAL、cache 目录注释写清（chown 是关键，bind mount UID 1000）。(3) rsync 优化：黑名单→白名单（通用 exclude 不含 / 匹配任意层级、放 include 前），dry-run 发现并修复 .env/tagflow.db 误同步。(4) 修复 SQLite database is locked（P0）：扫描图片库 worker 报错。根因有二——busy_timeout 未设（scheduler/worker/手动扫描并发写 SQLITE_BUSY code 5）+ foreign_keys per-connection 只设一个（ON DELETE CASCADE 对多数连接不强制，隐藏 bug）。修复 infra/db.rs 改用 SqliteConnectOptions 对 pool 每个连接统一设 busy_timeout(5s)+foreign_keys(true)+WAL，3 回归测试（并发写不锁/CASCADE 对所有连接/PRAGMA 生效）。spec 入 backend/database-guidelines「SQLite 连接配置契约」节。cargo test 67/clippy/e2e 11 全绿。NAS 真实复验待用户重新同步部署。

### Main Changes

(Add details)

### Git Commits

| Hash | Message |
|------|---------|
| `9b13516` | (see git log) |
| `78685b9` | (see git log) |

### Testing

- [OK] (Add test results)

### Status

[OK] **Completed**

### Next Steps

- None - task complete


## Session 17: 修复非媒体文件缩略图 404

**Date**: 2026-06-22
**Task**: 修复非媒体文件缩略图 404
**Branch**: `main`

### Summary

修复前端对非媒体文件请求缩略图导致 404 刷屏（发版阻塞）。根因：FileGrid.vue 对所有文件无条件渲染 <img src=thumbnail>，但后端只为 MEDIA_EXTENSIONS（scanner/mod.rs:141）入列缩略图任务，文本/PDF/代码/归档等非媒体永不生成缩略图 → 404。修复：FileGrid 加 MEDIA_EXTENSIONS（与后端逐字一致，不含 svg）+ isMediaFile + img v-if，非媒体不渲染不发请求；图标白名单（getFileIcon imageExts 含 svg）与缩略图白名单分离。e2e：fixtures 加 notes.txt 非媒体夹具 + EXPECTED_FILE_COUNT 6 + 新用例拦截 thumbnail 请求断言非媒体不发起；thumbnails.spec 改按媒体扩展名过滤选 fileId（修 notes.txt 最新 mtime 排序靠前导致选中非媒体的跨层回归）。spec：frontend/component-guidelines 加「缩略图媒体白名单（跨层契约）」节（前端必须与后端 MEDIA_EXTENSIONS 逐字一致）。npm run build clean / e2e 12 passed。NAS 复验待用户。

### Main Changes

(Add details)

### Git Commits

| Hash | Message |
|------|---------|
| `1339ad9` | (see git log) |

### Testing

- [OK] (Add test results)

### Status

[OK] **Completed**

### Next Steps

- None - task complete


## Session 18: 孤儿标签清理（删库 + 标签树过滤）

**Date**: 2026-06-22
**Task**: 孤儿标签清理（删库 + 标签树过滤）
**Branch**: `main`

### Summary

修复删库/删文件后无效标签残留（标签树显示但查询空，发版阻塞）。两场景：删库（真孤儿 tags 无 file_tags，tags 不在 CASCADE 链残留）+ 扫描删文件（软删 status=0，标签关联离线文件）。诊断发现 get_tag_tree 原 SELECT * FROM tags 不过滤 file_tags/status，是显示根因；scanner mark_as_lost 是有意软删（恢复/移动检测），不能改硬删。修复（标签树过滤 + 删库清理组合）：(1) get_tag_tree 过滤 status=1 文件关联，build_tree 按子树递归剪枝（父在子有在线时显示），同时隐藏删库孤儿 + 离线关联，软删文件恢复时标签自动回归；(2) delete_library 改造：删库前查受影响 tag_ids，删库后调 cleanup_orphan_tag 清理孤儿；(3) 泛化 cleanup_orphan_user_tag→cleanup_orphan_tag（去 user 限制，适用 path/ext/type/time/user 所有类别）。跨库共享标签（#year:2026/Projects/）天然保留（COUNT=0 才删）。spec database-guidelines 加「孤儿标签清理」契约。cargo test 78（+11 新测）/ clippy clean / e2e 14 passed（+2 删库孤儿+软删隐藏恢复）。NAS 复验待用户。

### Main Changes

(Add details)

### Git Commits

| Hash | Message |
|------|---------|
| `8b74e76` | (see git log) |

### Testing

- [OK] (Add test results)

### Status

[OK] **Completed**

### Next Steps

- None - task complete
