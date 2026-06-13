# JWT_SECRET 环境变量化与启动校验

## Goal

将 `tagflow-core/src/core/auth.rs:15` 硬编码的 `JWT_SECRET` 常量改造为运行时可配置项，并在启动阶段进行安全校验。这是 Milestone 9（部署、容器化与产品化）的硬阻断项——单二进制部署前必须先闭合密钥外部化这一安全口。

## What I already know

- 当前实现：`const JWT_SECRET: &[u8] = b"your_ultra_secret_key_change_in_production";`（auth.rs:15）
- 调用点仅 2 处：`create_jwt` (auth.rs:111) 与 `decode_jwt` (auth.rs:135)
- `auth_middleware`（api/auth.rs:95）间接调用 `decode_jwt`
- 项目现有 env 模式（main.rs）：
  - `TAGFLOW_PORT` —— `var().parse()` 失败回退默认值 + warn
  - `TAGFLOW_ADMIN_USERNAME` / `TAGFLOW_ADMIN_PASSWORD` —— 缺失回退默认值 + 控制台打印
- 测试：`test_jwt_create_and_decode`（auth.rs:154-160）直接调用 `create_jwt/decode_jwt`
- 用户共识：M9 部署前阻断项（来源：[[tagflow-roadmap-priorities]]）

## Requirements

1. 移除 `auth.rs:15` 的硬编码 `JWT_SECRET` 常量
2. 通过环境变量 `TAGFLOW_JWT_SECRET` 提供密钥
3. 使用 `std::sync::OnceLock<Vec<u8>>` 在首次访问时缓存密钥，保持 `create_jwt/decode_jwt` 函数签名不变
4. 启动阶段在 `main.rs` 调用初始化函数执行安全校验
5. 按环境区分行为（决策 C，触发条件 `cfg!(debug_assertions)`）：
   - debug build（开发模式）：缺失时使用开发默认值并 `warn!` 日志
   - release build（生产模式）：缺失时返回 `Err`，main 透传错误使进程退出
6. 密钥长度校验（决策 C）：非空且 `< 32` 字节启动失败，错误信息明确提示 HS256 推荐长度
7. 文档同步：CLAUDE.md「运行时配置」、README 部署说明均补全 `TAGFLOW_JWT_SECRET`

## Acceptance Criteria

- [ ] `cargo build` (debug) 不设置 `TAGFLOW_JWT_SECRET` 能正常启动，控制台出现 warn 日志
- [ ] `cargo build --release` 不设置 `TAGFLOW_JWT_SECRET` 启动失败，错误信息明确
- [ ] `cargo build --release` 设置短于 32 字节的 `TAGFLOW_JWT_SECRET` 启动失败
- [ ] `TAGFLOW_JWT_SECRET=<32+ bytes>` 启动后登录 → 携带 token 请求受保护路由 → 成功
- [ ] `cargo test` 全绿（`test_jwt_create_and_decode` 适配新初始化路径）
- [ ] `cargo clippy` 无新警告
- [ ] CLAUDE.md / README 文档已更新环境变量说明

## Definition of Done

- 单元测试覆盖（含密钥外部化路径与开发默认值分支）
- 真实 e2e 验证：dev/release 两种构建分别验证启动行为；release 模式下完整登录→请求闭环
- 文档说明生产部署必须设置 `TAGFLOW_JWT_SECRET`（≥ 32 字节）
- Rollout 提示：密钥更换将使已签发的 token 全部失效，用户需重新登录

## Technical Approach

**核心设计**：OnceLock + 启动期初始化，保持调用点零改动。

```rust
// auth.rs
use std::sync::OnceLock;

static JWT_SECRET: OnceLock<Vec<u8>> = OnceLock::new();

const DEV_DEFAULT_SECRET: &[u8] = b"tagflow_dev_only_secret_do_not_use_in_production_32b";

pub fn init_jwt_secret() -> Result<()> {
    let secret = match std::env::var("TAGFLOW_JWT_SECRET") {
        Ok(s) if !s.is_empty() => s.into_bytes(),
        _ => {
            if cfg!(debug_assertions) {
                warn!("TAGFLOW_JWT_SECRET 未设置，使用开发默认密钥（仅 debug 构建可用）");
                DEV_DEFAULT_SECRET.to_vec()
            } else {
                return Err(anyhow!("生产模式必须设置 TAGFLOW_JWT_SECRET 环境变量（≥ 32 字节）"));
            }
        }
    };
    if secret.len() < 32 {
        return Err(anyhow!(
            "TAGFLOW_JWT_SECRET 长度 {} < 32 字节，不满足 HS256 安全要求",
            secret.len()
        ));
    }
    JWT_SECRET.set(secret).map_err(|_| anyhow!("JWT_SECRET 已初始化"))?;
    Ok(())
}

fn secret() -> &'static [u8] {
    JWT_SECRET.get().expect("JWT_SECRET 未初始化，请确保 main 已调用 init_jwt_secret")
}
```

- `create_jwt` / `decode_jwt` 将 `EncodingKey::from_secret(JWT_SECRET)` 改为 `EncodingKey::from_secret(secret())`
- `main.rs` 在 `init` 日志后、`init_db` 前调用 `core::auth::init_jwt_secret()?`
- 单元测试通过 `OnceLock::get_or_init` 或在 `#[test]` 顶部调用 `init_jwt_secret().ok();` 适配

## Decision (ADR-lite)

**Context**: JWT 密钥是安全敏感项，但项目本地开发零配置体验需保留。

**Decision**:
1. 缺失行为：按 `cfg!(debug_assertions)` 区分——debug 用开发默认值 + warn，release fail-fast
2. 长度校验：≥ 32 字节（HS256 规范），不满足启动失败
3. 注入方式：`OnceLock<Vec<u8>>`，保持 `create_jwt/decode_jwt` 签名不变

**Consequences**:
- 优点：零额外 env 配置；开发体验不退化；生产部署绝不带默认密钥上线；HS256 密钥强度有保证
- 风险：release build 跑开发场景需手动 set env；密钥更换使已签发 token 全部失效
- 后续可演进：如需更细粒度控制，未来可叠加 `TAGFLOW_ENV` 显式覆盖

## Out of Scope

- JWT 密钥轮换 / 多密钥支持
- 引入 KMS / Vault 等外部密钥管理
- 配置文件（YAML/TOML）支持
- 重构 `create_jwt` / `decode_jwt` 函数签名
- `TAGFLOW_ADMIN_PASSWORD` 的同等生产 fail-fast（建议另起任务跟进）

## Technical Notes

- 文件：`tagflow-core/src/core/auth.rs`、`tagflow-core/src/main.rs`
- 现有 env 模式参考：main.rs:97-103（TAGFLOW_PORT）、main.rs:125-128（admin 凭据）
- OnceLock 模式可保持函数签名稳定，同时避免每次 JWT 调用都执行 `env::var`
- 单元测试运行在 debug build，会用开发默认值，无需改测试逻辑（仅在 `create_jwt/decode_jwt` 调用前确保 init 已触发）

## Implementation Plan

- PR1（本任务单 PR）：auth.rs 改造 + main.rs 启动初始化 + 测试适配 + 文档更新
  - 一次性合并，因改动耦合（删除 const 后必须同步加 OnceLock，否则编译失败）
