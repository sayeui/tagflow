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
