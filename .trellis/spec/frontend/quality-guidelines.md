# Quality Guidelines

> Code quality standards for frontend development.

---

## Overview

- Quality gate: `npm run build` (= `vue-tsc && vite build`) must pass — strict type-check is the enforced lint. **No ESLint/Prettier config exists**; follow the prevailing style (2-space indent, no semicolons in TS files, single quotes).
- Automated testing: **Playwright e2e** lives in `tagflow-e2e/` (covers login / file list / search / view switch / tag tree / library scan / thumbnail lazy-load). No component unit tests yet (Vitest not introduced) — known gap, not a decision against. Run: `cd tagflow-e2e && npx playwright test`. `npm run build` type-check remains the minimum gate; manual browser verification is still a complement (see Testing Requirements).
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
- New interactive elements that e2e must drive (virtualized items in `FileGrid`/`FileList`, `Home` search box / view switcher / tag tree, etc.) carry a kebab-case `data-testid` — the only stable locator for virtual-scroll items (only visible DOM renders). Don't locate these by index, class, or text.

## Testing Requirements

Automated coverage is **Playwright e2e** in `tagflow-e2e/` — the primary safety net for user-facing flows (login, file list / search / view-switch / tag-tree, library scan, thumbnail lazy-load). Component-level unit tests are not yet set up (Vitest not introduced) — known gap, not a decision against.

### Running e2e

```bash
cd tagflow-e2e && npx playwright test
```

`playwright.config.ts` `webServer` auto-runs `cargo run` and gates on `/api/health`, so one command brings up the full stack (rust-embed serves the UI from the same backend process). First run may compile for several minutes.

### Isolation contract (do not break)

The e2e backend is fully isolated via env and must never touch the real `tagflow-core/tagflow.db` or repo-root `./cache`:

| Env key | Purpose |
|---|---|
| `TAGFLOW_DB_PATH` | temp SQLite file (auto-created via `?mode=rwc`) |
| `TAGFLOW_CACHE_DIR` | temp thumbnail cache |
| `TAGFLOW_PORT` | fixed test port (18080) |
| `TAGFLOW_ADMIN_PASSWORD` / `TAGFLOW_JWT_SECRET` | fixed test credentials |

Temp dirs are created at `playwright.config.ts` **module top level** (before `webServer.env` is built) — not in `globalSetup`, or the backend would already be up with default paths. `globalTeardown` cleans up.

### Locators

Virtual-scroll items (`RecycleScroller` in `FileGrid`) only render visible DOM — locate them by `getByTestId`, never by index/class/text. See the `data-testid` Required Pattern above.

### ffmpeg

Thumbnail assertions need `ffmpeg` on PATH. `globalSetup` probes it; if absent, thumbnail specs `test.skip` with a reason instead of failing the suite.

### Still required for every change

1. `npm run build` passes (type-check) — the minimum gate.
2. Manual verification (`npm run dev` against a running backend) remains a complement for UX/visual checks the e2e doesn't cover — golden path + error path (invalid input → Toast, 401 → login redirect).

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
- [ ] New interactive elements carry kebab-case `data-testid` for e2e (virtual-scroll items especially)?
- [ ] Tailwind-only styling; Chinese UI text?
- [ ] `npm run build` clean; no unused vars?
- [ ] Change scope limited to the requirement — no drive-by refactors?
