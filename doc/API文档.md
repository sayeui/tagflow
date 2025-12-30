# TagFlow API 文档

**版本：** v1.1
**基础 URL：** `http://localhost:8080`
**内容类型：** `application/json`

---

## 概述

TagFlow API 提供用户认证、标签树查询和文件检索功能，支持层级标签的递归过滤。

### 认证方式

API 使用 JWT (JSON Web Token) 进行认证。除登录接口外，所有 API 请求需要在请求头中携带有效的访问令牌：

```http
Authorization: Bearer <your_token>
```

**令牌有效期：** 24 小时

**默认管理员凭据：**
- 用户名：`admin`
- 密码：`admin123`

> 生产环境应通过环境变量 `TAGFLOW_ADMIN_USERNAME` 和 `TAGFLOW_ADMIN_PASSWORD` 配置管理员凭据。

---

## API 端点

### 1. 用户登录

使用用户名和密码登录，获取访问令牌。

**端点：** `POST /api/auth/login`

**请求头：**
```http
Content-Type: application/json
```

**请求体：**

| 字段 | 类型 | 必填 | 描述 |
|------|------|------|------|
| `username` | string | 是 | 用户名 |
| `password` | string | 是 | 密码 |

**请求示例：**

```bash
curl -X POST http://localhost:8080/api/auth/login \
  -H "Content-Type: application/json" \
  -d '{"username":"admin","password":"admin123"}'
```

**成功响应 (200)：**

```json
{
  "token": "eyJ0eXAiOiJKV1QiLCJhbGciOiJIUzI1NiJ9.eyJzdWIiOiJhZG1pbiIsImV4cCI6MTc2NzA5MzIwMX0.ohXCPW5ueKxemlTc3zguAZz1uXTJV-KhFyidS-L60ZA"
}
```

**失败响应 (401)：**

无响应体。

---

### 2. 获取标签树

返回所有标签的嵌套树形结构。

**端点：** `GET /api/v1/tags/tree`

**请求头：**
```http
Authorization: Bearer <your_token>
```

**请求参数：** 无

**响应示例：**

```json
[
  {
    "id": 1,
    "name": "Personal",
    "category": "path",
    "children": []
  },
  {
    "id": 2,
    "name": "Work",
    "category": "path",
    "children": [
      {
        "id": 3,
        "name": "Design",
        "category": "path",
        "children": []
      },
      {
        "id": 4,
        "name": "Dev",
        "category": "path",
        "children": []
      }
    ]
  }
]
```

**响应字段说明：**

| 字段 | 类型 | 描述 |
|------|------|------|
| `id` | integer | 标签唯一标识符 |
| `name` | string | 标签名称 |
| `category` | string | 标签类别 (`path`, `type`, `user`, `time`) |
| `children` | array | 子标签列表 |

---

### 3. 获取文件列表

根据标签过滤或分页获取文件列表。

**端点：** `GET /api/v1/files`

**请求头：**
```http
Authorization: Bearer <your_token>
```

**查询参数：**

| 参数 | 类型 | 必填 | 默认值 | 描述 |
|------|------|------|--------|------|
| `tag_id` | integer | 否 | - | 按标签 ID 过滤文件 |
| `recursive` | boolean | 否 | `true` | 是否包含子标签的文件 |
| `page` | integer | 否 | `1` | 页码（从 1 开始） |
| `limit` | integer | 否 | `50` | 每页返回的文件数量 |

**请求示例：**

```bash
# 获取所有文件
GET /api/v1/files

# 按 Work 标签过滤（包含子标签）
GET /api/v1/files?tag_id=2&recursive=true

# 按 Work 标签过滤（仅直接关联的文件）
GET /api/v1/files?tag_id=2&recursive=false

# 分页查询
GET /api/v1/files?page=2&limit=20
```

**响应示例：**

```json
{
  "items": [
    {
      "id": 1,
      "filename": "diary.txt",
      "extension": "txt",
      "size": 5,
      "mtime": 1766978252,
      "parent_path": "Personal/"
    },
    {
      "id": 2,
      "filename": "logo.png",
      "extension": "png",
      "size": 5,
      "mtime": 1766978252,
      "parent_path": "Work/Design/"
    }
  ],
  "total": 2
}
```

**响应字段说明：**

| 字段 | 类型 | 描述 |
|------|------|------|
| `items` | array | 文件列表 |
| `items[].id` | integer | 文件唯一标识符 |
| `items[].filename` | string | 文件名 |
| `items[].extension` | string | 文件扩展名 |
| `items[].size` | integer | 文件大小（字节） |
| `items[].mtime` | integer | 修改时间戳（Unix 时间） |
| `items[].parent_path` | string | 父目录路径 |
| `total` | integer | 返回的文件总数 |

---

## 递归查询说明

当 `recursive=true` 时，系统使用 **递归 CTE (Common Table Expression)** 查找指定标签及其所有子孙标签下的文件。

例如，查询 `tag_id=2` (Work) 且 `recursive=true`：

```
Work (id=2)
  ├── Design (id=3)  ← 包含
  └── Dev (id=4)     ← 包含
```

将返回关联到 Work、Design 或 Dev 标签的所有文件。

---

## 错误响应

**HTTP 状态码：**

| 状态码 | 描述 |
|--------|------|
| 200 | 成功 |
| 400 | 请求参数错误 |
| 401 | 未授权（令牌无效或缺失） |
| 500 | 服务器内部错误 |

**错误响应示例：**

```json
{
  "error": "Invalid query parameter"
}
```

---

## 使用示例

### cURL

```bash
# 1. 登录获取令牌
TOKEN=$(curl -s -X POST http://localhost:8080/api/auth/login \
  -H "Content-Type: application/json" \
  -d '{"username":"admin","password":"admin123"}' \
  | jq -r '.token')

# 2. 使用令牌获取标签树
curl http://localhost:8080/api/v1/tags/tree \
  -H "Authorization: Bearer $TOKEN"

# 3. 使用令牌获取 Work 标签下的所有文件
curl "http://localhost:8080/api/v1/files?tag_id=2&recursive=true" \
  -H "Authorization: Bearer $TOKEN"
```

### JavaScript (fetch)

```javascript
// 登录获取令牌
const loginResponse = await fetch('http://localhost:8080/api/auth/login', {
  method: 'POST',
  headers: { 'Content-Type': 'application/json' },
  body: JSON.stringify({ username: 'admin', password: 'admin123' })
});
const { token } = await loginResponse.json();

// 使用令牌获取标签树
const tags = await fetch('http://localhost:8080/api/v1/tags/tree', {
  headers: { 'Authorization': `Bearer ${token}` }
}).then(res => res.json());

// 使用令牌获取文件列表
const files = await fetch('http://localhost:8080/api/v1/files?tag_id=2&recursive=true', {
  headers: { 'Authorization': `Bearer ${token}` }
}).then(res => res.json());

console.log(files.items);
```

### Python (requests)

```python
import requests

# 登录获取令牌
login_resp = requests.post('http://localhost:8080/api/auth/login',
  json={'username': 'admin', 'password': 'admin123'})
token = login_resp.json()['token']

# 设置认证头
headers = {'Authorization': f'Bearer {token}'}

# 获取标签树
tags = requests.get('http://localhost:8080/api/v1/tags/tree',
  headers=headers).json()

# 获取文件列表
files = requests.get('http://localhost:8080/api/v1/files',
  headers=headers,
  params={'tag_id': 2, 'recursive': True}).json()

print(files['items'])
```

---

## 安全说明

- **密码加密**：使用 Argon2 算法进行密码哈希存储
- **令牌传输**：JWT 通过 HTTPS 传输（生产环境建议启用）
- **令牌存储**：客户端应安全存储令牌（推荐使用 localStorage 或 sessionStorage）
- **令牌过期**：令牌有效期为 24 小时，过期后需重新登录

---

## 性能说明

- **标签树查询**：一次性加载所有标签到内存，在应用层构建树结构
- **文件递归查询**：使用 SQL 递归 CTE，在数据库层完成过滤，性能高效
- **分页支持**：使用 `LIMIT` 和 `OFFSET` 减少数据传输
- **认证中间件**：JWT 验证在请求处理前完成，无额外数据库查询

---

## 更新日志

| 版本 | 日期 | 变更 |
|------|------|------|
| v1.1 | 2025-12-30 | 新增用户认证模块，添加 JWT 认证和登录接口 |
| v1.0 | 2025-12-29 | 初始版本，实现标签树和文件检索 API |
