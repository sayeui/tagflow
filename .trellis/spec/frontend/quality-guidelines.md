# Quality Guidelines

> Code quality standards for frontend development.

---

## Overview

- Quality gate: `npm run build` (= `vue-tsc && vite build`) must pass — strict type-check is the enforced lint. **No ESLint/Prettier config exists**; follow the prevailing style (2-space indent, no semicolons in TS files, single quotes).
- No frontend test framework is set up (no vitest/jest). Correctness is verified by type-check + manual testing against the dev server (`npm run dev`, Vite proxies `/api` to `localhost:8080`).
- User-facing strings are Chinese; code identifiers and comments follow existing file style (comments mostly Chinese).

---

## Forbidden Patterns

- Direct `axios` imports in components/stores — all HTTP must go through `api/http.ts` instance (interceptors handle auth + 401).
- Reading/writing `auth_token`/`username` localStorage keys outside `stores/auth.ts`.
- Options API components or setup-style Pinia stores — stay with `<script setup>` + options stores.
- Unvirtualized rendering of file lists — use `RecycleScroller` (`FileGrid.vue`).
- `any` types; hardcoded backend URLs (use the `/api` proxy path).
- Adding UI frameworks/component libraries — styling is Tailwind utilities only; icons only from `lucide-vue-next`.

---

## Required Patterns

- New routes: lazy import + register in `router/index.ts`; the global `beforeEach` guard automatically protects them (everything except `Login` requires auth).
- New endpoints: add to the matching `xxxApi` object in `api/http.ts` with typed params.
- Server state changes flow: API object → store action (loading + try/catch/finally + rethrow) → view shows Toast on failure.
- Backend DTO mirrors in snake_case (see type-safety.md).
- Build must stay clean under `noUnusedLocals`/`noUnusedParameters` — delete dead code instead of suffixing `_`.

---

## Testing Requirements

Current reality: no automated frontend tests. Minimum bar for changes:

1. `npm run build` passes (type-check).
2. Manual verification in the browser via `npm run dev` against a running backend — golden path plus error path (e.g. invalid input shows Toast, 401 redirects to login).

If introducing a test framework, that's a team decision — don't bootstrap one inside an unrelated task.

---

## Common Mistakes

- **`<img style="display:none">` + `loading="lazy"` 不会加载**：浏览器对 `display:none` 的 lazy img 不发请求，`@load` 永不触发，图片永远不显示（`FileGrid` 缩略图历史坑）。改用 `opacity` 控制：`@load` 设 `opacity:1`、`@error` 设 `opacity:0`；`opacity:0` 的 img 仍会加载。
- **受保护路由的媒体 `src` 会 401**：`<img>`/`<video>`/`<iframe src>` 不带 `Authorization` 头，受 `auth_middleware` 保护的资源（缩略图/文件内容）须用 `mediaUrl()`（`api/http.ts`）拼 `?token=<jwt>`，后端兜底逻辑见 backend `error-handling.md`。

---

## Code Review Checklist

- [ ] HTTP via `api/http.ts` only; no token handling outside the auth store?
- [ ] Types mirror backend DTOs exactly (snake_case, `| null`)?
- [ ] Loading/error states handled in store actions; user feedback via Toast?
- [ ] Large lists virtualized; images lazy with icon fallback?
- [ ] Tailwind-only styling; Chinese UI text?
- [ ] `npm run build` clean; no unused vars?
- [ ] Change scope limited to the requirement — no drive-by refactors?
