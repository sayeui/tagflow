import { defineConfig, devices } from '@playwright/test'
import fs from 'node:fs'
import os from 'node:os'
import path from 'node:path'
import {
  ADMIN_PASSWORD,
  ADMIN_USERNAME,
  BASE_URL,
  CACHE_DIR_ENV,
  DB_PATH_ENV,
  JWT_SECRET,
  TMP_DIR_ENV,
  TEST_PORT,
} from './lib/env'

/**
 * 时序要点（务必遵守，否则隔离会破）：
 *
 *   Playwright 执行顺序：webServer 启动 → globalSetup → tests → globalTeardown → webServer 停止
 *
 * 因此临时 DB / cache 目录**必须在模块加载顶层**就创建好并把绝对路径塞进 env 对象，
 * 让 webServer.env 注入到后端进程；后端一启动就读到正确的 TAGFLOW_DB_PATH /
 * TAGFLOW_CACHE_DIR。如果在 globalSetup 里创建目录就太晚了——那时后端已带着
 * （缺省）路径起来，会污染仓库内真实的 tagflow.db / ./cache。
 *
 * reuseExistingServer: !CI 让本地复用提速；复用时下方 env 仍会被 Playwright
 * 透传，但由于后端已用旧 env 起来，新 env 不生效——所以复用模式要求用户自行确保
 * 已起的后端就是用同一组测试 env 启动的（典型做法：用本仓库提供的脚本起后端）。
 * CI 下强制新起，行为可预测。
 */

// ---- 模块顶层：创建本运行的临时目录 ----
const TMP_DIR = fs.mkdtempSync(path.join(os.tmpdir(), 'tagflow-e2e-'))
const DB_PATH = path.join(TMP_DIR, 'tagflow-e2e.db')
const CACHE_DIR = path.join(TMP_DIR, 'cache')
fs.mkdirSync(CACHE_DIR, { recursive: true })

// 把路径挂到 process.env，让 globalTeardown 与 spec 能读取。
process.env[TMP_DIR_ENV] = TMP_DIR
process.env[DB_PATH_ENV] = DB_PATH
process.env[CACHE_DIR_ENV] = CACHE_DIR

// 注入到后端进程的 env 子集（与 infra/config.rs、main.rs 约定一致）。
const backendEnv: Record<string, string> = {
  TAGFLOW_DB_PATH: DB_PATH,
  TAGFLOW_CACHE_DIR: CACHE_DIR,
  TAGFLOW_PORT: String(TEST_PORT),
  TAGFLOW_ADMIN_USERNAME: ADMIN_USERNAME,
  TAGFLOW_ADMIN_PASSWORD: ADMIN_PASSWORD,
  TAGFLOW_JWT_SECRET: JWT_SECRET,
  // 降低日志噪音（保留 info 及以上）
  RUST_LOG: 'tagflow_core=info,axum=warn',
}

// 测试未跑完就被 Ctrl-C 时也尽量清理临时目录。
function cleanupTmpDir() {
  try {
    fs.rmSync(TMP_DIR, { recursive: true, force: true })
  } catch {
    // 忽略：os tmpdir 由 OS 定期清理，残留不影响隔离正确性
  }
}
process.once('exit', cleanupTmpDir)
process.once('SIGINT', () => {
  cleanupTmpDir()
  process.exit(130)
})
process.once('SIGTERM', () => {
  cleanupTmpDir()
  process.exit(143)
})

export default defineConfig({
  testDir: './tests',
  fullyParallel: false, // 单后端 + 共享隔离 DB，串行更可预测
  forbidOnly: !!process.env.CI,
  retries: process.env.CI ? 1 : 0,
  workers: 1, // 单后端，避免并发请求互相干扰（虚拟滚动/分页状态）
  reporter: process.env.CI ? [['github'], ['list']] : 'list',
  use: {
    baseURL: BASE_URL,
    trace: 'on-first-retry',
    actionTimeout: 10_000,
    navigationTimeout: 15_000,
  },
  projects: [
    {
      name: 'chromium',
      use: { ...devices['Desktop Chrome'] },
    },
  ],
  // 隔离后端：cwd 指向 tagflow-core，保证 cargo run 编译/运行的是后端。
  // debug 构建在首次编译后增量很快；timeout 留足首次冷启动编译时间。
  webServer: {
    command: 'cargo run',
    cwd: path.resolve(__dirname, '..', 'tagflow-core'),
    env: backendEnv,
    url: `http://127.0.0.1:${TEST_PORT}/api/health`,
    reuseExistingServer: !process.env.CI,
    timeout: 600_000, // 10 分钟，覆盖首次 cargo 全量编译
    stdout: 'pipe', // 不污染测试输出；失败时 Playwright 仍会打印日志
    stderr: 'pipe',
  },
  globalSetup: require.resolve('./globalSetup.ts'),
  globalTeardown: require.resolve('./globalTeardown.ts'),
})
