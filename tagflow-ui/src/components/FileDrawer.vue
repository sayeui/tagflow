<script setup lang="ts">
import { computed, onMounted, onUnmounted, ref, watch } from 'vue'
import MarkdownIt from 'markdown-it'
// @ts-expect-error: DynamicScroller/DynamicScrollerItem 运行时存在，但 vue-virtual-scroller beta.8 的 .d.ts 未导出
import { DynamicScroller, DynamicScrollerItem } from 'vue-virtual-scroller'
import 'vue-virtual-scroller/dist/vue-virtual-scroller.css'
import { X, Download, FileText } from 'lucide-vue-next'
import { useResourceStore } from '@/stores/useResourceStore'
import { fileApi } from '@/api/http'

const store = useResourceStore()

// markdown-it：html:false 禁掉原始 HTML 直通，免 XSS（无需 DOMPurify）
const md = new MarkdownIt({ html: false, linkify: true, breaks: true })

const open = computed(() => store.selectedFileId !== null)
const detail = computed(() => store.fileDetail)

// ===== 类型分流（与后端 is_text/content_type 对齐）=====
const MD_EXTS = ['md', 'markdown']
const TEXT_EXTS = [
  'txt', 'log', 'csv', 'tsv', 'json', 'xml', 'html', 'htm', 'yaml', 'yml',
  'ini', 'conf', 'toml', 'js', 'ts', 'css', 'scss', 'py', 'rs', 'go', 'java',
  'c', 'cpp', 'h', 'hpp', 'sh', 'bat', 'sql', 'vue', 'srt', 'vtt',
]
const IMG_EXTS = ['jpg', 'jpeg', 'png', 'gif', 'webp', 'bmp', 'svg']
const VIDEO_EXTS = ['mp4', 'webm', 'mkv', 'mov', 'avi']
const AUDIO_EXTS = ['mp3', 'wav', 'ogg', 'flac', 'aac', 'm4a']

type Kind = 'text' | 'markdown' | 'pdf' | 'image' | 'video' | 'audio' | 'unknown'
const kind = computed<Kind>(() => {
  const e = (detail.value?.extension ?? '').toLowerCase()
  if (MD_EXTS.includes(e)) return 'markdown'
  if (TEXT_EXTS.includes(e)) return 'text'
  if (e === 'pdf') return 'pdf'
  if (IMG_EXTS.includes(e)) return 'image'
  if (VIDEO_EXTS.includes(e)) return 'video'
  if (AUDIO_EXTS.includes(e)) return 'audio'
  return 'unknown'
})

// ===== 文本内容加载（detail 变化时触发，覆盖打开/切换）=====
// 用 textLoaded 而非 textContent 真值判断"已加载"，否则空文件（textContent=''）
// 会落到所有 v-else-if 之外什么都不渲染。
const textContent = ref('')
const textLoading = ref(false)
const textLoaded = ref(false)
const textError = ref('')

const textLines = computed(() =>
  textContent.value.split('\n').map((text, i) => ({ id: i, text })),
)
const renderedMd = computed(() => md.render(textContent.value))

watch(
  () => store.fileDetail,
  async (d) => {
    textContent.value = ''
    textLoaded.value = false
    textError.value = ''
    if (!d) return
    const e = (d.extension ?? '').toLowerCase()
    if (MD_EXTS.includes(e) || TEXT_EXTS.includes(e)) {
      textLoading.value = true
      try {
        textContent.value = await fileApi.contentText(d.id)
        textLoaded.value = true
      } catch {
        textError.value = '内容加载失败'
      } finally {
        textLoading.value = false
      }
    }
  },
)

// ===== 媒体 URL（带 token）=====
const contentSrc = computed(() =>
  store.selectedFileId !== null ? fileApi.contentUrl(store.selectedFileId) : '',
)
const downloadUrl = computed(() =>
  store.selectedFileId !== null
    ? fileApi.contentUrl(store.selectedFileId, { download: true })
    : '',
)

// 图片全屏
const fullscreen = ref(false)

// ===== Esc 关闭 =====
function onKey(e: KeyboardEvent) {
  if (e.key === 'Escape') {
    if (fullscreen.value) fullscreen.value = false
    else if (open.value) store.closeFile()
  }
}
onMounted(() => window.addEventListener('keydown', onKey))
onUnmounted(() => window.removeEventListener('keydown', onKey))

// ===== 格式化 =====
function formatSize(bytes: number): string {
  if (!bytes) return '0 B'
  const k = 1024
  const sizes = ['B', 'KB', 'MB', 'GB', 'TB']
  const i = Math.min(Math.floor(Math.log(bytes) / Math.log(k)), sizes.length - 1)
  return parseFloat((bytes / Math.pow(k, i)).toFixed(1)) + ' ' + sizes[i]
}
function formatDate(ts: number): string {
  return new Date(ts * 1000).toLocaleString('zh-CN', {
    year: 'numeric',
    month: '2-digit',
    day: '2-digit',
    hour: '2-digit',
    minute: '2-digit',
  })
}
const TAG_STYLE: Record<string, string> = {
  type: 'bg-purple-50 text-purple-700',
  ext: 'bg-blue-50 text-blue-700',
  path: 'bg-emerald-50 text-emerald-700',
  time: 'bg-amber-50 text-amber-700',
  user: 'bg-pink-50 text-pink-700',
}
const tagClass = (cat: string) => TAG_STYLE[cat] ?? 'bg-gray-100 text-gray-600'

// ===== 手动标签：添加 / 移除 =====
const newTagPath = ref('')
const tagAdding = ref(false)
const tagError = ref('')

async function addTag() {
  const path = newTagPath.value.trim()
  tagError.value = ''
  if (!path || tagAdding.value) return
  tagAdding.value = true
  try {
    await store.addTagToFile(path)
    newTagPath.value = ''
  } catch {
    tagError.value = '添加失败：路径为空、非法或服务端错误'
  } finally {
    tagAdding.value = false
  }
}

async function removeTag(tagId: number) {
  tagError.value = ''
  try {
    await store.removeTagFromFile(tagId)
  } catch {
    tagError.value = '移除失败，请重试'
  }
}</script>

<template>
  <Transition name="drawer">
    <div v-if="open" class="fixed inset-0 z-50 flex justify-end">
      <!-- 遮罩（点击关闭） -->
      <div class="absolute inset-0 bg-black/40" @click="store.closeFile()"></div>

      <!-- 抽屉面板 -->
      <div
        class="drawer-panel relative z-10 h-full w-full max-w-2xl bg-white shadow-2xl flex flex-col"
      >
        <!-- 头部 -->
        <header class="flex items-center justify-between px-6 py-4 border-b border-gray-200 shrink-0">
          <h2 class="font-semibold text-gray-900 truncate" :title="detail?.filename">
            {{ detail?.filename ?? '加载中…' }}
          </h2>
          <button
            @click="store.closeFile()"
            class="p-2 -mr-2 text-gray-400 hover:text-gray-700 hover:bg-gray-100 rounded-lg transition-colors"
            title="关闭 (Esc)"
          >
            <X class="w-5 h-5" />
          </button>
        </header>

        <!-- 元数据 + 标签 -->
        <div v-if="detail" class="px-6 py-3 border-b border-gray-200 shrink-0 space-y-2">
          <div class="grid grid-cols-2 gap-x-6 gap-y-1 text-sm text-gray-600">
            <div><span class="text-gray-400">类型：</span>{{ detail.extension ?? '—' }}</div>
            <div><span class="text-gray-400">大小：</span>{{ formatSize(detail.size) }}</div>
            <div><span class="text-gray-400">修改：</span>{{ formatDate(detail.mtime) }}</div>
            <div><span class="text-gray-400">路径：</span><span class="truncate inline-block max-w-[16rem] align-bottom">{{ detail.parent_path || '/' }}</span></div>
          </div>
          <div class="flex flex-wrap items-center gap-1.5 pt-1">
            <span
              v-for="t in detail.tags"
              :key="t.id"
              class="inline-flex items-center px-2 py-0.5 rounded text-xs font-medium"
              :class="tagClass(t.category)"
            >
              {{ t.category }}:{{ t.name }}
              <!-- 仅手动标签可移除（source === 'manual'），自动标签受保护 -->
              <button
                v-if="t.source === 'manual'"
                @click="removeTag(t.id)"
                class="ml-1 -mr-0.5 opacity-50 hover:opacity-100 transition-opacity"
                title="移除标签"
              >
                <X class="w-3 h-3" />
              </button>
            </span>
            <input
              v-model="newTagPath"
              @keydown.enter.prevent="addTag"
              placeholder="添加标签 / 可用 / 分级"
              :disabled="tagAdding"
              class="px-2 py-0.5 text-xs rounded border border-dashed border-gray-300 focus:border-blue-400 focus:outline-none w-48"
            />
          </div>
          <p v-if="tagError" class="text-xs text-red-500 pt-1">{{ tagError }}</p>
        </div>

        <!-- 预览区 -->
        <div class="flex-1 min-h-0 bg-gray-50">
          <!-- detail 加载中 -->
          <div v-if="!detail" class="h-full flex items-center justify-center text-gray-400">
            加载中…
          </div>

          <!-- 文本（虚拟滚动；textLoaded 容许空文件也渲染空白行） -->
          <DynamicScroller
            v-else-if="kind === 'text' && textLoaded && !textError"
            :items="textLines"
            :min-item-size="24"
            key-field="id"
            class="h-full"
          >
            <template #default="{ item, index, active }">
              <DynamicScrollerItem :item="item" :active="active" :data-index="index">
                <div class="px-6 py-0.5 whitespace-pre-wrap break-words text-sm leading-loose text-gray-800 font-sans">
                  {{ item.text || ' ' }}
                </div>
              </DynamicScrollerItem>
            </template>
          </DynamicScroller>

          <!-- Markdown -->
          <div
            v-else-if="kind === 'markdown' && textLoaded && !textError"
            class="h-full overflow-auto px-8 py-6 markdown-body"
            v-html="renderedMd"
          ></div>

          <!-- PDF（浏览器原生） -->
          <iframe
            v-else-if="kind === 'pdf'"
            :src="contentSrc"
            :title="detail?.filename ?? 'pdf'"
            class="w-full h-full bg-white"
          ></iframe>

          <!-- 图片 -->
          <div
            v-else-if="kind === 'image'"
            class="h-full overflow-auto flex items-center justify-center p-6"
          >
            <img
              :src="contentSrc"
              :alt="detail?.filename ?? ''"
              class="max-w-full max-h-full object-contain cursor-zoom-in shadow-sm"
              @click="fullscreen = true"
            />
          </div>

          <!-- 视频 -->
          <div
            v-else-if="kind === 'video'"
            class="h-full flex items-center justify-center bg-black p-4"
          >
            <video :src="contentSrc" controls preload="metadata" class="max-w-full max-h-full"></video>
          </div>

          <!-- 音频 -->
          <div
            v-else-if="kind === 'audio'"
            class="h-full flex flex-col items-center justify-center gap-4 p-6 text-gray-400"
          >
            <FileText class="w-16 h-16 opacity-40" />
            <audio :src="contentSrc" controls class="w-full max-w-md"></audio>
          </div>

          <!-- 不支持的类型 -->
          <div
            v-else-if="kind === 'unknown'"
            class="h-full flex flex-col items-center justify-center gap-2 text-gray-400"
          >
            <FileText class="w-16 h-16 opacity-40" />
            <p class="text-sm">该类型暂不支持在线预览</p>
            <p class="text-xs">请点击下方「下载」查看</p>
          </div>

          <!-- 文本读取中 -->
          <div
            v-else-if="(kind === 'text' || kind === 'markdown') && textLoading"
            class="h-full flex items-center justify-center text-gray-400"
          >
            <svg
              class="animate-spin h-4 w-4 border-2 border-gray-300 border-t-blue-500 rounded-full"
              viewBox="0 0 24 24"
            ></svg>
            <span class="ml-2 text-sm">读取中…</span>
          </div>

          <!-- 文本读取失败 -->
          <div
            v-else-if="textError"
            class="h-full flex items-center justify-center text-red-400 text-sm"
          >
            {{ textError }}
          </div>
        </div>

        <!-- 底部：下载 -->
        <footer
          v-if="detail"
          class="px-6 py-3 border-t border-gray-200 flex justify-end shrink-0 bg-white"
        >
          <a
            :href="downloadUrl"
            class="inline-flex items-center px-4 py-2 bg-blue-600 text-white text-sm font-medium rounded-lg hover:bg-blue-700 transition-colors"
          >
            <Download class="w-4 h-4 mr-1.5" />
            下载
          </a>
        </footer>
      </div>

      <!-- 图片全屏 -->
      <Transition name="fade">
        <div
          v-if="fullscreen"
          class="fixed inset-0 z-[60] bg-black/90 flex items-center justify-center"
          @click="fullscreen = false"
        >
          <img
            :src="contentSrc"
            :alt="detail?.filename ?? ''"
            class="max-w-[95vw] max-h-[95vh] object-contain"
          />
        </div>
      </Transition>
    </div>
  </Transition>
</template>

<style>
/* 抽屉滑入/滑出：遮罩 opacity + 面板 translateX */
.drawer-enter-from,
.drawer-leave-to {
  opacity: 0;
}
.drawer-enter-from .drawer-panel,
.drawer-leave-to .drawer-panel {
  transform: translateX(100%);
}
.drawer-enter-active,
.drawer-leave-active {
  transition: opacity 0.28s ease;
}
.drawer-enter-active .drawer-panel,
.drawer-leave-active .drawer-panel {
  transition: transform 0.32s cubic-bezier(0.4, 0, 0.2, 1);
}

/* 图片全屏淡入 */
.fade-enter-active,
.fade-leave-active {
  transition: opacity 0.2s ease;
}
.fade-enter-from,
.fade-leave-to {
  opacity: 0;
}

/* Markdown 渲染样式（v-html 注入，需全局 / 非 scoped） */
.markdown-body {
  font-size: 14px;
  line-height: 1.7;
  color: #1f2937;
}
.markdown-body h1 {
  font-size: 1.5em;
  font-weight: 600;
  margin: 0.6em 0 0.4em;
}
.markdown-body h2 {
  font-size: 1.3em;
  font-weight: 600;
  margin: 0.6em 0 0.4em;
}
.markdown-body h3 {
  font-size: 1.15em;
  font-weight: 600;
  margin: 0.5em 0 0.3em;
}
.markdown-body p {
  margin: 0.5em 0;
}
.markdown-body ul,
.markdown-body ol {
  margin: 0.5em 0;
  padding-left: 1.5em;
}
.markdown-body li {
  margin: 0.2em 0;
}
.markdown-body code {
  background: #f3f4f6;
  padding: 0.1em 0.3em;
  border-radius: 3px;
  font-family: ui-monospace, SFMono-Regular, monospace;
  font-size: 0.9em;
}
.markdown-body pre {
  background: #1f2937;
  color: #e5e7eb;
  padding: 0.8em;
  border-radius: 6px;
  overflow-x: auto;
  margin: 0.6em 0;
}
.markdown-body pre code {
  background: none;
  padding: 0;
  color: inherit;
}
.markdown-body blockquote {
  border-left: 3px solid #d1d5db;
  padding-left: 1em;
  color: #6b7280;
  margin: 0.6em 0;
}
.markdown-body a {
  color: #2563eb;
  text-decoration: underline;
}
.markdown-body table {
  border-collapse: collapse;
  margin: 0.6em 0;
}
.markdown-body th,
.markdown-body td {
  border: 1px solid #e5e7eb;
  padding: 0.3em 0.6em;
}
.markdown-body hr {
  border: none;
  border-top: 1px solid #e5e7eb;
  margin: 1em 0;
}
</style>
