/**
 * API 辅助：在 spec 内直打隔离后端，复用 globalSetup 已 seed 的 admin token。
 *
 * 所有方法返回 Playwright APIResponse（或已 .json() 解析的 DTO）；调用方按需
 * .status() / .headers() / .body() 断言。不读 / 不写 localStorage
 * （auth_token 由 stores/auth.ts 独占），测试场景不在此约束内。
 */

import { request, type APIRequestContext, type APIResponse } from '@playwright/test'
import { BASE_URL, getSeededToken } from './env'

/** FileItem 字段镜像（对齐 tagflow-core/src/models/dto.rs::FileItem，snake_case）。 */
export interface FileItemDTO {
  id: number
  filename: string
  extension: string | null
  size: number
  mtime: number
  parent_path: string
}

/** LibraryResponse 字段镜像（对齐 dto.rs::LibraryResponse）。 */
export interface LibraryDTO {
  id: number
  name: string
  protocol: string
  base_path: string
  last_scanned_at: string | null
  /** 全局扫描间隔（秒），前端据此推算「预计下次扫描」。 */
  scan_interval_secs: number
}

/**
 * TagNode 字段镜像（对齐 tagflow-core/src/models/dto.rs::TagNode）。
 * 后端 `get_tag_tree` 返回的树根数组；children 递归同构。
 */
export interface TagNodeDTO {
  id: number
  name: string
  category: string
  children: TagNodeDTO[]
}

/**
 * 建一个带 Seeded Bearer Token 的 APIRequestContext，用例结束自动 dispose。
 *
 * 用法（Playwright fixture 风格）：
 *   const ctx = await newAuthedContext()
 *   try { ... } finally { await ctx.dispose() }
 *
 * getSeededToken 在 globalSetup seed 完成后必有值；未 seed 抛错便于定位。
 */
export async function newAuthedContext(): Promise<APIRequestContext> {
  const token = getSeededToken()
  if (!token) {
    throw new Error('getSeededToken() 返回空（globalSetup 未 seed 或尚未完成）')
  }
  return request.newContext({
    baseURL: BASE_URL,
    extraHTTPHeaders: { Authorization: `Bearer ${token}` },
  })
}

/** GET /api/v1/files，返回 items + total。失败抛错（含状态码与 body 便于定位）。 */
export async function fetchFiles(
  ctx: APIRequestContext,
  params: { limit?: number; page?: number; keyword?: string } = {},
): Promise<{ items: FileItemDTO[]; total: number }> {
  const qs = new URLSearchParams()
  qs.set('limit', String(params.limit ?? 50))
  qs.set('page', String(params.page ?? 1))
  if (params.keyword) qs.set('keyword', params.keyword)
  const resp = await ctx.get(`/api/v1/files?${qs.toString()}`)
  if (!resp.ok()) {
    throw new Error(`GET /files 失败 status=${resp.status()} body=${await resp.text()}`)
  }
  return (await resp.json()) as { items: FileItemDTO[]; total: number }
}

/** GET /api/v1/files/:id/thumbnail —— 返回 Playwright APIResponse，调用方判 status。 */
export async function fetchThumbnail(
  ctx: APIRequestContext,
  fileId: number,
): Promise<APIResponse> {
  return ctx.get(`/api/v1/files/${fileId}/thumbnail`)
}

/** 列全部资源库（用于按名查 id、断言 last_scanned_at 等）。 */
export async function fetchLibraries(ctx: APIRequestContext): Promise<LibraryDTO[]> {
  const resp = await ctx.get('/api/v1/libraries')
  if (!resp.ok()) {
    throw new Error(
      `GET /libraries 失败 status=${resp.status()} body=${await resp.text()}`,
    )
  }
  return (await resp.json()) as LibraryDTO[]
}

/** POST /api/v1/libraries —— 创建本地资源库，返回原始 APIResponse（成功为 201 无 body）。 */
export async function createLibrary(
  ctx: APIRequestContext,
  payload: { name: string; protocol: string; base_path: string; config_json?: string | null },
): Promise<APIResponse> {
  return ctx.post('/api/v1/libraries', {
    data: { config_json: null, ...payload },
  })
}

/** POST /api/v1/libraries/:id/scan —— 触发扫描，返回原始 APIResponse。 */
export async function triggerScan(
  ctx: APIRequestContext,
  libraryId: number,
): Promise<APIResponse> {
  return ctx.post(`/api/v1/libraries/${libraryId}/scan`)
}

/** DELETE /api/v1/libraries/:id —— 删除资源库，返回原始 APIResponse。 */
export async function deleteLibrary(
  ctx: APIRequestContext,
  libraryId: number,
): Promise<APIResponse> {
  return ctx.delete(`/api/v1/libraries/${libraryId}`)
}

/**
 * GET /api/v1/tags/tree —— 获取标签树（已过滤在线文件关联的节点）。
 *
 * 后端契约（tagflow-core/src/api/tag.rs::get_tag_tree）：
 *   - 只显示有 status=1 文件关联（含子树）的标签
 *   - 删库后的孤儿标签、仅关联离线文件的标签不返回
 *   - 跨库共享标签：只要还有任一库的在线文件关联就保留
 *
 * 失败抛错（含状态码与 body 便于定位）。
 */
export async function fetchTagTree(ctx: APIRequestContext): Promise<TagNodeDTO[]> {
  const resp = await ctx.get('/api/v1/tags/tree')
  if (!resp.ok()) {
    throw new Error(`GET /tags/tree 失败 status=${resp.status()} body=${await resp.text()}`)
  }
  return (await resp.json()) as TagNodeDTO[]
}

/**
 * 在 TagNode 树中查找指定 category 下的节点（递归扁平化，仅按 name 匹配）。
 *
 * 用于断言某标签是否仍显示。返回 undefined 表示未找到（删库后应如此）。
 */
export function findTagNodeByName(
  nodes: TagNodeDTO[],
  name: string,
): TagNodeDTO | undefined {
  for (const n of nodes) {
    if (n.name === name) return n
    const hit = findTagNodeByName(n.children, name)
    if (hit) return hit
  }
  return undefined
}
