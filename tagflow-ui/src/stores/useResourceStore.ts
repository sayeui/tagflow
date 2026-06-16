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

/** 文件详情面板展示的单个标签（id + 名称 + 类别 + 来源）。
 *  source 区分 auto（扫描器自动）/ manual（用户手动），前端据此决定是否显示「×」移除按钮。 */
export interface FileTagInfo {
  id: number
  name: string
  category: string
  source: string
}

/** 文件详情（GET /api/v1/files/:id）：元数据 + 该文件全部标签。 */
export interface FileDetail {
  id: number
  filename: string
  extension: string | null
  size: number
  mtime: number
  parent_path: string
  tags: FileTagInfo[]
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
    // 文件详情抽屉
    selectedFileId: null as number | null,
    fileDetail: null as FileDetail | null,
    // 分页 / 无限滚动状态
    page: 1,
    pageSize: 50,
    total: 0,
    hasMore: false,
    loadingMore: false,
    keyword: '' as string,
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

    /** 设置搜索关键词并重置分页拉取（标签切换/清空/搜索框变化都走这个） */
    setKeyword(keyword: string) {
      this.keyword = keyword
      this.fetchFiles()
    },

    /** 重置分页后拉取并替换 files（用于标签切换/搜索变化/清空） */
    async fetchFiles() {
      this.loading = true
      this.page = 1
      try {
        const tagIds = this.selectedTagIds.length ? this.selectedTagIds : undefined
        const keyword = this.keyword.trim() || undefined
        const res = await fileApi.list({
          tag_ids: tagIds,
          recursive: true,
          page: this.page,
          limit: this.pageSize,
          keyword,
        })
        this.files = res.data.items
        this.total = res.data.total
        this.hasMore = this.files.length < this.total
      } catch (error) {
        console.error('Failed to fetch files:', error)
        throw error
      } finally {
        this.loading = false
      }
    },

    /** 追加下一页（无限滚动触底时调用）。
     *  守卫：hasMore && !loadingMore && !loading，防重复触发。 */
    async fetchMore() {
      if (!this.hasMore || this.loadingMore || this.loading) return
      this.loadingMore = true
      try {
        const nextPage = this.page + 1
        const tagIds = this.selectedTagIds.length ? this.selectedTagIds : undefined
        const keyword = this.keyword.trim() || undefined
        const res = await fileApi.list({
          tag_ids: tagIds,
          recursive: true,
          page: nextPage,
          limit: this.pageSize,
          keyword,
        })
        this.files.push(...res.data.items)
        this.page = nextPage
        this.total = res.data.total
        this.hasMore = this.files.length < this.total
      } catch (error) {
        console.error('Failed to fetch more files:', error)
        throw error
      } finally {
        this.loadingMore = false
      }
    },

    /** 打开文件详情抽屉：设置选中 id 并拉取详情（含标签） */
    async openFile(id: number) {
      this.selectedFileId = id
      this.fileDetail = null
      try {
        const res = await fileApi.detail(id)
        this.fileDetail = res.data
      } catch (error) {
        console.error('Failed to fetch file detail:', error)
      }
    },

    /** 关闭抽屉 */
    closeFile() {
      this.selectedFileId = null
      this.fileDetail = null
    },

    /** 给当前文件添加手动标签，成功后更新抽屉标签列表并刷新侧栏标签树
     *  （新 user 节点出现在「自定义」分区）。 */
    async addTagToFile(path: string) {
      if (this.selectedFileId === null) return
      try {
        const res = await fileApi.addTag(this.selectedFileId, path)
        if (this.fileDetail) this.fileDetail.tags = res.data
        await this.fetchTags()
      } catch (error) {
        console.error('Failed to add tag:', error)
        throw error
      }
    },

    /** 移除当前文件的手动标签，成功后更新抽屉标签列表并刷新侧栏
     *  （无引用的空 user 节点会被后端自动清理而消失）。 */
    async removeTagFromFile(tagId: number) {
      if (this.selectedFileId === null) return
      try {
        const res = await fileApi.removeTag(this.selectedFileId, tagId)
        if (this.fileDetail) this.fileDetail.tags = res.data
        await this.fetchTags()
      } catch (error) {
        console.error('Failed to remove tag:', error)
        throw error
      }
    },
  },
})
