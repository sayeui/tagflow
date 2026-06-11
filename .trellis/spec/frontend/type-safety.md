# Type Safety

> Type safety patterns in this project.

---

## Overview

- TypeScript ~5.6 in **strict mode** (`tsconfig.json`: `"strict": true`, `noUnusedLocals`, `noUnusedParameters`, `noFallthroughCasesInSwitch`).
- Type-checking is part of the production build: `npm run build` runs `vue-tsc && vite build` — a type error fails the build. There is no separate runtime validation library (no zod); the backend is trusted as the schema source.

---

## Type Organization

No central `types/` directory. Types live next to their owner:

- **Server data shapes** are `export interface` in the store that owns them — `TagNode` and `FileItem` in `stores/useResourceStore.ts:4-18`. Components import them: `import type { TagNode } from '../stores/useResourceStore'`.
- **API request payloads** are inline object types on the API functions (`libraryApi.create(data: { name: string; protocol: string; base_path: string; config_json?: string })` in `api/http.ts:76-81`).
- **Untyped dependencies** get a local `.d.ts` shim: `src/vue-virtual-scroller.d.ts`.

Use `import type { ... }` for type-only imports (required by `isolatedModules`).

---

## DTO Conventions

Frontend interfaces mirror the backend DTOs in `tagflow-core/src/models/dto.rs` **field-for-field, in snake_case** — no camelCase renaming layer:

```ts
export interface FileItem {
  id: number
  filename: string
  extension: string | null   // backend Option<String> → `T | null`, not `T?`
  size: number
  mtime: number
  parent_path: string        // snake_case preserved
}
```

When the backend DTO changes, update the matching interface in the same change (cross-layer consistency).

---

## Patterns in Use

- Nullable state: explicit union types via cast in store state (`null as number | null`).
- DOM narrowing with `as` only where the element type is certain: `(e.target as HTMLInputElement)`.
- Router types from the library: `const routes: RouteRecordRaw[]` (`router/index.ts:10`).
- Typed props/emits generics in components (see component-guidelines.md).

---

## Common Mistakes

- Don't rename backend snake_case fields to camelCase — interceptors/DTOs do no transformation.
- Don't use `any`; if a third-party module is untyped, add a `.d.ts` shim like `vue-virtual-scroller.d.ts`.
- Don't model backend `Option<T>` as optional `field?:` — use `field: T | null` to match serde output.
- Axios responses are currently untyped (`res.data`); when adding generics prefer `instance.get<FileResponse>(...)` but don't retrofit the whole file in unrelated changes.
