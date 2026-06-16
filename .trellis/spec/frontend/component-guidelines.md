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

- Large lists must use `RecycleScroller` from `vue-virtual-scroller` (see `FileGrid.vue`, `FileList.vue`): flat data is chunked into fixed-height rows (`computedRows`) and rendered via `v-slot="{ item }"` with `key-field="id"`.

### RecycleScroller 约定（vue-virtual-scroller@2.0.0-beta.x）

`RecycleScroller` 用绝对定位（transform）排布每个 item，由此有三类极易踩、且会直接导致渲染错乱或功能静默失效的约定：

**1. `item-size` 必须严格等于该 item 的真实 DOM 行高（含行间距）。**
- `RecycleScroller` 用 `item-size` 直接计算每个 item 的绝对 Y 位置；若 `item-size` 小于真实行高，相邻虚拟行会重叠/裁切（`FileGrid.vue` 曾因 168px 行高 < 卡片实际 ~176px，导致上下卡片重叠）。
- 行间距必须"贡献进" `item-size`：用固定高度容器内置顶 + 留白（如 `h-[176px] items-start`，卡片置顶、下方自然成间距），或把容器 padding 算入行高。**不要**让卡片自然高度贴近或超过 `item-size`。
- 改行布局后必须重算 `item-size`，并在真实字体下目测确认无重叠（中文字体行高/换行边界与估算可能差几 px）。

**2. 事件：无限滚动用 `@scroll-end`，不是 `@scroll`。**
- `vue-virtual-scroller@2.0.0-beta.8` 的 `RecycleScroller` **不发射 `scroll` 事件**，`emits` 仅有 `resize / visible / hidden / update / scroll-start / scroll-end`（核对 `node_modules/vue-virtual-scroller/dist/*.js` 的 `emits` 数组）。监听 `@scroll` 永远收不到事件 → "滚到底加载下一页"的无限滚动会静默失效。
- 触底加载监听 `@scroll-end`（最后一个 item 进入可视区、被回收池分配视图时触发）。注意边界：最后一页不满一行 / items 为空时不应误触发；首屏未满一屏时视需求决定是否自动续载。

**3. 分页追加数据后，行 `key` 必须全局唯一。**
- `computedRows` 的 row id 必须用**全局**值（每行起始的绝对数组索引，或行内 `file.id` 等业务主键），**不能用每页内的相对索引** —— 分页 `push` 追加后相对索引会重复，导致 `key-field` 冲突、RecycleScroller 渲染错乱。
- 单文件行（如 `FileList.vue`）可直接 `key-field="id"` 绑 `FileItem.id`（DB 主键天然全局唯一）。
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
- Don't listen for a `scroll` event on `RecycleScroller` (it doesn't emit one in beta.8) — use `@scroll-end` for infinite scroll; and don't reuse per-page relative indices as row `key` across paginated appends (use the absolute array index or `file.id`).
