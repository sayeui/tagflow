<script setup lang="ts">
import { ChevronRight, Folder } from 'lucide-vue-next'
import type { TagNode } from '../stores/useResourceStore'

defineProps<{ node: TagNode; depth: number }>()
const emit = defineEmits<{
  select: [id: number]
}>()
</script>

<template>
  <div class="select-none">
    <div
      @click="emit('select', node.id)"
      class="flex items-center p-2 hover:bg-gray-100 cursor-pointer rounded transition-colors"
      :style="{ paddingLeft: `${depth * 12 + 8}px` }"
    >
      <ChevronRight v-if="node.children?.length" class="w-4 h-4 mr-1 text-gray-400" />
      <Folder class="w-4 h-4 mr-2 text-blue-500" />
      <span class="text-sm">{{ node.name }}</span>
    </div>

    <div v-if="node.children?.length">
      <TagItem
        v-for="child in node.children"
        :key="child.id"
        :node="child"
        :depth="depth + 1"
        @select="(id) => emit('select', id)"
      />
    </div>
  </div>
</template>
