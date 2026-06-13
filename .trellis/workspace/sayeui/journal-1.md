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
