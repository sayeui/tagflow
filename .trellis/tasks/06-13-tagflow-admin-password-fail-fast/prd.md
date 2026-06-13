# TAGFLOW_ADMIN_PASSWORD 生产 fail-fast

## Goal

将 `tagflow-core/src/main.rs:121-153` `ensure_admin_user` 中硬编码的默认管理员密码 `PhVENfYaWv`（main.rs:132）改为生产 fail-fast。这是 JWT_SECRET 任务的姊妹项——闭合 TagFlow 部署侧剩余的"默认凭据上生产"风险点，让 M9 容器化前所有凭据外部化工作全部完成。

## What I already know

- 当前实现（main.rs:121-153）：
  - 仅在 `users` 表为空（count == 0）时创建管理员
  - `TAGFLOW_ADMIN_USERNAME` 缺失默认 `"admin"`（main.rs:130）
  - `TAGFLOW_ADMIN_PASSWORD` 缺失默认 `"PhVENfYaWv"`（main.rs:132）
  - 创建后密码以 Argon2 hash 存入数据库，后续启动不再读 env
- 与 JWT_SECRET 的关键语义差异：
  - JWT_SECRET 每次启动都使用 → 启动无条件 fail-fast
  - ADMIN_PASSWORD 仅空表初始化时用一次 → 应在「需创建」分支内校验
- 上一任务已落地模式（参考 `auth.rs`）：`cfg!(debug_assertions)` 区分 + 长度校验 + anyhow context + 中文日志

## Requirements

1. 移除 main.rs:132 的硬编码默认密码常量
2. 在 `ensure_admin_user` 的 `count == 0` 分支内：
   - debug 模式：缺失 `TAGFLOW_ADMIN_PASSWORD` 用开发默认值（`"tagflow_dev_only_admin_pw"`，明显标识非生产）+ `warn!`
   - release 模式：缺失 → 返回 `Err`，main 透传使进程退出
3. 密码长度校验：低于 12 字节（OWASP 推荐）→ 返回 `Err`，错误信息明确
4. `TAGFLOW_ADMIN_USERNAME` 保持现有 `unwrap_or_else("admin")` 行为（决策 Q1-A）
5. 非空 users 表（已有管理员）：完全跳过校验逻辑，保持现状（env 不会被使用）
6. 文档同步 CLAUDE.md「运行时配置」

## Acceptance Criteria

- [ ] release + 空库 + 未设 `TAGFLOW_ADMIN_PASSWORD` → 启动失败，错误明确
- [ ] release + 空库 + 短密码（< 12 字节）→ 启动失败
- [ ] release + 空库 + 合法密码（≥ 12 字节）→ 创建 admin 成功，可用该密码登录
- [ ] release + 非空库 + 未设 env → 启动正常（不校验，因为不创建）
- [ ] debug + 空库 + 未设 env → warn + 用开发默认密码创建 admin
- [ ] `cargo test` / `cargo clippy` 全绿
- [ ] CLAUDE.md 更新

## Definition of Done

- 单元测试覆盖（密码校验函数纯化为 `validate_admin_password_len`）
- 真实 e2e 验证：release 空库三种场景 + debug 模式 + 已有用户场景（隔离 /tmp 工作目录，事后清理）
- 文档说明生产部署首次启动必须设置 `TAGFLOW_ADMIN_PASSWORD`
- Rollout 提示：本变更不影响已有部署（非空库不触发校验）

## Technical Approach

**核心设计**：复用 JWT_SECRET 模式，但校验位置在 `count==0` 分支内（与 JWT_SECRET 启动无条件 fail-fast 不同）。

```rust
// 在 ensure_admin_user 函数内，count == 0 分支
let admin_password = match std::env::var("TAGFLOW_ADMIN_PASSWORD") {
    Ok(s) if !s.is_empty() => {
        validate_admin_password_len(s.len())?;
        s
    }
    _ => {
        if cfg!(debug_assertions) {
            warn!("TAGFLOW_ADMIN_PASSWORD 未设置，使用开发默认密码（仅 debug 构建可用）");
            "tagflow_dev_only_admin_pw".to_string()
        } else {
            return Err(anyhow!("生产模式首次启动必须设置 TAGFLOW_ADMIN_PASSWORD 环境变量（≥ 12 字节）"));
        }
    }
};
```

- `validate_admin_password_len(len: usize) -> Result<()>` 抽为纯函数，便于单元测试
- 不动 `TAGFLOW_ADMIN_USERNAME` 行为
- 常量建议：`const MIN_ADMIN_PASSWORD_LEN: usize = 12;`、`const DEV_DEFAULT_ADMIN_PASSWORD: &str = "tagflow_dev_only_admin_pw";`、`const ADMIN_PASSWORD_ENV: &str = "TAGFLOW_ADMIN_PASSWORD";`

## Decision (ADR-lite)

**Context**: JWT_SECRET 任务闭合了"默认密钥"风险，但 `TAGFLOW_ADMIN_PASSWORD` 默认值 `PhVENfYaWv` 仍是同等量级的部署安全风险。

**Decision**:
1. 复用 JWT_SECRET 的「debug 默认+warn / release fail-fast + 长度校验」模式
2. 校验位置：`ensure_admin_user` 的 `count==0` 分支内（语义正确，非空库不受影响）
3. 范围：仅 password，username 保持现状（决策 Q1-A）
4. 长度阈值：12 字节（OWASP 推荐，决策 Q2-A）

**Consequences**:
- 优点：闭合最后一个默认凭据风险；非空库零影响；与 JWT_SECRET 形成对称安全姿态
- 风险：首次部署必须显式设 env（与 JWT_SECRET 一致的部署要求）
- 后续可演进：若引入多用户/角色管理，校验函数可直接复用

## Out of Scope

- 已存在用户的密码迁移 / 强制重置
- 多用户管理 UI
- 密码强度算法评估（zxcvbn 等）
- `reset-password` bin 工具改造（已有独立路径）
- JWT_SECRET 相关变更（已完成）
- `TAGFLOW_ADMIN_USERNAME` fail-fast（保持现状）

## Technical Notes

- 文件：`tagflow-core/src/main.rs`（核心改动）、`CLAUDE.md`（文档）
- 参考实现：上一任务的 `auth.rs::init_jwt_secret` / `validate_secret_length` 模式
- 关键差异：admin_password 校验**只在 count==0 分支内**触发，避免对已有用户的部署造成无意义失败
- 复用模式：`cfg!(debug_assertions)` + anyhow Result + tracing 中文日志
- 单元测试运行在 debug build，复用上任务经验

## Implementation Plan

- PR1（本任务单 PR）：main.rs 改造 + 纯函数 + 单元测试 + CLAUDE.md 更新
  - 一次性合并，因 main.rs `ensure_admin_user` 内聚，逻辑改动需要整体替换
