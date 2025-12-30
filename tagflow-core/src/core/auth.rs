//! 认证模块
//!
//! 提供密码哈希（Argon2）和 JWT 令牌管理功能。

use argon2::{
    password_hash::{rand_core::OsRng, PasswordHash, PasswordHasher, PasswordVerifier, SaltString},
    Argon2,
};
use anyhow::{anyhow, Result};
use chrono::{Duration, Utc};
use jsonwebtoken::{decode, encode, DecodingKey, EncodingKey, Header, Validation};
use serde::{Deserialize, Serialize};

/// JWT 密钥（生产环境应从环境变量读取）
const JWT_SECRET: &[u8] = b"your_ultra_secret_key_change_in_production";

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
        &EncodingKey::from_secret(JWT_SECRET),
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
        &DecodingKey::from_secret(JWT_SECRET),
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
}
