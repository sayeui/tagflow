# Milestone 6-1 认证 UI 与安全设置

## 1. 阶段目标
本阶段的目标是为系统构建完整、闭环的用户安全体验。从用户打开系统的登录校验，到 API 请求的安全拦截，再到个人密码的自我维护。

*   **核心产出：** 登录页面、请求拦截机制、路由守卫、修改密码页面、后端密码更新 API。

---

## 2. 后端开发任务 (Rust/Axum)

虽然 M6 已经完成了登录逻辑，但 M6-1 需要补充“密码修改”接口。

### 2.1 修改密码 API 实现
**文件：** `src/api/auth.rs` (或 `src/api/user.rs`)

```rust
use axum::{extract::{State, Request}, Json, http::StatusCode};
use sqlx::SqlitePool;
use crate::core::auth::{verify_password, hash_password, Claims};

#[derive(serde::Deserialize)]
pub struct UpdatePasswordRequest {
    pub old_password: String,
    pub new_password: String,
}

pub async fn update_password(
    State(pool): State<SqlitePool>,
    // 注意：这里的 Claims 是由中间件解析 Token 后注入到 Request 扩展中的
    req: Request, 
    Json(payload): Json<UpdatePasswordRequest>,
) -> StatusCode {
    // 1. 从 Request 扩展中获取当前登录的用户名 (由中间件放入)
    let claims = req.extensions().get::<Claims>().unwrap();
    let username = &claims.sub;

    // 2. 从数据库获取当前哈希
    let user = sqlx::query!("SELECT password_hash FROM users WHERE username = ?", username)
        .fetch_one(&pool).await.unwrap();

    // 3. 验证旧密码
    if !verify_password(&payload.old_password, &user.password_hash) {
        return StatusCode::FORBIDDEN; // 旧密码错误
    }

    // 4. 加密并存储新密码
    let new_hash = hash_password(&payload.new_password).unwrap();
    sqlx::query!("UPDATE users SET password_hash = ? WHERE username = ?", new_hash, username)
        .execute(&pool).await.unwrap();

    StatusCode::OK
}
```

---

## 3. 前端开发任务 (Vue 3 / Vite)

### 3.1 状态管理与持久化 (Pinia)
**文件：** `src/stores/auth.ts`

```typescript
import { defineStore } from 'pinia';

export const useAuthStore = defineStore('auth', {
  state: () => ({
    token: localStorage.getItem('auth_token') || null,
    username: localStorage.getItem('username') || null,
  }),
  getters: {
    isLoggedIn: (state) => !!state.token,
  },
  actions: {
    setToken(token: string, username: string) {
      this.token = token;
      this.username = username;
      localStorage.setItem('auth_token', token);
      localStorage.setItem('username', username);
    },
    logout() {
      this.token = null;
      localStorage.removeItem('auth_token');
      localStorage.removeItem('username');
      window.location.href = '/login';
    }
  }
});
```

### 3.2 路由守卫 (Navigation Guards)
**文件：** `src/router/index.ts`

```typescript
router.beforeEach((to, from, next) => {
  const authStore = useAuthStore();
  
  if (to.name !== 'Login' && !authStore.isLoggedIn) {
    // 如果不是去登录页且未登录，重定向到登录
    next({ name: 'Login' });
  } else if (to.name === 'Login' && authStore.isLoggedIn) {
    // 如果已登录还想去登录页，直接送回首页
    next({ name: 'Home' });
  } else {
    next();
  }
});
```

### 3.3 Axios 全局拦截器
**文件：** `src/api/http.ts` (封装 Axios)

```typescript
import axios from 'axios';
import { useAuthStore } from '@/stores/auth';

const instance = axios.create({ baseURL: '/api' });

// 请求拦截：自动附加 Token
instance.interceptors.request.use(config => {
  const authStore = useAuthStore();
  if (authStore.token) {
    config.headers.Authorization = `Bearer ${authStore.token}`;
  }
  return config;
});

// 响应拦截：处理 401 错误
instance.interceptors.response.use(
  res => res,
  err => {
    if (err.response?.status === 401) {
      const authStore = useAuthStore();
      authStore.logout(); // 清理并跳转
    }
    return Promise.reject(err);
  }
);
```

---

## 4. UI 组件开发

### 4.1 登录界面 (`views/Login.vue`)
*   **设计要点：** 居中卡片布局、Logo 展示、错误信息提示、加载状态。
*   **交互：** 点击登录 -> 调用 `/api/auth/login` -> 成功后存入 Store -> 跳转至 `/`。

### 4.2 安全设置页面 (`views/settings/Security.vue`)
*   **表单字段：** 当前密码、新密码、确认新密码。
*   **校验逻辑：**
    *   前端校验新密码长度（如 > 8位）。
    *   校验“新密码”与“确认新密码”是否一致。
    *   成功后弹出提示并要求重新登录（可选）。

---

## 5. 验收标准 (Definition of Done)

1.  **首屏拦截：** 清空浏览器 `localStorage` 后访问系统，必须自动跳转到登录页。
2.  **登录闭环：** 输入正确的 `admin` 账号密码能进入系统，错误的提示“用户名或密码错误”。
3.  **持久化测试：** 登录后刷新页面，依然保持登录状态（不跳转）。
4.  **接口防护：** 手动在 F12 控制台清空 `Authorization` 头发送请求，后端应返回 `401`，且前端自动跳转回登录。
5.  **安全维护：** 修改密码成功后，使用旧密码尝试登录应失败。

## 6. 开发提示 (Tips)
*   **环境变量：** 在开发阶段，可以在前端 Vite 配置中设置 `proxy`，将 `/api` 转发到 `localhost:8080`，避免跨域问题。
*   **用户体验：** 在修改密码成功后，建议给一个 2 秒的成功 Toast 提示，然后调用 `authStore.logout()` 让用户用新密码重新验证。

