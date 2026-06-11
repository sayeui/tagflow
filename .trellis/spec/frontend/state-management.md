# State Management

> How state is managed in this project.

---

## Overview

- **Pinia 2** with **Options-style stores** (`defineStore('id', { state, getters, actions })`). Both existing stores use this style — do NOT write setup-style stores; stay consistent.
- Two stores: `stores/auth.ts` (id `'auth'`) and `stores/useResourceStore.ts` (id `'resource'`).
- Local component state (form inputs, dialog visibility) stays in component refs; only cross-page or server-derived state goes into a store.

---

## Store Structure

Reference (`tagflow-ui/src/stores/useResourceStore.ts`):

```ts
export const useResourceStore = defineStore('resource', {
  state: () => ({
    tags: [] as TagNode[],
    files: [] as FileItem[],
    selectedTagId: null as number | null,
    loading: false,
  }),
  actions: {
    async fetchFiles(tagId?: number) {
      this.loading = true
      try {
        const res = await fileApi.list({ tag_id: tagId, recursive: true })
        this.files = res.data.items
      } catch (error) {
        console.error('Failed to fetch files:', error)
        throw error          // caller (view) decides how to surface it
      } finally {
        this.loading = false
      }
    },
  },
})
```

Conventions:
- State typed via `as` casts in the `state()` initializer (`[] as TagNode[]`, `null as number | null`).
- Async actions: set loading flag, `try/catch/finally`, `console.error` + re-`throw` so views can show a Toast.
- Pure helpers may live as actions too (`findTagName` recursion).
- Getters for derived booleans: `isLoggedIn: (state) => !!state.token` (`stores/auth.ts`).

---

## Server State

No query-cache library. Stores hold the latest server snapshot (`tags`, `files`) and re-fetch explicitly via actions. All HTTP goes through `api/http.ts` API objects — stores never call axios directly.

---

## Auth State & Persistence

`stores/auth.ts` is the single owner of the JWT:

- Hydrates from `localStorage` in `state()`: `token: localStorage.getItem('auth_token') || null`.
- `setToken(token, username)` writes both state and localStorage; `logout()` clears both.
- Consumers: axios request interceptor injects `Authorization: Bearer ${authStore.token}`; response interceptor calls `authStore.logout()` + redirects on 401; router guard checks `authStore.isLoggedIn` (`router/index.ts:39-51`).

Keys: `auth_token`, `username`. Don't read/write these localStorage keys anywhere else.

---

## Common Mistakes

- Don't duplicate server data into component refs when a store already holds it — bind to the store via `storeToRefs` or direct property access.
- Don't swallow errors in actions; log + rethrow so the view layer can show feedback.
- Don't introduce setup-style stores or a second persistence mechanism for auth.
