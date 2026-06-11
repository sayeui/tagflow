# Component Guidelines

> How components are built in this project.

---

## Overview

All components use **`<script setup lang="ts">`** (Composition API). No Options API components exist — do not introduce them. Components are presentational: data in via typed props, events out via typed emits; server interaction belongs in stores or views.

UI text shown to users is **Chinese** (e.g. `暂无文件` in `FileGrid.vue`). Icons come from `lucide-vue-next`.

---

## Component Structure

Standard file order: `<script setup lang="ts">` first, then `<template>`. No `<style>` blocks are used — styling is Tailwind utility classes in the template.

Reference example, recursive component (`tagflow-ui/src/components/TagItem.vue`):

```vue
<script setup lang="ts">
import { ChevronRight, Folder } from 'lucide-vue-next'
import type { TagNode } from '../stores/useResourceStore'

defineProps<{ node: TagNode; depth: number }>()
const emit = defineEmits<{
  select: [id: number]
}>()
</script>

<template>
  <!-- recursion by self-reference: <TagItem :node="child" :depth="depth + 1" /> -->
</template>
```

Helper functions are plain consts inside `<script setup>` (see `getFileIcon`, `formatFileSize` in `FileGrid.vue:25-45`); module-level constants in SCREAMING_CASE (`GRID_COLUMNS`, `ITEM_HEIGHT`).

---

## Props Conventions

- Type-only declarations: `defineProps<{ files: FileItem[] }>()` — never runtime object syntax.
- Typed tuple emits: `defineEmits<{ select: [id: number] }>()`.
- Shared data shapes (`TagNode`, `FileItem`) are imported from the store that owns them (`stores/useResourceStore.ts`), not redeclared.
- Recursive children re-emit upward: `@select="(id) => emit('select', id)"`.

---

## Styling Patterns

- **Tailwind CSS 3** utilities directly in templates; no scoped CSS, no CSS modules, no component library.
- Dynamic spacing that Tailwind can't express uses inline `:style` (e.g. tree indent `:style="{ paddingLeft: \`${depth * 12 + 8}px\` }"` in `TagItem.vue`).
- Conditional classes via `:class` object syntax (see icon coloring in `FileGrid.vue:76-81`).
- Common interaction states: `hover:bg-gray-100`, `transition-colors`, `cursor-pointer`, `rounded`.

---

## Performance Patterns

- Large lists must use `RecycleScroller` from `vue-virtual-scroller` (see `FileGrid.vue`): flat data is chunked into fixed-height rows (`computedRows`, 6 columns × 160px) and rendered via `v-slot="{ item }"` with `key-field="id"`.
- Images load lazily (`loading="lazy"`) with `@error`/`@load` fallback toggling to an icon (thumbnail pattern in `FileGrid.vue:66-81`).
- Route components are lazy-imported in the router.

---

## Accessibility

No formal a11y standard is enforced yet. Existing patterns to keep: `:alt` on images, `:title` on truncated filenames (`FileGrid.vue:83`). Interactive divs with `@click` are the current norm (known debt — prefer `<button>` for new work when it costs nothing).

---

## Common Mistakes

- Don't fetch data inside reusable components — fetch in views/stores, pass down as props.
- Don't redeclare `TagNode`/`FileItem` shapes locally; import from the store.
- Don't add `<style scoped>` blocks; keep styling in Tailwind classes for consistency.
- Don't render unbounded `v-for` lists for files; always go through the virtual scroller.
