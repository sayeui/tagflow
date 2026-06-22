/**
 * globalSetup：在 webServer 拉起隔离后端之后、用例之前执行。
 *
 * 职责：
 *   1. 探测 ffmpeg 是否可用（后续 PR 的缩略图用例据此 skip）。
 *   2. 等待 /api/health 可达（双保险；webServer.url 已做一次，但 globalSetup
 *      里再 ping 一次便于日志观测与失败定位）。
 *   3. seed 隔离后端：登录拿 token → 创建指向 fixtures/library 的本地资源库
 *      → 触发扫描 → 轮询 GET /api/v1/files 直到 6 个文件入库（5 张图片 + 1 个
 *      非媒体 notes.txt）。token 与 library id 写入 process.env，供用例复用。
 *
 * 只等"文件入库"，不等缩略图（缩略图是 worker 5s 轮询异步生成，PR3 才关心）。
 */

import { request } from '@playwright/test'
import { spawnSync } from 'node:child_process'
import {
  ADMIN_PASSWORD,
  ADMIN_USERNAME,
  BASE_URL,
  EXPECTED_FILE_COUNT,
  FIXTURES_LIBRARY_DIR,
  setFfmpegAvailable,
  setSeededLibraryId,
  setSeededToken,
} from './lib/env'

async function waitForHealth() {
  const ctx = await request.newContext({ baseURL: BASE_URL })
  try {
    // 最多等 ~30s；后端通常 1s 内就绪（webServer 已 gate 过）。
    const maxAttempts = 30
    let lastErr: unknown = null
    for (let i = 0; i < maxAttempts; i++) {
      try {
        const resp = await ctx.get('/api/health', { timeout: 2_000 })
        if (resp.ok()) {
          console.log(`[globalSetup] /api/health ok (attempt ${i + 1})`)
          return
        }
        lastErr = new Error(`health status ${resp.status()}`)
      } catch (e) {
        lastErr = e
      }
      await new Promise((r) => setTimeout(r, 1_000))
    }
    throw new Error(`后端 /api/health 在 ${maxAttempts}s 内未就绪：${String(lastErr)}`)
  } finally {
    await ctx.dispose()
  }
}

function probeFfmpeg() {
  try {
    const r = spawnSync('ffmpeg', ['-version'], { stdio: 'pipe' })
    const ok = r.status === 0
    setFfmpegAvailable(ok)
    if (ok) {
      const firstLine = r.stdout?.toString().split('\n')[0] ?? 'ffmpeg'
      console.log(`[globalSetup] ffmpeg 可用：${firstLine}`)
    } else {
      console.warn('[globalSetup] ffmpeg 不可用（exit 非 0），缩略图相关用例将被 skip')
    }
  } catch (e) {
    setFfmpegAvailable(false)
    console.warn(`[globalSetup] ffmpeg 未找到（${String(e)}），缩略图相关用例将被 skip`)
  }
}

/** APIRequestContext 的实例类型（request.newContext() 解析后的值）。 */
type ApiCtx = Awaited<ReturnType<typeof request.newContext>>

/** 登录并返回 JWT token（payload 字段对齐 tagflow-core/src/api/auth.rs 的 LoginRequest）。 */
async function login(ctx: ApiCtx): Promise<string> {
  const resp = await ctx.post('/api/auth/login', {
    data: { username: ADMIN_USERNAME, password: ADMIN_PASSWORD },
  })
  if (!resp.ok()) {
    throw new Error(`seed 登录失败 status=${resp.status()} body=${await resp.text()}`)
  }
  // 后端 LoginResponse { token, expires_at }
  const body = (await resp.json()) as { token?: string }
  const token = body.token
  if (!token) throw new Error(`seed 登录响应缺 token：${JSON.stringify(body)}`)
  return token
}

/** 创建指向 fixtures/library 的本地资源库，返回新建 library id。
 *  payload 字段对齐 tagflow-core/src/models/dto.rs 的 CreateLibraryRequest。 */
async function createFixturesLibrary(ctx: ApiCtx, token: string): Promise<number> {
  const resp = await ctx.post('/api/v1/libraries', {
    headers: { Authorization: `Bearer ${token}` },
    data: {
      name: 'e2e-fixtures',
      protocol: 'local',
      base_path: FIXTURES_LIBRARY_DIR,
      config_json: null,
    },
  })
  if (resp.status() !== 201) {
    // reuseExistingServer 复用后端时可能残留同名库（DB 不复用一般不会，但兜底）。
    // 400/500 不带具体语义，统一按名兜底查一次，命中则复用。
    if (resp.status() === 400 || resp.status() === 409 || resp.status() === 500) {
      console.warn(
        `[globalSetup] create_library 返回 ${resp.status()}（可能已存在），尝试按名复用：${await resp.text()}`,
      )
      const existing = await findFixturesLibraryId(ctx, token)
      if (existing !== null) return existing
    }
    throw new Error(
      `seed 创建资源库失败 status=${resp.status()} body=${await resp.text()}`,
    )
  }
  // create_library 返回 201 无 body，按 name 再查一次拿 id。
  const id = await findFixturesLibraryId(ctx, token)
  if (id === null) {
    throw new Error('seed 创建资源库后按名查询未找到，DB 状态异常')
  }
  return id
}

/** 按 name=e2e-fixtures 查 library id，未找到返回 null。 */
async function findFixturesLibraryId(ctx: ApiCtx, token: string): Promise<number | null> {
  const resp = await ctx.get('/api/v1/libraries', {
    headers: { Authorization: `Bearer ${token}` },
  })
  if (!resp.ok()) {
    throw new Error(`seed 列资源库失败 status=${resp.status()} body=${await resp.text()}`)
  }
  const list = (await resp.json()) as Array<{ id: number; name: string }>
  const hit = list.find((l) => l.name === 'e2e-fixtures')
  return hit ? hit.id : null
}

/** POST /api/v1/libraries/:id/scan 触发扫描（立即返回 202）。 */
async function triggerScan(ctx: ApiCtx, token: string, libraryId: number): Promise<void> {
  const resp = await ctx.post(`/api/v1/libraries/${libraryId}/scan`, {
    headers: { Authorization: `Bearer ${token}` },
  })
  if (resp.status() !== 202) {
    // 409 = 上次扫描进行中（reuseExistingServer 复用后端时可能），忽略即可。
    if (resp.status() === 409) {
      console.warn('[globalSetup] scan 返回 409（扫描进行中），忽略')
      return
    }
    throw new Error(`seed 触发扫描失败 status=${resp.status()} body=${await resp.text()}`)
  }
}

/** 轮询 GET /api/v1/files 直到文件数 >= EXPECTED_FILE_COUNT 或超时。
 *  注意：只看文件入库（status=1 的行），不看缩略图。 */
async function waitForFiles(ctx: ApiCtx, token: string): Promise<void> {
  const maxAttempts = 30 // 30 × 1s = 30s（扫描本地 6 个小文件通常 < 1s）
  let lastTotal = -1
  for (let i = 0; i < maxAttempts; i++) {
    const resp = await ctx.get('/api/v1/files?limit=1', {
      headers: { Authorization: `Bearer ${token}` },
    })
    if (resp.ok()) {
      const body = (await resp.json()) as { total?: number }
      const total = body.total ?? 0
      if (total !== lastTotal) {
        console.log(`[globalSetup] 文件入库进度：${total}/${EXPECTED_FILE_COUNT}`)
        lastTotal = total
      }
      if (total >= EXPECTED_FILE_COUNT) return
    }
    await new Promise((r) => setTimeout(r, 1_000))
  }
  throw new Error(
    `seed 等待文件入库超时（30s）：仅见到 ${lastTotal}/${EXPECTED_FILE_COUNT}`,
  )
}

async function seedFixtures() {
  const ctx = await request.newContext({ baseURL: BASE_URL })
  try {
    const token = await login(ctx)
    setSeededToken(token)
    console.log('[globalSetup] seed：已登录并保存 token')

    const libraryId = await createFixturesLibrary(ctx, token)
    setSeededLibraryId(libraryId)
    console.log(`[globalSetup] seed：资源库 id=${libraryId}（path=${FIXTURES_LIBRARY_DIR}）`)

    await triggerScan(ctx, token, libraryId)
    console.log('[globalSetup] seed：扫描已触发')

    await waitForFiles(ctx, token)
    console.log('[globalSetup] seed：文件已入库')
  } finally {
    await ctx.dispose()
  }
}

export default async function globalSetup() {
  console.log('[globalSetup] 开始')
  await waitForHealth()
  probeFfmpeg()
  await seedFixtures()
  console.log('[globalSetup] 完成')
}
