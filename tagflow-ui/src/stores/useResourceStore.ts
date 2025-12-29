import { defineStore } from 'pinia'
import axios from 'axios'

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

export const useResourceStore = defineStore('resource', {
  state: () => ({
    tags: [] as TagNode[],
    files: [] as FileItem[],
    selectedTagId: null as number | null,
    selectedTagName: '' as string,
    loading: false,
  }),

  actions: {
    async fetchTags() {
      try {
        const res = await axios.get<TagNode[]>('/api/v1/tags/tree')
        this.tags = res.data
      } catch (error) {
        console.error('Failed to fetch tags:', error)
        throw error
      }
    },

    async fetchFiles(tagId?: number) {
      this.loading = true
      this.selectedTagId = tagId || null

      // 查找选中的标签名称
      if (tagId) {
        this.selectedTagName = this.findTagName(this.tags, tagId) || ''
      } else {
        this.selectedTagName = ''
      }

      try {
        const res = await axios.get<{ items: FileItem[]; total: number }>('/api/v1/files', {
          params: { tag_id: tagId, recursive: true }
        })
        this.files = res.data.items
      } catch (error) {
        console.error('Failed to fetch files:', error)
        throw error
      } finally {
        this.loading = false
      }
    },

    findTagName(tags: TagNode[], id: number): string | null {
      for (const tag of tags) {
        if (tag.id === id) return tag.name
        if (tag.children.length > 0) {
          const found = this.findTagName(tag.children, id)
          if (found) return found
        }
      }
      return null
    },
  },
})
