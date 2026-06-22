/**
 * 共享测试常量与路径解析助手。
 *
 * 所有测试代码（globalSetup / globalTeardown / spec / config）必须从此模块读取
 * 后端端口、管理员凭据、JWT 密钥，避免散落的字面量导致不一致。
 *
 * 注意：临时 DB / cache 目录路径**不**在此处固定，而在 playwright.config.ts 模块
 * 顶层按运行 ID 动态生成后挂到 process.env，本模块仅提供读取与解析帮助函数。
 */

import path from 'node:path'

/** 固定测试端口（CI 与本地一致）。后端 TAGFLOW_PORT 与前端 baseURL 均引用此值。 */
export const TEST_PORT = 18080

/** 测试后端 baseURL（rust-embed 嵌入前端，单进程）。 */
export const BASE_URL = `http://127.0.0.1:${TEST_PORT}`

/** 测试管理员用户名（对应后端缺省 `admin`）。 */
export const ADMIN_USERNAME = 'admin'

/**
 * 测试管理员密码（≥ 12 字节，满足后端 validate_admin_password_len）。
 * 仅测试用、明文写在版本库中可接受。
 */
export const ADMIN_PASSWORD = 'tagflow-e2e-admin-pw'

/**
 * 测试 JWT 密钥（≥ 32 字节，满足后端 init_jwt_secret 校验）。
 * 固定值使 token 在本地复用后端场景下可解析。
 */
export const JWT_SECRET = 'tagflow-e2e-jwt-secret-fixed-32b+'

// ---- process.env 桥接的临时目录路径 ----
// playwright.config.ts 在模块顶层创建临时目录后写入 process.env；其余代码读取这里。

const TMP_DIR_ENV = 'TAGFLOW_E2E_TMP_DIR'
const DB_PATH_ENV = 'TAGFLOW_E2E_DB_PATH'
const CACHE_DIR_ENV = 'TAGFLOW_E2E_CACHE_DIR'
const FFMPEG_FLAG_ENV = 'TAGFLOW_E2E_FFMPEG_AVAILABLE'

// globalSetup seed 完成后写入；用例（与 globalTeardown/debug）可读取。
// token 用于 APIRequestContext 直接打后端；libraryId 用于扫描/清理等场景。
const SEEDED_TOKEN_ENV = 'TAGFLOW_E2E_SEEDED_TOKEN'
const SEEDED_LIBRARY_ID_ENV = 'TAGFLOW_E2E_SEEDED_LIBRARY_ID'

/** 读取本运行创建的临时根目录（DB 与 cache 均在其下）。 */
export function getTmpDir(): string {
  const v = process.env[TMP_DIR_ENV]
  if (!v) throw new Error(`${TMP_DIR_ENV} 未设置（playwright.config.ts 应在模块顶层写入）`)
  return v
}

/** 读取隔离 DB 文件路径（已注入后端 TAGFLOW_DB_PATH）。 */
export function getDbPath(): string {
  const v = process.env[DB_PATH_ENV]
  if (!v) throw new Error(`${DB_PATH_ENV} 未设置`)
  return v
}

/** 读取隔离 cache 目录路径（已注入后端 TAGFLOW_CACHE_DIR）。 */
export function getCacheDir(): string {
  const v = process.env[CACHE_DIR_ENV]
  if (!v) throw new Error(`${CACHE_DIR_ENV} 未设置`)
  return v
}

/** ffmpeg 探测结果（globalSetup 写入），供缩略图用例决定 skip。 */
export function isFfmpegAvailable(): boolean {
  return process.env[FFMPEG_FLAG_ENV] === '1'
}

/** 写入 ffmpeg 探测结果（仅 globalSetup 调用）。 */
export function setFfmpegAvailable(available: boolean): void {
  process.env[FFMPEG_FLAG_ENV] = available ? '1' : '0'
}

export { TMP_DIR_ENV, DB_PATH_ENV, CACHE_DIR_ENV, FFMPEG_FLAG_ENV }

// ---- globalSetup seed 产物（token + library id）----

/** 写入 seed 完成后的 admin token（仅 globalSetup 调用）。 */
export function setSeededToken(token: string): void {
  process.env[SEEDED_TOKEN_ENV] = token
}

/** 读取 seed 时签发的 admin token；未 seed 时返回 undefined。 */
export function getSeededToken(): string | undefined {
  return process.env[SEEDED_TOKEN_ENV]
}

/** 写入 seed 创建的资源库 id（仅 globalSetup 调用）。 */
export function setSeededLibraryId(id: number): void {
  process.env[SEEDED_LIBRARY_ID_ENV] = String(id)
}

/** 读取 seed 创建的资源库 id；未 seed 时抛错（用例必须先 seed）。 */
export function getSeededLibraryId(): number {
  const v = process.env[SEEDED_LIBRARY_ID_ENV]
  if (v === undefined) {
    throw new Error(`${SEEDED_LIBRARY_ID_ENV} 未设置（globalSetup 应已 seed 资源库）`)
  }
  const n = Number.parseInt(v, 10)
  if (!Number.isFinite(n)) {
    throw new Error(`${SEEDED_LIBRARY_ID_ENV} 非数字：${v}`)
  }
  return n
}

/** fixtures 下应有 6 个文件（5 个图片 + 1 个非媒体文本 notes.txt，见 prd.md 夹具清单）。 */
export const EXPECTED_FILE_COUNT = 6

/** fixtures/library 的绝对路径（后续 PR 用于 seed 资源库）。 */
export const FIXTURES_LIBRARY_DIR = path.resolve(__dirname, '..', 'fixtures', 'library')
