<script setup lang="ts">
import { RecycleScroller } from 'vue-virtual-scroller'
import 'vue-virtual-scroller/dist/vue-virtual-scroller.css'
import {
  FileText,
  Image as ImageIcon,
  FileCode,
  FileArchive,
} from 'lucide-vue-next'
import { useResourceStore } from '../stores/useResourceStore'
import type { FileItem } from '../stores/useResourceStore'

defineProps<{ files: FileItem[] }>()
const emit = defineEmits<{ 'reach-bottom': [] }>()

const store = useResourceStore()

// 列表视图：每行一个文件，单行高度严格对齐 item-size。
// 行内 DOM：p-3(上下12=24) + 单行文本(约20px 行高) ≈ 44px；容器固定 h-12(48px)，
// item-size 同步设为 48，保证虚拟定位不错乱。
const ROW_HEIGHT = 48

const getFileIcon = (extension: string | null) => {
  if (!extension) return FileText
  const ext = extension.toLowerCase()
  const imageExts = ['jpg', 'jpeg', 'png', 'gif', 'svg', 'webp', 'bmp']
  const codeExts = ['js', 'ts', 'vue', 'py', 'rs', 'go', 'java', 'c', 'cpp', 'h', 'css', 'html', 'json']
  const archiveExts = ['zip', 'rar', '7z', 'tar', 'gz']
  if (imageExts.includes(ext)) return ImageIcon
  if (codeExts.includes(ext)) return FileCode
  if (archiveExts.includes(ext)) return FileArchive
  return FileText
}

const formatFileSize = (bytes: number): string => {
  if (bytes === 0) return '0 B'
  const k = 1024
  const sizes = ['B', 'KB', 'MB', 'GB', 'TB']
  const i = Math.min(Math.floor(Math.log(bytes) / Math.log(k)), sizes.length - 1)
  return parseFloat((bytes / Math.pow(k, i)).toFixed(1)) + ' ' + sizes[i]
}

const formatDate = (ts: number): string =>
  new Date(ts * 1000).toLocaleString('zh-CN', {
    year: 'numeric',
    month: '2-digit',
    day: '2-digit',
    hour: '2-digit',
    minute: '2-digit',
  })

// 见 FileGrid.vue 同名注释：beta.8 用 `scroll-end` 事件而非 `scroll`，
// 在最后一个 item 进入可视区时触发，对应"触底加载下一页"。
const onScrollEnd = () => {
  emit('reach-bottom')
}
</script>

<template>
  <div class="h-full w-full flex flex-col">
    <!-- 表头：固定，不参与虚拟滚动 -->
    <div
      class="grid grid-cols-[1fr_120px_160px] gap-4 px-6 py-2 border-b border-gray-200 bg-gray-50 text-xs font-semibold text-gray-500 uppercase tracking-wide"
    >
      <span>文件名</span>
      <span class="text-right">大小</span>
      <span class="text-right">修改时间</span>
    </div>

    <RecycleScroller
      v-if="files.length > 0"
      class="flex-1 min-h-0"
      :items="files"
      :item-size="ROW_HEIGHT"
      key-field="id"
      @scroll-end="onScrollEnd"
      v-slot="{ item }"
    >
      <div
        data-testid="file-card"
        :data-filename="item.filename"
        :data-file-id="item.id"
        @click="store.openFile(item.id)"
        class="h-12 grid grid-cols-[1fr_120px_160px] gap-4 items-center px-6 border-b border-gray-100 hover:bg-blue-50 transition-colors cursor-pointer"
      >
        <div class="flex items-center min-w-0">
          <component
            :is="getFileIcon(item.extension)"
            class="w-4 h-4 mr-2 shrink-0"
            :class="{
              'text-green-500': getFileIcon(item.extension) === ImageIcon,
              'text-blue-500': getFileIcon(item.extension) === FileCode,
              'text-orange-500': getFileIcon(item.extension) === FileArchive,
              'text-gray-400': getFileIcon(item.extension) === FileText,
            }"
          />
          <span class="text-sm text-gray-800 truncate" :title="item.filename">{{ item.filename }}</span>
        </div>
        <span class="text-sm text-gray-500 text-right tabular-nums">{{ formatFileSize(item.size) }}</span>
        <span class="text-sm text-gray-400 text-right tabular-nums">{{ formatDate(item.mtime) }}</span>
      </div>
    </RecycleScroller>

    <div v-else class="flex-1 flex items-center justify-center text-gray-400">
      <div class="text-center">
        <FileText class="w-16 h-16 mx-auto mb-4 opacity-50" />
        <p>暂无文件</p>
      </div>
    </div>
  </div>
</template>
