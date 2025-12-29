进入 **Milestone 7：存储管理模块 (Storage Management) 实现**。

在这一阶段，我们将把系统的“资源源”从硬编码配置转变为动态管理。用户可以通过 UI 界面添加本地目录或 WebDAV 地址，并实时测试连接是否成功。

---

### 1. 定义 API 数据结构 (DTO)

在 `src/models/dto.rs` 中，我们需要定义创建和返回资源库的数据结构。

```rust
#[derive(Deserialize)]
pub struct CreateLibraryRequest {
    pub name: String,
    pub protocol: String, // "local" 或 "webdav"
    pub base_path: String,
    pub config_json: Option<String>,
}

#[derive(Serialize)]
pub struct LibraryResponse {
    pub id: i32,
    pub name: String,
    pub protocol: String,
    pub base_path: String,
    pub last_scanned_at: Option<chrono::DateTime<chrono::Utc>>,
}
```

---

### 2. 实现后端业务逻辑 (api/library.rs)

我们需要实现 CRUD（增删改查）以及一个关键的“连接测试”接口。

```rust
use axum::{extract::{State, Path}, Json, http::StatusCode};
use sqlx::SqlitePool;
use crate::models::dto::{CreateLibraryRequest, LibraryResponse};
use crate::models::db::Library;

/// 获取所有已配置的资源库
pub async fn list_libraries(State(pool): State<SqlitePool>) -> Json<Vec<LibraryResponse>> {
    let libs = sqlx::query_as!(Library, "SELECT * FROM libraries")
        .fetch_all(&pool).await.unwrap_or_default();
    
    let response = libs.into_iter().map(|l| LibraryResponse {
        id: l.id,
        name: l.name,
        protocol: l.protocol,
        base_path: l.base_path,
        last_scanned_at: l.last_scanned_at,
    }).collect();

    Json(response)
}

/// 添加新的资源库
pub async fn create_library(
    State(pool): State<SqlitePool>,
    Json(payload): Json<CreateLibraryRequest>
) -> Result<StatusCode, StatusCode> {
    sqlx::query!(
        "INSERT INTO libraries (name, protocol, base_path, config_json) VALUES (?, ?, ?, ?)",
        payload.name, payload.protocol, payload.base_path, payload.config_json
    ).execute(&pool).await.map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    Ok(StatusCode::CREATED)
}

/// 测试路径有效性
pub async fn test_library_connection(
    Json(payload): Json<CreateLibraryRequest>
) -> Json<bool> {
    if payload.protocol == "local" {
        // 检查本地目录是否存在且可读
        let path = std::path::Path::new(&payload.base_path);
        Json(path.exists() && path.is_dir())
    } else {
        // WebDAV 逻辑预留，目前返回 false 或实现简单探测
        Json(false)
    }
}

/// 删除资源库
pub async fn delete_library(
    State(pool): State<SqlitePool>,
    Path(id): Path<i32>
) -> StatusCode {
    let res = sqlx::query!("DELETE FROM libraries WHERE id = ?", id)
        .execute(&pool).await;
    
    if res.is_ok() { StatusCode::NO_CONTENT } else { StatusCode::INTERNAL_SERVER_ERROR }
}
```

---

### 3. 前端存储管理界面 (Vue 3)

我们需要一个管理页面，让用户能够输入路径并点击“测试”和“保存”。

在 `src/views/Settings.vue` (或新增组件) 中：

```vue
<script setup lang="ts">
import { ref, onMounted } from 'vue';
import axios from 'axios';
import { Plus, Trash2, CheckCircle, XCircle } from 'lucide-vue-next';

const libraries = ref([]);
const newLib = ref({ name: '', protocol: 'local', base_path: '' });
const testResult = ref<boolean | null>(null);

const fetchLibraries = async () => {
  const res = await axios.get('/api/v1/libraries');
  libraries.value = res.data;
};

const testConnection = async () => {
  const res = await axios.post('/api/v1/libraries/test', newLib.value);
  testResult.value = res.data;
};

const addLibrary = async () => {
  await axios.post('/api/v1/libraries', newLib.value);
  newLib.value = { name: '', protocol: 'local', base_path: '' };
  testResult.value = null;
  fetchLibraries();
};

const removeLibrary = async (id: number) => {
  await axios.delete(`/api/v1/libraries/${id}`);
  fetchLibraries();
};

onMounted(fetchLibraries);
</script>

<template>
  <div class="p-8 max-w-4xl mx-auto">
    <h1 class="text-2xl font-bold mb-6">存储池管理</h1>

    <!-- 添加表单 -->
    <div class="bg-white p-6 rounded-lg shadow-sm border mb-8">
      <h2 class="text-lg font-semibold mb-4">添加新资源库</h2>
      <div class="grid grid-cols-2 gap-4 mb-4">
        <input v-model="newLib.name" placeholder="库名称 (如: 我的照片)" class="border p-2 rounded" />
        <select v-model="newLib.protocol" class="border p-2 rounded">
          <option value="local">本地目录</option>
          <option value="webdav">WebDAV</option>
        </select>
        <input v-model="newLib.base_path" placeholder="物理路径 (如: /mnt/photos)" class="border p-2 rounded col-span-2" />
      </div>
      <div class="flex items-center gap-4">
        <button @click="testConnection" class="bg-gray-100 px-4 py-2 rounded hover:bg-gray-200">测试连接</button>
        <button @click="addLibrary" class="bg-blue-600 text-white px-4 py-2 rounded hover:bg-blue-700">保存库</button>
        
        <span v-if="testResult === true" class="text-green-600 flex items-center text-sm">
          <CheckCircle class="w-4 h-4 mr-1"/> 路径有效
        </span>
        <span v-if="testResult === false" class="text-red-600 flex items-center text-sm">
          <XCircle class="w-4 h-4 mr-1"/> 路径无效或无法访问
        </span>
      </div>
    </div>

    <!-- 列表展示 -->
    <div class="space-y-4">
      <div v-for="lib in libraries" :key="lib.id" class="flex justify-between items-center bg-white p-4 rounded border">
        <div>
          <div class="font-bold">{{ lib.name }}</div>
          <div class="text-sm text-gray-500">{{ lib.protocol }}://{{ lib.base_path }}</div>
        </div>
        <button @click="removeLibrary(lib.id)" class="text-red-500 hover:bg-red-50 p-2 rounded">
          <Trash2 class="w-5 h-5" />
        </button>
      </div>
    </div>
  </div>
</template>
```

---

### 4. 动态触发扫描逻辑

当用户添加了一个新库后，我们需要在后端能够触发扫描。我们可以增加一个 API 接口来手动开启扫描任务。

```rust
// api/library.rs
pub async fn trigger_scan(
    State(pool): State<SqlitePool>,
    Path(id): Path<i32>
) -> StatusCode {
    // 1. 获取库配置
    let lib = sqlx::query_as!(Library, "SELECT * FROM libraries WHERE id = ?", id)
        .fetch_one(&pool).await.unwrap();

    // 2. 异步启动扫描 (不阻塞 API)
    let scanner = crate::engine::scanner::Scanner::new(pool);
    tokio::spawn(async move {
        let _ = scanner.scan_library(&lib).await;
    });

    StatusCode::ACCEPTED
}
```

---

### Milestone 7 总结

1.  **从静态到动态：** 摆脱了 `.env` 或代码中写死路径的束缚，系统现在可以管理多个不相关的文件夹。
2.  **安全性验证：** “测试连接”功能避免了因输入错误路径导致扫描引擎报错。
3.  **UI 闭环：** 用户现在可以完成“登录 -> 添加文件夹 -> 开始管理”的完整业务流。

---