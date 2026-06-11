# Directory Structure

> How frontend code is organized in this project.

---

## Overview

The frontend is `tagflow-ui`, a Vue 3 + TypeScript + Vite SPA (no SSR). It is intentionally small and flat — do not introduce extra layers (no `services/`, `utils/`, `composables/` directories exist yet; create them only when there is real shared logic).

---

## Directory Layout

```
tagflow-ui/src/
├── main.ts                    # createApp + pinia + router
├── App.vue                    # Root shell
├── style.css                  # Tailwind entry (@tailwind directives)
├── api/
│   └── http.ts                # Axios instance + interceptors + ALL API modules (authApi, tagApi, fileApi, libraryApi)
├── components/                # Reusable presentational components
│   ├── TagItem.vue            # Recursive tag-tree node
│   ├── FileGrid.vue           # Virtual-scrolled file grid (vue-virtual-scroller)
│   └── Toast.vue              # Notification component
├── stores/                    # Pinia stores
│   ├── auth.ts                # JWT/localStorage auth state
│   └── useResourceStore.ts    # Tags + files + selection state
├── router/
│   └── index.ts               # Routes + global auth guard
├── views/                     # Route-level pages
│   ├── Login.vue
│   ├── Home.vue
│   └── settings/              # Settings pages grouped in a subfolder
│       ├── Security.vue
│       └── Libraries.vue
├── vite-env.d.ts
└── vue-virtual-scroller.d.ts  # Hand-written shim for untyped dependency
```

---

## Module Organization

- **Route page** → `views/` (group related pages in a subfolder like `views/settings/`). Register in `router/index.ts` with a lazy import: `const Login = () => import('@/views/Login.vue')`.
- **Reusable UI** → `components/`, kept presentational: receive data via props, signal upward via emits (see `FileGrid.vue` taking `files: FileItem[]`).
- **Server state & cross-page state** → a Pinia store in `stores/`.
- **HTTP calls** → only through the shared axios instance in `api/http.ts`; new endpoints are added as a new `xxxApi` object in that same file (no per-resource files yet).
- Path alias `@/` → `src/` (configured in both `vite.config.ts` and `tsconfig.json`).
- Dev-time backend access via Vite proxy: `/api` → `http://localhost:8080` (`vite.config.ts`).

---

## Naming Conventions

- Components/views: `PascalCase.vue` (`TagItem.vue`, `FileGrid.vue`, `Libraries.vue`).
- TS modules: `camelCase.ts` (`http.ts`, `auth.ts`, `useResourceStore.ts`).
- Stores: `useXxxStore` export name; Pinia store id is the short noun (`'auth'`, `'resource'`).
- API objects: `<resource>Api` (`libraryApi`, `tagApi`).
- Route names: PascalCase (`'Home'`, `'SecuritySettings'`); paths kebab/lowercase (`/settings/security`).

---

## Examples

- Page wiring + lazy routes + guard: `tagflow-ui/src/router/index.ts`
- Component organization reference: `tagflow-ui/src/components/FileGrid.vue`
- API module pattern: `tagflow-ui/src/api/http.ts`
