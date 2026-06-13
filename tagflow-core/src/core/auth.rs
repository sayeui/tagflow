//! 认证模块
//!
//! 提供密码哈希（Argon2）和 JWT 令牌管理功能。

use anyhow::{Result, anyhow};
use argon2::{
    Argon2,
    password_hash::{PasswordHash, PasswordHasher, PasswordVerifier, SaltString, rand_core::OsRng},
};
use chrono::{Duration, Utc};
use jsonwebtoken::{DecodingKey, EncodingKey, Header, Validation, decode, encode};
use serde::{Deserialize, Serialize};
use std::sync::OnceLock;
use tracing::warn;

/// JWT 密钥环境变量名
const JWT_SECRET_ENV: &str = "TAGFLOW_JWT_SECRET";

/// HS256 推荐最小密钥长度
const MIN_SECRET_LEN: usize = 32;

/// 开发默认密钥（仅在 debug 构建且未设置环境变量时使用）
///
/// 长度 ≥ 32 字节，满足 HS256 校验；生产环境必须通过 `TAGFLOW_JWT_SECRET` 覆盖。
const DEV_DEFAULT_SECRET: &[u8] = b"tagflow_dev_only_secret_do_not_use_in_production_32b";

/// 运行时缓存的 JWT 密钥
///
/// 由 [`init_jwt_secret`] 在启动阶段写入；测试或未显式初始化的边缘调用通过
/// [`secret`] 内的 `get_or_init` 回退到 [`DEV_DEFAULT_SECRET`]。
static JWT_SECRET: OnceLock<Vec<u8>> = OnceLock::new();

/// 校验密钥长度是否满足 HS256 安全要求
fn validate_secret_length(len: usize) -> Result<()> {
    if len < MIN_SECRET_LEN {
        return Err(anyhow!(
            "TAGFLOW_JWT_SECRET 长度 {} < {} 字节，不满足 HS256 安全要求",
            len,
            MIN_SECRET_LEN
        ));
    }
    Ok(())
}

/// 启动期初始化 JWT 密钥
///
/// 读取环境变量 `TAGFLOW_JWT_SECRET`：
/// - 非空字符串：按字节内容使用
/// - 空或缺失：
///     - debug 构建：回退到 [`DEV_DEFAULT_SECRET`] 并打印 `warn!`
///     - release 构建：返回 `Err`，调用方应使进程退出
///
/// 长度校验统一通过 [`validate_secret_length`]，无论密钥来源。
///
/// 重复调用返回 `Err`（OnceLock 已写入），调用方可在测试中忽略。
pub fn init_jwt_secret() -> Result<()> {
    let secret = match std::env::var(JWT_SECRET_ENV) {
        Ok(s) if !s.is_empty() => s.into_bytes(),
        _ => {
            if cfg!(debug_assertions) {
                warn!("TAGFLOW_JWT_SECRET 未设置，使用开发默认密钥（仅 debug 构建可用）");
                DEV_DEFAULT_SECRET.to_vec()
            } else {
                return Err(anyhow!(
                    "生产模式必须设置 TAGFLOW_JWT_SECRET 环境变量（≥ {} 字节）",
                    MIN_SECRET_LEN
                ));
            }
        }
    };

    validate_secret_length(secret.len())?;

    JWT_SECRET
        .set(secret)
        .map_err(|_| anyhow!("JWT_SECRET 已初始化"))?;
    Ok(())
}

/// 获取当前 JWT 密钥
///
/// 优先返回 [`init_jwt_secret`] 写入的密钥；若未初始化（如测试、bin 工具），
/// 回退到 [`DEV_DEFAULT_SECRET`] 以保证调用方零配置可用。
fn secret() -> &'static [u8] {
    JWT_SECRET
        .get_or_init(|| DEV_DEFAULT_SECRET.to_vec())
        .as_slice()
}

/// JWT 令牌声明
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Claims {
    /// 主题（用户名）
    pub sub: String,
    /// 过期时间（Unix 时间戳）
    pub exp: usize,
}

/// 密码加密：将明文密码转换为 Argon2 哈希
///
/// # 参数
/// * `password` - 明文密码
///
/// # 返回
/// 密码哈希字符串
///
/// # 示例
/// ```no_run
/// use tagflow_core::core::auth::hash_password;
///
/// let hash = hash_password("my_secure_password").unwrap();
/// println!("Hash: {}", hash);
/// ```
pub fn hash_password(password: &str) -> Result<String> {
    let salt = SaltString::generate(&mut OsRng);
    let argon2 = Argon2::default();
    let password_hash = argon2
        .hash_password(password.as_bytes(), &salt)
        .map_err(|e| anyhow!("Failed to hash password: {}", e))?
        .to_string();
    Ok(password_hash)
}

/// 密码校验：对比明文密码与存储的哈希值
///
/// # 参数
/// * `password` - 待校验的明文密码
/// * `hash` - 存储的密码哈希
///
/// # 返回
/// * `true` - 密码匹配
/// * `false` - 密码不匹配
///
/// # 示例
/// ```no_run
/// use tagflow_core::core::auth::{hash_password, verify_password};
///
/// let hash = hash_password("my_password").unwrap();
/// assert!(verify_password("my_password", &hash));
/// assert!(!verify_password("wrong_password", &hash));
/// ```
pub fn verify_password(password: &str, hash: &str) -> bool {
    let parsed_hash = match PasswordHash::new(hash) {
        Ok(h) => h,
        Err(_) => return false,
    };
    Argon2::default()
        .verify_password(password.as_bytes(), &parsed_hash)
        .is_ok()
}

/// 创建 JWT 令牌
///
/// # 参数
/// * `username` - 用户名
///
/// # 返回
/// JWT 令牌字符串
///
/// # 令牌有效期
/// 默认 24 小时
///
/// # 示例
/// ```no_run
/// use tagflow_core::core::auth::create_jwt;
///
/// let token = create_jwt("alice").unwrap();
/// println!("Token: {}", token);
/// ```
pub fn create_jwt(username: &str) -> Result<String> {
    let expiration = Utc::now()
        .checked_add_signed(Duration::hours(24))
        .expect("valid timestamp")
        .timestamp() as usize;

    let claims = Claims {
        sub: username.to_owned(),
        exp: expiration,
    };

    encode(
        &Header::default(),
        &claims,
        &EncodingKey::from_secret(secret()),
    )
    .map_err(|e| anyhow!("Failed to create JWT: {}", e))
}

/// 验证并解码 JWT 令牌
///
/// # 参数
/// * `token` - JWT 令牌字符串
///
/// # 返回
/// 成功返回 Claims，失败返回错误
///
/// # 示例
/// ```no_run
/// use tagflow_core::core::auth::{create_jwt, decode_jwt};
///
/// let token = create_jwt("alice").unwrap();
/// let claims = decode_jwt(&token).unwrap();
/// assert_eq!(claims.sub, "alice");
/// ```
pub fn decode_jwt(token: &str) -> Result<Claims> {
    decode::<Claims>(
        token,
        &DecodingKey::from_secret(secret()),
        &Validation::default(),
    )
    .map(|data| data.claims)
    .map_err(|e| anyhow!("Failed to decode JWT: {}", e))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_password_hash_and_verify() {
        let password = "test_password_123";
        let hash = hash_password(password).unwrap();
        assert!(verify_password(password, &hash));
        assert!(!verify_password("wrong_password", &hash));
    }

    #[test]
    fn test_jwt_create_and_decode() {
        let username = "alice";
        let token = create_jwt(username).unwrap();
        let claims = decode_jwt(&token).unwrap();
        assert_eq!(claims.sub, username);
    }

    #[test]
    fn test_validate_secret_length_rejects_short_secret() {
        // 短于 32 字节必须被拒
        assert!(validate_secret_length(0).is_err());
        assert!(validate_secret_length(31).is_err());
        // 恰好 32 字节通过
        assert!(validate_secret_length(32).is_ok());
        // 超过阈值通过
        assert!(validate_secret_length(64).is_ok());
    }
}
