进入 **Milestone 6：认证模块 (Authentication & Security) 实现**。

作为一名 Rust 初学者，这个阶段你会接触到 Rust 处理安全性的严谨方式。我们将引入 **Argon2**（目前最安全的密码哈希算法之一）和 **JWT**（无状态令牌），确保你的 TagFlow 即使暴露在互联网上也能保护数据隐私。

---

### 1. 更新依赖配置 (Cargo.toml)

我们需要添加处理加密和令牌的库：

```toml
[dependencies]
# ... 之前的依赖 ...
# 密码哈希
argon2 = "0.5"
# JWT 令牌
jsonwebtoken = "9.2"
# 生成随机盐值
rand_core = { version = "0.6", features = ["getrandom"] }
```

---

### 2. 数据库迁移：创建用户表

创建文件 `migrations/202512290002_create_users.sql`：

```sql
CREATE TABLE IF NOT EXISTS users (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    username TEXT NOT NULL UNIQUE,
    password_hash TEXT NOT NULL,
    created_at DATETIME DEFAULT CURRENT_TIMESTAMP
);
```

---

### 3. 定义安全核心逻辑 (core/auth.rs)

我们需要两个功能：**加密密码**和**发放令牌**。

```rust
use argon2::{
    password_hash::{rand_core::OsRng, PasswordHash, PasswordHasher, PasswordVerifier, SaltString},
    Argon2,
};
use jsonwebtoken::{encode, decode, Header, Algorithm, Validation, EncodingKey, DecodingKey};
use serde::{Deserialize, Serialize};
use chrono::{Utc, Duration};

const JWT_SECRET: &[u8] = b"your_ultra_secret_key"; // 实际应从环境变量读取

#[derive(Debug, Serialize, Deserialize)]
pub struct Claims {
    pub sub: String,    // 用户名
    pub exp: usize,     // 过期时间
}

/// 密码加密：将明文转换为哈希
pub fn hash_password(password: &str) -> anyhow::Result<String> {
    let salt = SaltString::generate(&mut OsRng);
    let argon2 = Argon2::default();
    let password_hash = argon2.hash_password(password.as_bytes(), &salt)
        .map_err(|e| anyhow::anyhow!(e.to_string()))?
        .to_string();
    Ok(password_hash)
}

/// 密码校验：对比明文与哈希
pub fn verify_password(password: &str, hash: &str) -> bool {
    let parsed_hash = PasswordHash::new(hash).unwrap();
    Argon2::default().verify_password(password.as_bytes(), &parsed_hash).is_ok()
}

/// 创建 JWT 令牌
pub fn create_jwt(username: &str) -> anyhow::Result<String> {
    let expiration = Utc::now()
        .checked_add_signed(Duration::hours(24))
        .expect("valid timestamp")
        .timestamp();

    let claims = Claims {
        sub: username.to_owned(),
        exp: expiration as usize,
    };

    encode(&Header::default(), &claims, &EncodingKey::from_secret(JWT_SECRET))
        .map_err(|e| anyhow::anyhow!(e))
}
```

---

### 4. 实现登录 API (api/auth.rs)

```rust
use axum::{extract::State, Json};
use sqlx::SqlitePool;
use serde::{Deserialize, Serialize};
use crate::core::auth::{verify_password, create_jwt};

#[derive(Deserialize)]
pub struct LoginRequest {
    pub username: String,
    pub password: String,
}

#[derive(Serialize)]
pub struct AuthResponse {
    pub token: String,
}

pub async fn login(
    State(pool): State<SqlitePool>,
    Json(payload): Json<LoginRequest>,
) -> Result<Json<AuthResponse>, axum::http::StatusCode> {
    // 1. 从数据库查找用户
    let user = sqlx::query!("SELECT password_hash FROM users WHERE username = ?", payload.username)
        .fetch_optional(&pool)
        .await
        .map_err(|_| axum::http::StatusCode::INTERNAL_SERVER_ERROR)?;

    if let Some(user_record) = user {
        // 2. 校验密码
        if verify_password(&payload.password, &user_record.password_hash) {
            // 3. 生成并返回 Token
            let token = create_jwt(&payload.username).map_err(|_| axum::http::StatusCode::INTERNAL_SERVER_ERROR)?;
            return Ok(Json(AuthResponse { token }));
        }
    }

    // 登录失败
    Err(axum::http::StatusCode::UNAUTHORIZED)
}
```

---

### 5. 实现鉴权中间件 (Middleware)

中间件就像大门的保安，它会检查每个请求的 `Authorization` 请求头。

```rust
use axum::{
    body::Body,
    http::{Request, StatusCode},
    middleware::Next,
    response::Response,
};
use jsonwebtoken::{decode, DecodingKey, Validation, Algorithm};
use crate::core::auth::{Claims, JWT_SECRET};

pub async fn auth_middleware(
    mut req: Request<Body>,
    next: Next,
) -> Result<Response, StatusCode> {
    let auth_header = req.headers()
        .get(axum::http::header::AUTHORIZATION)
        .and_then(|h| h.to_str().ok());

    if let Some(auth_header) = auth_header {
        if auth_header.starts_with("Bearer ") {
            let token = &auth_header[7..];
            let validation = Validation::new(Algorithm::HS256);
            
            if decode::<Claims>(token, &DecodingKey::from_secret(JWT_SECRET), &validation).is_ok() {
                // Token 有效，允许通过
                return Ok(next.run(req).await);
            }
        }
    }

    // 无 Token 或 Token 无效
    Err(StatusCode::UNAUTHORIZED)
}
```

---

### 6. 应用中间件：保护路由 (main.rs)

在 Rust 的 Axum 中，你可以很方便地选择哪些路由需要保护。

```rust
use axum::{middleware, routing::{get, post}};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // ... 之前的初始化代码 ...

    // 1. 公开路由（如登录）
    let auth_routes = Router::new()
        .route("/api/auth/login", post(api::auth::login));

    // 2. 受保护的路由（需要 Token 访问）
    let protected_routes = Router::new()
        .route("/api/v1/files", get(api::file::list_files))
        .route("/api/v1/tags/tree", get(api::tag::get_tag_tree))
        // 在这一层级的所有路由都会经过 auth_middleware
        .layer(middleware::from_fn(api::auth::auth_middleware));

    let app = Router::new()
        .merge(auth_routes)
        .merge(protected_routes)
        .with_state(pool);

    // ... 启动服务器 ...
    Ok(())
}
```

---

### 7. 初次启动：初始化管理员逻辑

由于没有注册页面（为了安全），我们可以在程序启动时检查是否存在用户，如果不存在，则根据环境变量创建一个默认管理员。

```rust
// 在 main.rs 启动数据库后调用
async fn ensure_admin_user(pool: &SqlitePool) -> anyhow::Result<()> {
    let count = sqlx::query_scalar!("SELECT count(*) FROM users").fetch_one(pool).await?;
    if count == 0 {
        let password = "admin_password"; // 实际应从环境变量读取
        let hash = crate::core::auth::hash_password(password)?;
        sqlx::query!("INSERT INTO users (username, password_hash) VALUES (?, ?)", "admin", hash)
            .execute(pool).await?;
        println!("检测到新系统：已创建默认管理员 [admin / admin_password]");
    }
    Ok(())
}
```

---