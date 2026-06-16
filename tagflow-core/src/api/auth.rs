//! 认证 API 模块
//!
//! 提供登录接口和鉴权中间件。

use axum::{
    Extension,
    body::Body,
    extract::State,
    http::{Request, StatusCode, header},
    middleware::Next,
    response::{Json, Response},
};
use serde::{Deserialize, Serialize};
use sqlx::SqlitePool;

use crate::core::auth::{Claims, create_jwt, decode_jwt, hash_password, verify_password};

/// 登录请求
#[derive(Debug, Deserialize)]
pub struct LoginRequest {
    pub username: String,
    pub password: String,
}

/// 认证响应
#[derive(Debug, Serialize)]
pub struct AuthResponse {
    pub token: String,
}

/// 错误响应
#[derive(Debug, Serialize)]
pub struct ErrorResponse {
    pub error: String,
}

/// 登录 API 处理函数
///
/// # 路由
/// POST /api/auth/login
///
/// # 请求体
/// ```json
/// {
///   "username": "admin",
///   "password": "password"
/// }
/// ```
///
/// # 成功响应 (200)
/// ```json
/// {
///   "token": "eyJhbGciOiJIUzI1NiIsInR5cCI6IkpXVCJ9..."
/// }
/// ```
///
/// # 失败响应 (401)
/// 无响应体
pub async fn login(
    State(pool): State<SqlitePool>,
    Json(payload): Json<LoginRequest>,
) -> Result<Json<AuthResponse>, StatusCode> {
    // 从数据库查找用户密码哈希
    #[derive(sqlx::FromRow)]
    struct UserRecord {
        password_hash: String,
    }

    let result =
        sqlx::query_as::<_, UserRecord>("SELECT password_hash FROM users WHERE username = ?")
            .bind(&payload.username)
            .fetch_optional(&pool)
            .await;

    let user = match result {
        Ok(Some(u)) => u,
        Ok(None) => return Err(StatusCode::UNAUTHORIZED),
        Err(_) => return Err(StatusCode::INTERNAL_SERVER_ERROR),
    };

    // 校验密码
    if verify_password(&payload.password, &user.password_hash) {
        // 生成并返回 Token
        match create_jwt(&payload.username) {
            Ok(token) => Ok(Json(AuthResponse { token })),
            Err(_) => Err(StatusCode::INTERNAL_SERVER_ERROR),
        }
    } else {
        Err(StatusCode::UNAUTHORIZED)
    }
}

/// 鉴权中间件
///
/// 验证请求头中的 JWT 令牌，令牌有效则放行，否则返回 401。
///
/// # 使用方式
/// 在需要保护的路由上应用此中间件：
/// ```ignore
/// let protected_routes = Router::new()
///     .route("/api/v1/files", get(api::file::list_files))
///     .layer(middleware::from_fn(api::auth::auth_middleware));
/// ```
///
/// # 请求头
/// 客户端需要在请求头中携带：
/// ```text
/// Authorization: Bearer <token>
/// ```
pub async fn auth_middleware(mut req: Request<Body>, next: Next) -> Result<Response, StatusCode> {
    // 1. 优先 Authorization: Bearer <token>
    let header_token = req
        .headers()
        .get(header::AUTHORIZATION)
        .and_then(|h| h.to_str().ok())
        .and_then(|v| v.strip_prefix("Bearer "));

    // 2. 兜底查询参数 ?token=<jwt>（供 <img>/<video>/<iframe src> 等无法携带
    //    Authorization 头的媒体请求；同时修复缩略图因裸 <img src> 被 401 的历史问题）。
    //    安全权衡：JWT 进入 URL（server log / 浏览器历史可见）；自托管单用户 LAN 可接受，
    //    未来可硬化为短时效 media token。
    let query_token = query_token(req.uri().query());

    match header_token.or(query_token).and_then(|t| decode_jwt(t).ok()) {
        Some(claims) => {
            req.extensions_mut().insert(claims);
            Ok(next.run(req).await)
        }
        None => Err(StatusCode::UNAUTHORIZED),
    }
}

/// 从 URL query 中提取首个非空 `token` 参数值。
///
/// 抽成纯函数便于单测；不引入 serde_urlencoded 以保持中间件零额外分配。
fn query_token(query: Option<&str>) -> Option<&str> {
    let q = query?;
    for kv in q.split('&') {
        let mut it = kv.splitn(2, '=');
        if it.next() == Some("token") {
            return it.next().filter(|s| !s.is_empty());
        }
    }
    None
}

/// 从请求扩展中提取当前用户信息的辅助函数
///
/// # 使用方式
/// ```ignore
/// pub async fn protected_handler(
///     claims: Option<axum::Extension<crate::core::auth::Claims>>,
/// ) -> impl IntoResponse {
///     match claims {
///         Some(claims) => format!("Hello, {}!", claims.sub),
///         None => StatusCode::UNAUTHORIZED.into_response(),
///     }
/// }
/// ```
pub fn current_user(req: &Request<Body>) -> Option<&Claims> {
    req.extensions().get::<Claims>()
}

/// 修改密码请求
#[derive(Debug, Deserialize)]
pub struct UpdatePasswordRequest {
    pub old_password: String,
    pub new_password: String,
}

/// 修改密码 API 处理函数
///
/// # 路由
/// POST /api/auth/update-password
///
/// # 请求头
/// ```http
/// Authorization: Bearer <token>
/// ```
///
/// # 请求体
/// ```json
/// {
///   "old_password": "current_password",
///   "new_password": "new_password"
/// }
/// ```
///
/// # 成功响应 (200)
/// 无响应体
///
/// # 失败响应
/// - 403: 旧密码错误
/// - 401: 未授权
/// - 500: 服务器错误
pub async fn update_password(
    State(pool): State<SqlitePool>,
    Extension(claims): Extension<Claims>,
    Json(payload): Json<UpdatePasswordRequest>,
) -> StatusCode {
    // 1. 从 Extension 中获取当前登录用户信息（由中间件放入）
    let username = &claims.sub;

    // 2. 从数据库获取当前密码哈希
    #[derive(sqlx::FromRow)]
    struct UserRecord {
        password_hash: String,
    }

    let result =
        sqlx::query_as::<_, UserRecord>("SELECT password_hash FROM users WHERE username = ?")
            .bind(username)
            .fetch_optional(&pool)
            .await;

    let user = match result {
        Ok(Some(u)) => u,
        Ok(None) => return StatusCode::UNAUTHORIZED,
        Err(_) => return StatusCode::INTERNAL_SERVER_ERROR,
    };

    // 3. 验证旧密码
    if !verify_password(&payload.old_password, &user.password_hash) {
        return StatusCode::FORBIDDEN;
    }

    // 4. 加密新密码并更新数据库
    match hash_password(&payload.new_password) {
        Ok(new_hash) => {
            if sqlx::query("UPDATE users SET password_hash = ? WHERE username = ?")
                .bind(&new_hash)
                .bind(username)
                .execute(&pool)
                .await
                .is_err()
            {
                return StatusCode::INTERNAL_SERVER_ERROR;
            }
        }
        Err(_) => return StatusCode::INTERNAL_SERVER_ERROR,
    }

    StatusCode::OK
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_login_request_deserialize() {
        let json = r#"{"username": "admin", "password": "pass"}"#;
        let req: LoginRequest = serde_json::from_str(json).unwrap();
        assert_eq!(req.username, "admin");
        assert_eq!(req.password, "pass");
    }

    #[test]
    fn test_auth_response_serialize() {
        let resp = AuthResponse {
            token: "test_token".to_string(),
        };
        let json = serde_json::to_string(&resp).unwrap();
        assert_eq!(json, r#"{"token":"test_token"}"#);
    }

    #[test]
    fn query_token_extracts_first_non_empty() {
        assert_eq!(query_token(Some("token=abc")), Some("abc"));
        assert_eq!(
            query_token(Some("foo=1&token=abc&bar=2")),
            Some("abc")
        );
        assert_eq!(query_token(Some("foo=1")), None);
        assert_eq!(query_token(None), None);
        // 空值不算有效 token（避免拼接 `?token=` 时误判）
        assert_eq!(query_token(Some("token=")), None);
    }
}
