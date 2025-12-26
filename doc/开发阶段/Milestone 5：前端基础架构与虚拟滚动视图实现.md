进入 **Milestone 5：前端基础架构与虚拟滚动视图实现**。

在这一阶段，我们将从后端转向前端。为了确保在处理数万个文件时界面依然流畅，我们将采用 **Vue 3** 企业级架构，并重点实现**虚拟滚动网格**。

### 1. 技术栈初始化

首先，我们需要创建一个基于 Vite 的 Vue 3 项目，并安装核心依赖。

```bash
# 创建项目
npm create vite@latest tagflow-ui -- --template vue-ts
cd tagflow-ui

# 安装依赖
npm install pinia axios lucide-vue-next
npm install -D tailwindcss postcss autoprefixer
npx tailwindcss init -p

# 安装虚拟滚动组件
npm install vue-virtual-scroller
```

### 2. 定义全局状态管理 (Pinia Store)

我们需要管理当前的选中标签、标签树以及文件列表。

在 `src/stores/useResourceStore.ts` 中：

```typescript
import { defineStore } from 'pinia';
import axios from 'axios';

export const useResourceStore = defineStore('resource', {
  state: () => ({
    tags: [] as any[],        // 标签树
    files: [] as any[],       // 当前视图下的文件
    selectedTagId: null as number | null,
    loading: false,
  }),
  actions: {
    async fetchTags() {
      const res = await axios.get('/api/v1/tags/tree');
      this.tags = res.data;
    },
    async fetchFiles(tagId?: number) {
      this.loading = true;
      this.selectedTagId = tagId || null;
      const res = await axios.get('/api/v1/files', {
        params: { tag_id: tagId, recursive: true }
      });
      this.files = res.data.items;
      this.loading = false;
    }
  }
});
```

### 3. 实现层级标签树组件 (TagTree)

这是一个递归组件，用于展现后端生成的层级标签逻辑。

在 `src/components/TagItem.vue` 中：

```vue
<script setup lang="ts">
import { ChevronRight, Folder } from 'lucide-vue-next';
const props = defineProps<{ node: any; depth: number }>();
const emit = defineEmits(['select']);
</script>

<template>
  <div class="select-none">
    <div 
      @click="emit('select', node.id)"
      class="flex items-center p-2 hover:bg-gray-100 cursor-pointer rounded"
      :style="{ paddingLeft: `${depth * 12 + 8}px` }"
    >
      <ChevronRight v-if="node.children?.length" class="w-4 h-4 mr-1 text-gray-400" />
      <Folder class="w-4 h-4 mr-2 text-blue-500" />
      <span class="text-sm">{{ node.name }}</span>
    </div>
    
    <div v-if="node.children?.length">
      <TagItem 
        v-for="child in node.children" 
        :key="child.id" 
        :node="child" 
        :depth="depth + 1"
        @select="(id) => emit('select', id)"
      />
    </div>
  </div>
</template>
```

### 4. 实现虚拟滚动网格 (FileGrid)

这是前端最核心的部分。我们使用 `vue-virtual-scroller` 来渲染网格，它只会渲染用户视口内的那一小部分 DOM，即便文件总数有 10 万个。

在 `src/components/FileGrid.vue` 中：

```vue
<script setup lang="ts">
import { RecycleScroller } from 'vue-virtual-scroller';
import 'vue-virtual-scroller/dist/vue-virtual-scroller.css';
import { FileText, Image as ImageIcon } from 'lucide-vue-next';

defineProps<{ files: any[] }>();

// 假设网格每行显示 6 个，每个高度 160px
const GRID_COLUMNS = 6;
const ITEM_HEIGHT = 160;

// 将扁平数组转化为行数组，适配网格渲染
const computedRows = (items: any[]) => {
  const rows = [];
  for (let i = 0; i < items.length; i += GRID_COLUMNS) {
    rows.push({ id: i, items: items.slice(i, i + GRID_COLUMNS) });
  }
  return rows;
};
</script>

<template>
  <RecycleScroller
    class="h-full w-full"
    :items="computedRows(files)"
    :item-size="ITEM_HEIGHT"
    key-field="id"
    v-slot="{ item }"
  >
    <div class="grid grid-cols-6 gap-4 p-4">
      <div v-for="file in item.items" :key="file.id" 
           class="flex flex-col items-center p-2 border rounded hover:shadow-md transition bg-white">
        <div class="w-24 h-24 flex items-center justify-center bg-gray-50 rounded mb-2">
          <ImageIcon v-if="file.extension === 'jpg'" class="text-green-500 w-12 h-12" />
          <FileText v-else class="text-gray-400 w-12 h-12" />
        </div>
        <span class="text-xs truncate w-full text-center px-2">{{ file.filename }}</span>
      </div>
    </div>
  </RecycleScroller>
</template>
```

### 5. 主页面布局 (App.vue)

将侧边栏和主视图组合在一起。

```vue
<script setup lang="ts">
import { onMounted } from 'vue';
import { useResourceStore } from './stores/useResourceStore';
import TagItem from './components/TagItem.vue';
import FileGrid from './components/FileGrid.vue';

const store = useResourceStore();

onMounted(() => {
  store.fetchTags();
  store.fetchFiles(); // 初始加载全部
});
</script>

<template>
  <div class="flex h-screen bg-gray-50 text-gray-900 overflow-hidden">
    <!-- 左侧侧边栏 -->
    <aside class="w-64 border-r bg-white flex flex-col">
      <div class="p-4 font-bold text-xl border-b text-blue-600">TagFlow</div>
      <div class="flex-1 overflow-y-auto p-2">
        <TagItem 
          v-for="tag in store.tags" 
          :key="tag.id" 
          :node="tag" 
          :depth="0" 
          @select="(id) => store.fetchFiles(id)"
        />
      </div>
    </aside>

    <!-- 右侧主区域 -->
    <main class="flex-1 flex flex-col min-w-0">
      <header class="h-14 border-b bg-white flex items-center px-6 justify-between">
        <div class="text-sm font-medium text-gray-500">
          当前查看: <span class="text-gray-900">{{ store.selectedTagId || '全部文件' }}</span>
        </div>
        <div v-if="store.loading" class="animate-spin rounded-full h-4 w-4 border-2 border-blue-500 border-t-transparent"></div>
      </header>
      
      <section class="flex-1 overflow-hidden">
        <FileGrid :files="store.files" />
      </section>
    </main>
  </div>
</template>
```

### 6. Milestone 5 交付点

1.  **现代化 UI：** 使用 TailwindCSS 构建了典型的资产管理系统布局。
2.  **高性能网格：** 引入虚拟滚动技术，解决了海量文件在浏览器端的 DOM 堆积问题。
3.  **交互闭环：** 点击左侧层级标签，右侧利用递归查询实时更新文件视图。

---

### 下一步建议

**Milestone 6：异步任务流水线与缩略图生成。**

目前我们的界面只有图标，没有预览。
1.  **后台 Worker：** 在 Rust 后端实现一个消费 `tasks` 表的任务执行器。
2.  **FFmpeg 集成：** 调用 FFmpeg 提取视频第一帧或压缩图片。
3.  **前端预览：** 修改 `FileGrid` 渲染逻辑，展示生成的 WebP 缩略图。

**是否继续进行 Milestone 6 的开发？**