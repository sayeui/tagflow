<script setup lang="ts">
import { RecycleScroller } from 'vue-virtual-scroller'
import 'vue-virtual-scroller/dist/vue-virtual-scroller.css'
import { FileText, Image as ImageIcon, FileCode, FileArchive } from 'lucide-vue-next'
import type { FileItem } from '../stores/useResourceStore'

defineProps<{ files: FileItem[] }>()

// 假设网格每行显示 6 个，每个高度 160px
const GRID_COLUMNS = 6
const ITEM_HEIGHT = 160

// 将扁平数组转化为行数组，适配网格渲染
const computedRows = (items: FileItem[]) => {
  const rows: Array<{ id: number; items: FileItem[] }> = []
  for (let i = 0; i < items.length; i += GRID_COLUMNS) {
    rows.push({ id: i, items: items.slice(i, i + GRID_COLUMNS) })
  }
  return rows
}

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
  const sizes = ['B', 'KB', 'MB', 'GB']
  const i = Math.floor(Math.log(bytes) / Math.log(k))
  return parseFloat((bytes / Math.pow(k, i)).toFixed(1)) + ' ' + sizes[i]
}
</script>

<template>
  <RecycleScroller
    v-if="files.length > 0"
    class="h-full w-full"
    :items="computedRows(files)"
    :item-size="ITEM_HEIGHT"
    key-field="id"
    v-slot="{ item }"
  >
    <div class="grid grid-cols-6 gap-4 px-4">
      <div
        v-for="file in item.items"
        :key="file.id"
        class="flex flex-col items-center p-3 border border-gray-200 rounded-lg hover:shadow-md hover:border-blue-300 transition-all cursor-pointer bg-white"
      >
        <div class="w-24 h-24 flex items-center justify-center bg-gray-50 rounded-lg mb-2">
          <component :is="getFileIcon(file.extension)" class="w-12 h-12" :class="{
            'text-green-500': getFileIcon(file.extension) === ImageIcon,
            'text-blue-500': getFileIcon(file.extension) === FileCode,
            'text-orange-500': getFileIcon(file.extension) === FileArchive,
            'text-gray-400': getFileIcon(file.extension) === FileText,
          }" />
        </div>
        <span class="text-xs text-gray-700 truncate w-full text-center px-1" :title="file.filename">
          {{ file.filename }}
        </span>
        <span class="text-xs text-gray-400 mt-1">{{ formatFileSize(file.size) }}</span>
      </div>
    </div>
  </RecycleScroller>

  <div v-else class="flex items-center justify-center h-full text-gray-400">
    <div class="text-center">
      <FileText class="w-16 h-16 mx-auto mb-4 opacity-50" />
      <p>暂无文件</p>
    </div>
  </div>
</template>
