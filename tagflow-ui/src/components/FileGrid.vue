<script setup lang="ts">
import { RecycleScroller } from 'vue-virtual-scroller'
import 'vue-virtual-scroller/dist/vue-virtual-scroller.css'
import {
  FileText,
  Image as ImageIcon,
  FileCode,
  FileArchive,
} from 'lucide-vue-next'
import { fileApi } from '../api/http'
import { useResourceStore } from '../stores/useResourceStore'
import type { FileItem } from '../stores/useResourceStore'

defineProps<{ files: FileItem[] }>()
const emit = defineEmits<{ 'reach-bottom': [] }>()

const store = useResourceStore()

// 缩略图请求判断的媒体扩展名白名单。
// **必须与后端 `tagflow-core/src/engine/scanner/mod.rs` 的 `MEDIA_EXTENSIONS`
// 逐字完全一致**（single source of truth 在后端，此处是 cross-layer 契约镜像）：
//   - 后端只为这些扩展名入列缩略图任务（`is_media_extension` / scanner）；
//   - 前端按此判断是否渲染 thumbnail `<img>`，非媒体不渲染 → 不发请求；
//   - 改任一方都必须同步另一方，否则会出现一侧请求、一侧不生成 → 404 刷屏。
// 注意：**不含 `svg`** —— svg 在后端不入列缩略图任务，前端按图标分类走 ImageIcon，
// 不应请求缩略图（历史坑：曾把 svg 算进缩略图白名单，导致 404 刷屏）。
const MEDIA_EXTENSIONS = [
  // 图片
  'jpg', 'jpeg', 'png', 'gif', 'webp', 'bmp',
  // 视频
  'mp4', 'mov', 'm4v', 'mkv', 'avi', 'webm',
]

/** 判断扩展名是否在缩略图媒体白名单中（大小写不敏感，对齐后端 is_media_extension）。 */
const isMediaFile = (ext: string | null): boolean => {
  return !!ext && MEDIA_EXTENSIONS.includes(ext.toLowerCase())
}

// 卡片视图布局常量。
//
// RecycleScroller 用绝对定位排布每个虚拟 item，要求 `item-size` 严格等于
// 该 item 在垂直方向的「总占位高度」（含行间空白），否则后续 item 定位错乱
// → 上下行重叠/裁切。
//
// 卡片自然高度精确拆解（border + p-3 + 内容）：
//   - border 1px × 2 = 2px
//   - p-3 上下 = 24px
//   - 缩略图 h-24 = 96px
//   - mb-2 = 8px
//   - 文件名 text-xs 单行(truncate) ≈ 16px
//   - mt-1(4px) + 文件大小 text-xs ≈ 20px
//   - 合计 ≈ 166px
// 行容器高度 = 卡片高度(166) + 行间距(10) = 176px，卡片置顶，下方 10px 间距由
// 容器剩余空间承担（不在卡片内加 pt/pb 撑高，避免内容溢出固定行高）。
const GRID_COLUMNS = 6
const ROW_HEIGHT = 176

// 将扁平数组转化为行数组，适配网格渲染（每行 GRID_COLUMNS 个）。
const computedRows = (items: FileItem[]) => {
  const rows: Array<{ id: number; items: FileItem[] }> = []
  for (let i = 0; i < items.length; i += GRID_COLUMNS) {
    rows.push({ id: i, items: items.slice(i, i + GRID_COLUMNS) })
  }
  return rows
}

// 图标分类（getFileIcon）的扩展名白名单。
// **注意**：这里的 `imageExts`（含 svg）是「图标分类用途」，决定文件卡片显示哪种
// 图标（svg 显 ImageIcon）；与上面 `MEDIA_EXTENSIONS`（不含 svg，缩略图请求判断）
// 语义不同，**不要强行合并**——svg 在前端按图片显示图标，但后端不为 svg 生成缩略图，
// 若把 svg 并入 MEDIA_EXTENSIONS 会导致 svg 文件请求缩略图 404 刷屏。
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

// vue-virtual-scroller@2.0.0-beta.8 的 RecycleScroller 不发射 `scroll` 事件，
// 可用事件为 `scroll-start` / `scroll-end` / `update` / `resize` / `visible` / `hidden`。
// `scroll-end` 在「最后一个 item 进入可视区（被回收池新分配视图）」时触发，
// 正好对应"滚动到底部 → 加载下一页"语义。父组件 onReachBottom → store.fetchMore，
// 由 store 的 hasMore/loading 守卫防重复。
const onScrollEnd = () => {
  emit('reach-bottom')
}
</script>

<template>
  <RecycleScroller
    v-if="files.length > 0"
    class="h-full w-full"
    :items="computedRows(files)"
    :item-size="ROW_HEIGHT"
    key-field="id"
    @scroll-end="onScrollEnd"
    v-slot="{ item }"
  >
    <!-- 行容器：固定高度 = item-size(176px)；卡片自然高 ~166px，下方 ~10px 为行间距 -->
    <div class="h-[176px] grid grid-cols-6 gap-4 px-4 items-start">
      <div
        v-for="file in item.items"
        :key="file.id"
        data-testid="file-card"
        :data-filename="file.filename"
        :data-file-id="file.id"
        @click="store.openFile(file.id)"
        class="flex flex-col items-center p-3 border border-gray-200 rounded-lg hover:shadow-md hover:border-blue-300 transition-all cursor-pointer bg-white"
      >
        <!-- 缩略图容器 -->
        <div class="w-24 h-24 flex items-center justify-center bg-gray-50 rounded-lg mb-2 overflow-hidden relative shrink-0">
          <!-- 备用图标（缩略图加载前/失败时显示，z-0 下层） -->
          <component :is="getFileIcon(file.extension)" class="w-12 h-12 relative z-0" :class="{
            'text-green-500': getFileIcon(file.extension) === ImageIcon,
            'text-blue-500': getFileIcon(file.extension) === FileCode,
            'text-orange-500': getFileIcon(file.extension) === FileArchive,
            'text-gray-400': getFileIcon(file.extension) === FileText,
          }" />
          <!-- 缩略图：仅对媒体文件渲染（与后端 MEDIA_EXTENSIONS 一致）。
               非媒体文件后端永不生成缩略图 → 若渲染 <img> 会发请求 → 404 刷屏。
               opacity（而非 display:none）控制显隐：display:none 的 lazy img
               浏览器不加载，曾导致缩略图永远不显示。 -->
          <img
            v-if="isMediaFile(file.extension)"
            :src="fileApi.thumbnailUrl(file.id)"
            :alt="file.filename"
            class="absolute inset-0 w-full h-full object-cover z-10 transition-opacity duration-200"
            @error="(e) => { (e.target as HTMLImageElement).style.opacity = '0' }"
            @load="(e) => { (e.target as HTMLImageElement).style.opacity = '1' }"
            loading="lazy"
            style="opacity: 0"
          />
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
