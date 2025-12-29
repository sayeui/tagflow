# TagFlow API 文档

**版本：** v1.0
**基础 URL：** `http://localhost:8080`
**内容类型：** `application/json`

---

## 概述

TagFlow API 提供标签树查询和文件检索功能，支持层级标签的递归过滤。

---

## API 端点

### 1. 获取标签树

返回所有标签的嵌套树形结构。

**端点：** `GET /api/v1/tags/tree`

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

### 2. 获取文件列表

根据标签过滤或分页获取文件列表。

**端点：** `GET /api/v1/files`

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
# 获取标签树
curl http://localhost:8080/api/v1/tags/tree

# 获取 Work 标签下的所有文件
curl "http://localhost:8080/api/v1/files?tag_id=2&recursive=true"
```

### JavaScript (fetch)

```javascript
// 获取标签树
const tags = await fetch('http://localhost:8080/api/v1/tags/tree')
  .then(res => res.json());

// 获取文件列表
const files = await fetch('http://localhost:8080/api/v1/files?tag_id=2&recursive=true')
  .then(res => res.json());

console.log(files.items);
```

### Python (requests)

```python
import requests

# 获取标签树
tags = requests.get('http://localhost:8080/api/v1/tags/tree').json()

# 获取文件列表
files = requests.get('http://localhost:8080/api/v1/files', params={
    'tag_id': 2,
    'recursive': True
}).json()

print(files['items'])
```

---

## 性能说明

- **标签树查询**：一次性加载所有标签到内存，在应用层构建树结构
- **文件递归查询**：使用 SQL 递归 CTE，在数据库层完成过滤，性能高效
- **分页支持**：使用 `LIMIT` 和 `OFFSET` 减少数据传输

---

## 更新日志

| 版本 | 日期 | 变更 |
|------|------|------|
| v1.0 | 2025-12-29 | 初始版本，实现标签树和文件检索 API |
