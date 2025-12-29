<script setup lang="ts">
import { onMounted } from 'vue'
import { useResourceStore } from './stores/useResourceStore'
import TagItem from './components/TagItem.vue'
import FileGrid from './components/FileGrid.vue'
import { FolderOpen } from 'lucide-vue-next'

const store = useResourceStore()

onMounted(() => {
  store.fetchTags()
  store.fetchFiles()
})

const handleSelectAll = () => {
  store.fetchFiles()
}
</script>

<template>
  <div class="flex h-screen bg-gray-50 text-gray-900 overflow-hidden">
    <!-- 左侧侧边栏 -->
    <aside class="w-64 border-r border-gray-200 bg-white flex flex-col">
      <div class="p-4 font-bold text-xl border-b border-gray-200 text-blue-600 flex items-center">
        <FolderOpen class="w-6 h-6 mr-2" />
        TagFlow
      </div>

      <div class="p-2">
        <button
          @click="handleSelectAll"
          class="w-full text-left px-3 py-2 rounded-lg hover:bg-gray-100 transition-colors text-sm font-medium text-gray-700 flex items-center"
          :class="{ 'bg-blue-50 text-blue-600': store.selectedTagId === null }"
        >
          <FolderOpen class="w-4 h-4 mr-2" />
          全部文件
        </button>
      </div>

      <div class="flex-1 overflow-y-auto p-2">
        <TagItem
          v-for="tag in store.tags"
          :key="tag.id"
          :node="tag"
          :depth="0"
          @select="(id) => store.fetchFiles(id)"
        />
      </div>

      <div class="p-4 border-t border-gray-200 text-xs text-gray-400">
        <div>共 {{ store.files.length }} 个文件</div>
      </div>
    </aside>

    <!-- 右侧主区域 -->
    <main class="flex-1 flex flex-col min-w-0">
      <header class="h-14 border-b border-gray-200 bg-white flex items-center px-6 justify-between">
        <div class="text-sm">
          <span class="text-gray-500">当前查看:</span>
          <span class="ml-2 font-medium text-gray-900">
            {{ store.selectedTagName || '全部文件' }}
          </span>
        </div>
        <div v-if="store.loading" class="flex items-center text-blue-500">
          <svg class="animate-spin rounded-full h-4 w-4 border-2 border-blue-500 border-t-transparent" viewBox="0 0 24 24">
            <circle class="opacity-25" cx="12" cy="12" r="10" stroke="currentColor" stroke-width="4" fill="none" />
            <path class="opacity-75" fill="currentColor" d="M4 12a8 8 0 018-8V0C5.373 0 0 5.373 0 12h4zm2 5.291A7.962 7.962 0 014 12H0c0 3.042 1.135 5.824 3 7.938l3-2.647z" />
          </svg>
          <span class="ml-2 text-sm">加载中...</span>
        </div>
      </header>

      <section class="flex-1 overflow-hidden">
        <FileGrid :files="store.files" />
      </section>
    </main>
  </div>
</template>
