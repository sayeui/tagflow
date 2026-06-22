<script setup lang="ts">
import { onMounted, onUnmounted, ref, watch } from 'vue'
import { useResourceStore } from '@/stores/useResourceStore'
import TagItem from '@/components/TagItem.vue'
import FileGrid from '@/components/FileGrid.vue'
import FileList from '@/components/FileList.vue'
import FileDrawer from '@/components/FileDrawer.vue'
import { FolderOpen, Settings, X, LayoutGrid, List, Search } from 'lucide-vue-next'

const store = useResourceStore()

// 视图偏好持久化（grid / list）。key 集中在此一处，不在他处读写。
const VIEW_MODE_KEY = 'tagflow.viewMode'
type ViewMode = 'grid' | 'list'
const viewMode = ref<ViewMode>(
  (localStorage.getItem(VIEW_MODE_KEY) as ViewMode) === 'list' ? 'list' : 'grid',
)
const setViewMode = (mode: ViewMode) => {
  viewMode.value = mode
  localStorage.setItem(VIEW_MODE_KEY, mode)
}

// 文件名搜索：300ms 防抖。输入变化 → 写 keyword → 重置分页重拉。
// 用 watch + setTimeout 手写防抖（项目未引入 lodash）。
const keywordInput = ref(store.keyword)
let debounceTimer: ReturnType<typeof setTimeout> | null = null
watch(keywordInput, (val) => {
  if (debounceTimer) clearTimeout(debounceTimer)
  debounceTimer = setTimeout(() => {
    store.setKeyword(val)
  }, 300)
})

// 视图触底 → 触发下一页（store 内有 hasMore/loading 守卫）
const onReachBottom = () => {
  store.fetchMore()
}

onMounted(() => {
  store.fetchTags()
  store.fetchFiles()
})

// 卸载时清理防抖定时器，避免离开页面后滞后触发 store.setKeyword（陈旧请求/泄漏）
onUnmounted(() => {
  if (debounceTimer) {
    clearTimeout(debounceTimer)
    debounceTimer = null
  }
})
</script>

<template>
  <div class="flex h-[calc(100vh-3.5rem)] bg-gray-50 text-gray-900 overflow-hidden">
    <!-- 左侧侧边栏：按 category 分区的多选标签树 -->
    <aside class="w-64 border-r border-gray-200 bg-white flex flex-col">
      <div class="p-4">
        <button
          data-testid="all-files-button"
          @click="store.clearSelection()"
          class="w-full text-left px-3 py-2 rounded-lg hover:bg-gray-100 transition-colors text-sm font-medium text-gray-700 flex items-center"
          :class="{ 'bg-blue-50 text-blue-600': store.selectedTagIds.length === 0 }"
        >
          <FolderOpen class="w-4 h-4 mr-2" />
          全部文件
        </button>
      </div>

      <div data-testid="tag-tree" class="flex-1 overflow-y-auto px-2">
        <div v-for="group in store.groupedTags" :key="group.category" class="mb-3" :data-tag-category="group.category">
          <!-- 分组标题（容器，不可勾选） -->
          <div class="px-3 py-1 text-xs font-semibold text-gray-400 uppercase tracking-wide">
            {{ group.label }}
          </div>
          <!-- 组内根标签递归渲染 -->
          <TagItem
            v-for="tag in group.tags"
            :key="tag.id"
            :node="tag"
            :depth="0"
            :selected-ids="store.selectedTagIds"
            @toggle="(id) => store.toggleTag(id)"
          />
        </div>

        <div
          v-if="store.groupedTags.length === 0"
          class="px-4 py-8 text-center text-xs text-gray-400"
        >
          暂无标签
        </div>
      </div>

      <div class="p-4 border-t border-gray-200 text-xs text-gray-400">
        <div>共 {{ store.files.length }} / {{ store.total }} 个文件</div>
      </div>
    </aside>

    <!-- 右侧主区域 -->
    <main class="flex-1 flex flex-col min-w-0">
      <header class="h-14 border-b border-gray-200 bg-white flex items-center px-6 justify-between gap-4">
        <!-- 面包屑：当前过滤上下文（category:name 多标签以 ∧ 连接） -->
        <div class="text-sm flex items-center gap-2 min-w-0">
          <span class="text-gray-500 shrink-0">当前查看:</span>
          <template v-if="store.selectedTagLabels.length === 0">
            <span class="font-medium text-gray-900">全部文件</span>
          </template>
          <template v-else>
            <span
              v-for="t in store.selectedTagLabels"
              :key="t.id"
              class="inline-flex items-center px-2 py-0.5 rounded bg-blue-50 text-blue-700 text-xs font-medium"
            >
              {{ t.category }}:{{ t.name }}
              <button
                @click="store.toggleTag(t.id)"
                class="ml-1 hover:text-blue-900"
                title="取消该过滤"
              >
                <X class="w-3 h-3" />
              </button>
            </span>
          </template>
        </div>

        <div class="flex items-center gap-4 shrink-0">
          <!-- 文件名搜索框 -->
          <div class="relative">
            <Search class="w-4 h-4 absolute left-2.5 top-1/2 -translate-y-1/2 text-gray-400 pointer-events-none" />
            <input
              v-model="keywordInput"
              type="text"
              data-testid="search-input"
              placeholder="搜索文件名..."
              class="w-56 pl-8 pr-3 py-1.5 text-sm border border-gray-200 rounded-lg focus:outline-none focus:ring-2 focus:ring-blue-100 focus:border-blue-300 transition-colors"
            />
          </div>

          <!-- 加载指示 -->
          <div v-if="store.loading || store.loadingMore" class="flex items-center text-blue-500">
            <svg
              class="animate-spin rounded-full h-4 w-4 border-2 border-blue-500 border-t-transparent"
              viewBox="0 0 24 24"
            ></svg>
            <span class="ml-2 text-sm">{{ store.loadingMore ? '加载更多...' : '加载中...' }}</span>
          </div>

          <!-- 视图切换 -->
          <div
            class="flex items-center gap-1 border border-gray-200 rounded-lg p-0.5"
            data-testid="view-switcher"
          >
            <button
              @click="setViewMode('grid')"
              data-testid="view-grid-button"
              class="p-1.5 rounded transition-colors"
              :class="viewMode === 'grid' ? 'bg-blue-50 text-blue-600' : 'text-gray-500 hover:bg-gray-100'"
              title="卡片视图"
            >
              <LayoutGrid class="w-4 h-4" />
            </button>
            <button
              @click="setViewMode('list')"
              data-testid="view-list-button"
              class="p-1.5 rounded transition-colors"
              :class="viewMode === 'list' ? 'bg-blue-50 text-blue-600' : 'text-gray-500 hover:bg-gray-100'"
              title="列表视图"
            >
              <List class="w-4 h-4" />
            </button>
          </div>

          <!-- 设置按钮 -->
          <div class="flex items-center gap-2 border-l border-gray-200 pl-4">
            <router-link
              to="/settings/libraries"
              class="p-2 text-gray-600 hover:bg-gray-100 rounded-lg transition-colors"
              title="存储库管理"
            >
              <Settings class="w-5 h-5" />
            </router-link>
          </div>
        </div>
      </header>

      <section
        class="flex-1 overflow-hidden"
        :data-view-mode="viewMode"
        data-testid="file-area"
      >
        <FileGrid
          v-if="viewMode === 'grid'"
          :files="store.files"
          @reach-bottom="onReachBottom"
        />
        <FileList v-else :files="store.files" @reach-bottom="onReachBottom" />
      </section>
    </main>

    <!-- 文件详情/预览抽屉 -->
    <FileDrawer />
  </div>
</template>
