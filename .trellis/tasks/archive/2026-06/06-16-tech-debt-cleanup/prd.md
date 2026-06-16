# 技术债清理：密码文案/路径校验/扫描死代码

## Goal

清理三处已核实、影响真实体验/定位一致性的技术债。系统已闭环可用（M1–M9 完成），这是迭代计划「先清技术债」批次，为后续 V1 手动标签等正式迭代做收尾打磨。

## What I already know（已核实）

### 债1：密码前端校验与项目安全标准不一致
- 前端 `tagflow-ui/src/views/settings/Security.vue`：
  - L32 `'新密码长度至少为 6 位'`（validate 错误消息）
  - L136 `minlength="6"`
  - L138 `placeholder="请输入新密码（至少 6 位）"`
  - L140 `<p>密码长度至少为 6 位</p>`（hint 文案）
- 后端 `tagflow-core/src/api/auth.rs` `update_password`（L196-228）：**无任何长度校验**，直接 `hash_password` + UPDATE。前端卡 6 位，后端不卡，可被绕过。
- 项目安全标准：`TAGFLOW_ADMIN_PASSWORD` 要求 ≥12 字节（M9 前的 fail-fast 任务确立）。这是项目的密码门槛意图，但 update_password 未对齐。

### 债2：create_library 不校验路径存在
- `tagflow-core/src/api/library.rs` `create_library`（L106-）：仅调 `validate_path_security`（路径遍历/绝对路径），**不校验 base_path 是否存在**。
- `test_connection`（L208+）已有完整的 `path.exists()` + 是目录 + 可读校验逻辑，可复用。
- 现状后果：用户填错路径 → 库创建成功 → 扫描时 OpenDAL Fs **自动创建该目录并以 0 文件成功扫描**，产生"幽灵空库"，与"非侵入式"定位有张力。

### 债3：Libraries.vue 扫描错误分支死代码 + 409 无区分
- `tagflow-ui/src/views/settings/Libraries.vue` `triggerScan`（L194-205）：
  - L199 `if (error.response?.status === 501)` → `'扫描功能尚未实现'`：**死代码**。扫描接口只返 200/409/500，永不返 501。
  - 409（同库扫描进行中）走通用 `'启动扫描失败'` 文案，无区分，用户看不到"正在扫描中"的提示。

## Decision (ADR-lite)

**决策1 — 密码门槛**：前后端统一 ≥12 字节。前端 Security.vue 校验/文案 4 处改 12；后端 update_password 补 ≥12 校验（防前端绕过，对齐项目 ADMIN_PASSWORD 标准）。
- Context：前端卡 6 位、后端不卡，且项目安全标准（ADMIN_PASSWORD）已是 ≥12，存在不一致与绕过风险。
- Consequences：已认证用户改密码需 ≥12 位，更安全；后端 +~5 行校验 + 单测。

**决策2 — 路径校验**：create_library 检测到 base_path 不存在/非目录/不可读时拒绝创建（400 + 明确提示）。
- Context：OpenDAL Fs 会自动建目录，导致错填路径产生"幽灵空库"，与"非侵入式"定位有张力。
- Consequences：杜绝幽灵库；复用 test_connection 的 exists/可读校验逻辑；用户须填真实存在的目录。

## Requirements

- 债1：前端 Security.vue 密码校验/文案统一为 ≥12 位（4 处）；后端 update_password 补 ≥12 字节校验（返 4xx）。
- 债2：create_library 增加 base_path 存在性 + 是目录 + 可读校验（复用 test_connection 逻辑），不存在则拒绝创建（400）。
- 债3：删除 Libraries.vue triggerScan 的 501 死代码分支；409 显示"该资源库正在扫描中，请稍后再试"。

## Acceptance Criteria（evolving）

- [ ] Security.vue 密码相关 4 处文案/校验对齐 ≥12
- [ ] update_password 对 <12 字节新密码返回 4xx，单测覆盖
- [ ] create_library 对不存在/非目录/不可读路径返回 400 + 提示，单测覆盖
- [ ] Libraries.vue 不再有 501 死代码分支
- [ ] 扫描冲突（409）显示明确的"扫描中"提示
- [ ] cargo test / clippy 绿；前端 typecheck 绿
- [ ] e2e：改短密码被拦、创建不存在路径库被拒、重复触发扫描见 409 提示

## Definition of Done

- 后端单测覆盖 create_library 新增校验 + update_password 长度校验（若加）
- lint / typecheck / test 全绿
- 行为变更无需改文档（内部一致性修正）
- e2e 真实验证三条修复路径

## Out of Scope

- WebDAV 资源库实现（V4 迭代）
- 密码强度评分（仅长度门槛）
- 文件卡片点击等其它非本批次发现

## Technical Notes

- `tagflow-ui/src/views/settings/Security.vue` L32/136/138/140
- `tagflow-core/src/api/auth.rs` update_password L196-228
- `tagflow-core/src/api/library.rs` create_library L106-、test_connection L208-（可复用 exists/可读校验）
- `tagflow-ui/src/views/settings/Libraries.vue` triggerScan L194-205
- 项目密码门槛来源：`06-13-tagflow-admin-password-fail-fast`（ADMIN_PASSWORD ≥12 字节）
