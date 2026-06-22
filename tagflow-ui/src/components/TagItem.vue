<script setup lang="ts">
import { computed, ref } from 'vue'
import { ChevronRight, ChevronDown, Folder, Hash } from 'lucide-vue-next'
import type { TagNode } from '../stores/useResourceStore'

const props = defineProps<{ node: TagNode; depth: number; selectedIds: number[] }>()
const emit = defineEmits<{
  toggle: [id: number]
}>()

const hasChildren = computed(() => (props.node.children?.length ?? 0) > 0)
const checked = computed(() => props.selectedIds.includes(props.node.id))

// 折叠态：仅本组件内部，无需上抛
const collapsed = ref(false)
</script>

<template>
  <div class="select-none">
    <div
      :data-testid="`tag-node`"
      :data-tag-id="node.id"
      :data-tag-name="node.name"
      :data-tag-category="node.category"
      class="flex items-center p-2 hover:bg-gray-100 rounded transition-colors"
      :style="{ paddingLeft: `${depth * 12 + 8}px` }"
    >
      <!-- 折叠箭头（仅有子节点时显示） -->
      <component
        :is="collapsed ? ChevronRight : ChevronDown"
        v-if="hasChildren"
        class="w-4 h-4 mr-1 text-gray-400 cursor-pointer shrink-0"
        @click.stop="collapsed = !collapsed"
      />
      <div v-else class="w-4 mr-1 shrink-0" />

      <!-- 多选 checkbox -->
      <input
        type="checkbox"
        :checked="checked"
        @change="emit('toggle', node.id)"
        class="w-3.5 h-3.5 mr-2 shrink-0 cursor-pointer accent-blue-600"
      />

      <component
        :is="hasChildren ? Folder : Hash"
        class="w-4 h-4 mr-1.5 shrink-0"
        :class="hasChildren ? 'text-blue-500' : 'text-gray-400'"
      />
      <span class="text-sm truncate" :title="node.name">{{ node.name }}</span>
    </div>

    <div v-if="hasChildren && !collapsed">
      <TagItem
        v-for="child in node.children"
        :key="child.id"
        :node="child"
        :depth="depth + 1"
        :selected-ids="selectedIds"
        @toggle="(id) => emit('toggle', id)"
      />
    </div>
  </div>
</template>
