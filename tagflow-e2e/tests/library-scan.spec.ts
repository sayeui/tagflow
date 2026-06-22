import { test, expect } from '@playwright/test'
import fs from 'node:fs'
import os from 'node:os'
import path from 'node:path'
import {
  createLibrary,
  deleteLibrary,
  fetchLibraries,
  triggerScan,
  newAuthedContext,
} from '../lib/api'
import { getSeededLibraryId } from '../lib/env'

/**
 * 资源库扫描触发 e2e（PR3）。
 *
 * 后端契约（tagflow-core/src/api/library.rs::trigger_scan）：
 *   - 200 (202 ACCEPTED)：扫描任务已接受，后台异步执行
 *   - 404：library id 不存在
 *   - 409：同库扫描进行中（进程内 HashSet 并发防护）
 *
 * 稳定性取舍（详见 prd.md 与本任务 PR3 说明）：
 *   - 正常扫描触发（202）：用 seeded 库直接 POST，断言 202 + last_scanned_at 已更新。
 *   - 不存在的 library id（404）：用大随机 id 触发，必然 404，无时序依赖，稳定。
 *   - 409 并发：seeded 库的扫描在 globalSetup 时已完成（SCANNING 锁已释放）；
 *     再触发会立即拿到 202 而非 409。要稳定触发 409 需精确卡在 scan_library 执行中
 *     的窗口——对小夹具（5 张图 < 1s 扫完）几乎不可能，且重试会危及隔离。
 *     故 409 不在自动化覆盖范围，已知缺口留给后续手测/集成测试。
 *
 * scheduled-scan PR3 引入的时序变化：
 *   playwright.config.ts 现在注入 TAGFLOW_SCAN_INTERVAL=2，scheduler 每 2s 扫一轮
 *   所有库（与手动 trigger_scan 共享同一把 409 锁）。这意味着手动 trigger_scan 有
 *   小概率撞上 scheduler 正在扫描 → 拿到 409。这是合法的瞬时态（后端契约未变），
 *   不是回归。下方 202 断言用 triggerScanAcceptingSchedulerConflict 包一层短重试
 *   （收到 409 等几百毫秒再试，最多 3 次），把"必为 202"的语义从"第一次就 202"
 *   放宽为"短时间内一定能拿到 202"。既不削弱断言意图，又对 scheduler 抖动稳健。
 *
 * 隔离：正常扫描用 seeded 库（不破坏其状态）；额外创建的临时库用后即删。
 */

/**
 * 触发扫描并容忍与 scheduler 的瞬时 409 冲突。
 *
 * 收到 409 时短退避后重试，最多 3 次；最终返回最后一次响应（调用方按需断言 status）。
 * 收到非 409 立即返回。仅在 e2e（scheduler 2s 频扫）下需要，production 不受影响。
 */
async function triggerScanAcceptingSchedulerConflict(
  ctx: Parameters<typeof triggerScan>[0],
  libraryId: number,
): ReturnType<typeof triggerScan> {
  const maxAttempts = 3
  let resp: Awaited<ReturnType<typeof triggerScan>> | null = null
  for (let i = 0; i < maxAttempts; i++) {
    resp = await triggerScan(ctx, libraryId)
    if (resp.status() !== 409) return resp
    // scheduler 持有锁：等 300ms 让它扫完（5 张图 <100ms），再试
    await new Promise((r) => setTimeout(r, 300))
  }
  // 三次都 409：返回最后一次让调用方断言失败暴露问题（而非在这里 throw）
  return resp!
}

test.describe('资源库扫描触发', () => {
  test('对 seeded 库触发扫描返回 202 ACCEPTED，且 last_scanned_at 被更新', async () => {
    const ctx = await newAuthedContext()
    try {
      const libraryId = getSeededLibraryId()

      // 记录触发前的 last_scanned_at（globalSetup 已扫描过，应有值）。
      const before = await fetchLibraries(ctx)
      const beforeHit = before.find((l) => l.id === libraryId)
      expect(beforeHit, 'seeded 库应存在').toBeDefined()
      const beforeTs = beforeHit!.last_scanned_at
      expect(beforeTs, 'globalSetup 扫描后应已写入 last_scanned_at').not.toBeNull()

      // 触发扫描（seeded 库扫描在 globalSetup 已完成，SCANNING 锁已释放 → 必 202）。
      // scheduler 2s 频扫可能瞬时持锁 → 409；triggerScanAcceptingSchedulerConflict
      // 包了短重试，把"必为 202"放宽为"短时间内一定能拿到 202"。
      const resp = await triggerScanAcceptingSchedulerConflict(ctx, libraryId)
      expect(resp.status()).toBe(202)

      // 轮询 libraries 直到 last_scanned_at 推进到 afterTs（扫描完成后异步更新）。
      // 超时 15s 覆盖本地小夹具扫描 + DB 更新（通常 < 1s）。
      await expect.poll(
        async () => {
          const list = await fetchLibraries(ctx)
          const hit = list.find((l) => l.id === libraryId)
          return hit?.last_scanned_at ?? null
        },
        { timeout: 15_000, intervals: [500, 1_000, 2_000] },
      ).not.toBe(beforeTs)
    } finally {
      await ctx.dispose()
    }
  })

  test('对不存在的 library id 触发扫描返回 404', async () => {
    const ctx = await newAuthedContext()
    try {
      // 大随机 id，与 seeded 库 id（通常 1）不可能冲突。
      const ghostId = 999_987
      const resp = await triggerScan(ctx, ghostId)
      expect(resp.status()).toBe(404)
    } finally {
      await ctx.dispose()
    }
  })

  test('创建 → 扫描 → 删除 临时库的完整生命周期（不污染 seeded 库）', async () => {
    const ctx = await newAuthedContext()
    // OS 临时目录下建一个独立空目录作为临时库 base_path（合法可达路径）。
    const tmpBase = fs.mkdtempSync(path.join(os.tmpdir(), 'tagflow-e2e-libscan-'))
    // 放一个小图，使扫描能真实入库（而非空库）。
    const fixtureSrc = path.resolve(__dirname, '..', 'fixtures', 'library', 'Photos', 'sunset.jpg')
    fs.copyFileSync(fixtureSrc, path.join(tmpBase, 'tmp-scan.jpg'))

    let tmpId: number | null = null
    try {
      // 1. 创建临时库
      const createResp = await createLibrary(ctx, {
        name: 'e2e-temp-scan',
        protocol: 'local',
        base_path: tmpBase,
      })
      expect(createResp.status()).toBe(201)

      // 2. 按名查回 id（create 返回 201 无 body，list 取 id）
      const libs = await fetchLibraries(ctx)
      const hit = libs.find((l) => l.name === 'e2e-temp-scan')
      expect(hit, '刚创建的临时库应能列出').toBeDefined()
      tmpId = hit!.id

      // 3. 触发扫描 → 202
      //    注：scheduler 2s 频扫可能刚扫到这个新库并持锁 → 409；
      //    即便如此 scheduler 自己也会在 2s 内完成扫描，last_scanned_at 仍会推进。
      //    这里仍想拿到 202 以验证手动入口，故包一层 409 容忍重试。
      const scanResp = await triggerScanAcceptingSchedulerConflict(ctx, tmpId)
      expect(scanResp.status()).toBe(202)

      // 4. 轮询 last_scanned_at：从 null 推进到非 null，证明扫描完成。
      await expect.poll(
        async () => {
          const list = await fetchLibraries(ctx)
          return list.find((l) => l.id === tmpId)?.last_scanned_at ?? null
        },
        { timeout: 15_000, intervals: [500, 1_000, 2_000] },
      ).not.toBeNull()

      // 5. 删除临时库（cleanup）—— DELETE 返回 204。
      const delResp = await deleteLibrary(ctx, tmpId)
      expect(delResp.status()).toBe(204)
      tmpId = null

      // 6. seeded 库仍健在（未被影响）
      const seededId = getSeededLibraryId()
      const after = await fetchLibraries(ctx)
      expect(after.find((l) => l.id === seededId), 'seeded 库应仍存在').toBeDefined()
      expect(
        after.find((l) => l.name === 'e2e-temp-scan'),
        '临时库应已被删除',
      ).toBeUndefined()
    } finally {
      // 兜底 cleanup：即便断言失败也尽量不留垃圾（DB 行 + 临时目录）。
      if (tmpId !== null) {
        try {
          await deleteLibrary(ctx, tmpId)
        } catch {
          // 忽略：隔离 DB 跑完即销毁
        }
      }
      try {
        fs.rmSync(tmpBase, { recursive: true, force: true })
      } catch {
        // 忽略：os tmpdir 由 OS 定期清理
      }
      await ctx.dispose()
    }
  })
})
