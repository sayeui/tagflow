import { test, expect } from '@playwright/test'
import fs from 'node:fs'
import path from 'node:path'
import { fetchFiles, fetchLibraries, newAuthedContext } from '../lib/api'
import { FIXTURES_LIBRARY_DIR, getSeededLibraryId } from '../lib/env'

/**
 * 定时增量扫描 e2e（PR3）。
 *
 * 验证目标（对齐 prd.md「Acceptance Criteria」）：
 *   后台 scheduler 启动后立即首轮，之后按 TAGFLOW_SCAN_INTERVAL 周期扫描所有库；
 *   文件增删改在下一轮自动同步进库（无需手动 POST /scan）。
 *
 * 实现思路：
 *   playwright.config.ts 把 TAGFLOW_SCAN_INTERVAL 压到 2s（+ TAGFLOW_E2E_FAST_SCAN=1
 *   绕过后端 60s 的生产 clamp）。本 spec 往 globalSetup 已 seed 的 fixtures/library
 *   投一个新图片，**不调** POST /scan，用 expect.poll 轮询 GET /api/v1/files 直到
 *   该文件出现（证明 scheduler 自动扫入）。
 *
 * 夹具卫生（关键）：
 *   - 投入的新文件落在 fixtures/library/Photos/new_auto.jpg（Git 未跟踪，但会留在
 *     工作区）；afterEach 必须删除，保持 fixtures/library 原 6 文件原貌。
 *   - 用 copyFileSync 从 Photos/sunset.jpg 复制（保证是合法 jpg，扫描器能处理）。
 *   - 即便 beforeAll/测试中途抛错，afterEach 也会兜底删除（fs.rmSync force:true）。
 *
 * 失败容错用例（prd.md 标为「可选」）：
 *   单库扫描失败不阻塞其他库——要稳定构造一个「扫描必失败」的库（如 base_path 指向
 *   不可达路径），但 scan_library_job 内部把扫描失败记日志后继续，外部观察不到明显
 *   信号；且造一个稳定失败的库本身脆弱（权限/路径条件因 OS 而异）。故该用例不纳入
 *   自动化覆盖，已知缺口在本 spec 顶部透明记录，后续靠后端 unit test 或手测补充。
 */

/** 投入夹具的新文件绝对路径（必须落在 seeded 库的 base_path 之下，scheduler 才会扫到）。 */
const DROPPED_FILE_PATH = path.join(FIXTURES_LIBRARY_DIR, 'Photos', 'new_auto.jpg')

/** 复制源（已存在的小图，保证内容是合法 jpg）。 */
const DROPPED_FILE_SRC = path.join(FIXTURES_LIBRARY_DIR, 'Photos', 'sunset.jpg')

/** 投入文件在 GET /files 中应出现的文件名（断言用）。 */
const DROPPED_FILE_NAME = 'new_auto.jpg'

/** 轮询超时：覆盖 2s scheduler 间隔 + 扫描 + 余量。
 *  - 最坏情况：刚投入后 scheduler 刚跑完一轮，需等 ~2s 下一轮；
 *  - 加上扫描 + DB 写入 ~200ms；
 *  - 留 10s+ 余量覆盖 cargo debug 构建下偶发的调度抖动。 */
const SCAN_POLL_TIMEOUT = 15_000

test.describe('定时增量扫描（scheduler）', () => {
  test.afterEach(() => {
    // 夹具卫生：无论测试成功/失败/超时，都把投入的文件清掉，保持 fixtures/library 原貌。
    try {
      if (fs.existsSync(DROPPED_FILE_PATH)) {
        fs.rmSync(DROPPED_FILE_PATH, { force: true })
      }
    } catch (e) {
      // rmSync force:true 一般不抛；真出错也只影响夹具整洁，不影响测试结论。
      console.warn(`[scheduled-scan] afterEach 清理 ${DROPPED_FILE_PATH} 失败：${String(e)}`)
    }
  })

  test('无手动触发，scheduler 在下一轮自动扫入新文件', async () => {
    const ctx = await newAuthedContext()
    try {
      const libraryId = getSeededLibraryId()

      // 前置断言：seeded 库的 scan_interval_secs 应反映了注入的 2s（e2e 模式下）。
      // 这一步同时验证 PR2 的 DTO 字段端到端流转。
      const libsBefore = await fetchLibraries(ctx)
      const before = libsBefore.find((l) => l.id === libraryId)
      expect(before, 'seeded 库应存在').toBeDefined()
      expect(before!.scan_interval_secs, 'e2e 模式 scan_interval 应为 2').toBe(2)

      // 前置断言：投入前，DROPPED_FILE_NAME 不在文件列表里。
      const beforeFiles = await fetchFiles(ctx, { limit: 100 })
      expect(
        beforeFiles.items.find((f) => f.filename === DROPPED_FILE_NAME),
        '投入前新文件不应存在',
      ).toBeUndefined()

      // === 关键动作：往 seeded 库目录投入新文件，不调 POST /scan ===
      fs.copyFileSync(DROPPED_FILE_SRC, DROPPED_FILE_PATH)

      // === 轮询：scheduler 应在下一轮（≤2s）自动扫入 ===
      // 用 expect.poll 而非硬 sleep——文件出现即提前结束，不浪费 wall-clock。
      await expect.poll(
        async () => {
          const { items } = await fetchFiles(ctx, { limit: 100 })
          return items.find((f) => f.filename === DROPPED_FILE_NAME)?.filename ?? null
        },
        {
          timeout: SCAN_POLL_TIMEOUT,
          intervals: [500, 1_000, 2_000],
        },
      ).toBe(DROPPED_FILE_NAME)

      // === 收尾：删掉磁盘文件，等 scheduler 下一轮把它标记为 status=0（lost），
      // 从而从 GET /files（仅 status=1）消失。这一步同时验证了 scheduler 对「删除」
      // 的自动同步（prd.md AC「文件增删改在下一轮自动同步进库」），也避免遗留的
      // 失效 DB 行污染后续 spec（thumbnails.spec.ts 取 items[0] 可能正好命中这个
      // 已删文件 → 缩略图永远 404 → 用例失败）。afterEach 兜底只删磁盘文件，不替我们
      // 等 DB 同步，所以这里必须 await 把同步轮次耗完。===
      fs.rmSync(DROPPED_FILE_PATH, { force: true })
      await expect.poll(
        async () => {
          const { items } = await fetchFiles(ctx, { limit: 100 })
          return items.find((f) => f.filename === DROPPED_FILE_NAME)?.filename ?? null
        },
        {
          timeout: SCAN_POLL_TIMEOUT,
          intervals: [500, 1_000, 2_000],
        },
      ).toBeNull()
    } finally {
      await ctx.dispose()
    }
  })
})
