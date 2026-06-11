# Hook Guidelines

> How composables (Vue "hooks") are used in this project.

---

## Overview

This is a Vue 3 project — "hooks" means **composables**. Currently the project has **no custom composables** and no `composables/` directory. Shared reactive logic lives in Pinia stores instead (see [state-management.md](./state-management.md)).

Do not invent composables for one-off logic; component-local consts and store actions cover current needs.

---

## Data Fetching

There is no fetch composable / query library (no TanStack Query, no useFetch). The established pattern:

1. API call functions live in `tagflow-ui/src/api/http.ts` (`tagApi.getTree()`, `fileApi.list(params)`).
2. Pinia store actions wrap them with loading/error state (`useResourceStore.fetchFiles` sets `this.loading` in a `try/finally`).
3. Views call store actions in lifecycle hooks / event handlers.

Follow this chain for new data needs instead of fetching directly in components.

---

## Naming Conventions (when a composable becomes justified)

- File: `src/composables/useXxx.ts`, named export `useXxx`.
- Must follow Vue composable rules: call only in `setup` context, return refs/computed, accept refs or plain values.
- Keep them stateless across consumers unless intentionally shared — shared state belongs in Pinia.

---

## Built-in Composition API Usage

- `defineProps` / `defineEmits` with type-only generics (see component-guidelines.md).
- Stores accessed via `useAuthStore()` / `useResourceStore()` — note `http.ts` calls `useAuthStore()` inside the request interceptor (after Pinia is installed in `main.ts`), which is the accepted pattern for store access outside components.

---

## Common Mistakes

- Don't put server state in a composable's module-level ref — use a Pinia store so devtools/SSR-safety/consistency are preserved.
- Don't call `useAuthStore()` at module top-level in non-component files; call it lazily inside functions (as `http.ts` interceptors do) so Pinia is initialized first.
