import { defineStore } from 'pinia'
import { tagApi, fileApi } from '@/api/http'

export interface TagNode {
  id: number
  name: string
  category: string
  children: TagNode[]
}

export interface FileItem {
  id: number
  filename: string
  extension: string | null
  size: number
  mtime: number
  parent_path: string
}

/** 分组显示顺序与中文标签 */
const CATEGORY_ORDER = ['type', 'ext', 'path', 'time', 'user'] as const
const CATEGORY_LABELS: Record<string, string> = {
  type: '类型',
  ext: '扩展名',
  path: '路径',
  time: '时间',
  user: '自定义',
}

export interface TagGroup {
  category: string
  label: string
  tags: TagNode[]
}

export const useResourceStore = defineStore('resource', {
  state: () => ({
    tags: [] as TagNode[],
    files: [] as FileItem[],
    selectedTagIds: [] as number[],
    loading: false,
  }),

  getters: {
    /** 按 category 分组根标签，用于侧栏分区渲染 */
    groupedTags(state): TagGroup[] {
      const buckets = new Map<string, TagNode[]>()
      for (const tag of state.tags) {
        const cat = tag.category || 'other'
        if (!buckets.has(cat)) buckets.set(cat, [])
        buckets.get(cat)!.push(tag)
      }
      return CATEGORY_ORDER.filter((c) => buckets.has(c)).map((c) => ({
        category: c,
        label: CATEGORY_LABELS[c] || c,
        tags: buckets.get(c)!,
      }))
    },

    /** 当前选中标签的展示信息（id/name/category），供面包屑渲染 */
    selectedTagLabels(state): Array<{ id: number; name: string; category: string }> {
      const wanted = new Set(state.selectedTagIds)
      const found: Array<{ id: number; name: string; category: string }> = []
      const walk = (nodes: TagNode[]) => {
        for (const n of nodes) {
          if (wanted.has(n.id)) found.push(n)
          if (n.children?.length) walk(n.children)
        }
      }
      walk(state.tags)
      return found
    },
  },

  actions: {
    async fetchTags() {
      try {
        const res = await tagApi.getTree()
        this.tags = res.data
      } catch (error) {
        console.error('Failed to fetch tags:', error)
        throw error
      }
    },

    /** 勾选/取消勾选一个标签，随后按当前选中集合（AND）重新拉取文件 */
    toggleTag(tagId: number) {
      const idx = this.selectedTagIds.indexOf(tagId)
      if (idx >= 0) {
        this.selectedTagIds.splice(idx, 1)
      } else {
        this.selectedTagIds.push(tagId)
      }
      this.fetchFiles()
    },

    /** 清空选择，查看全部文件 */
    clearSelection() {
      this.selectedTagIds = []
      this.fetchFiles()
    },

    async fetchFiles() {
      this.loading = true
      try {
        const tagIds = this.selectedTagIds.length ? this.selectedTagIds : undefined
        const res = await fileApi.list({ tag_ids: tagIds, recursive: true })
        this.files = res.data.items
      } catch (error) {
        console.error('Failed to fetch files:', error)
        throw error
      } finally {
        this.loading = false
      }
    },
  },
})
