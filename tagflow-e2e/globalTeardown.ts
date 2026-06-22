/**
 * globalTeardown：用例全部跑完后执行（webServer 在此之后才被停止）。
 *
 * PR1 职责：删除 playwright.config.ts 在模块顶层创建的临时目录（DB + cache）。
 * 即便删除失败也不抛错——os tmpdir 会被 OS 定期清理，残留不影响隔离正确性。
 */

import fs from 'node:fs'
import { getTmpDir } from './lib/env'

export default async function globalTeardown() {
  const tmpDir = getTmpDir()
  try {
    fs.rmSync(tmpDir, { recursive: true, force: true })
    console.log(`[globalTeardown] 已清理临时目录：${tmpDir}`)
  } catch (e) {
    console.warn(`[globalTeardown] 清理临时目录失败（忽略）：${String(e)}`)
  }
}
