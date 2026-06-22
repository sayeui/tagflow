import { test, expect } from '@playwright/test'
import fs from 'node:fs'
import os from 'node:os'
import path from 'node:path'
import {
  createLibrary,
  deleteLibrary,
  fetchLibraries,
  fetchTagTree,
  findTagNodeByName,
  newAuthedContext,
  triggerScan,
} from '../lib/api'
import { getSeededLibraryId } from '../lib/env'

/**
 * 标签树清理 e2e（孤儿标签清理 PR3）。
 *
 * 后端契约（tagflow-core/src/api/{tag,library,file}.rs）：
 *   - `get_tag_tree` 只返回有 status=1 文件关联（含子树）的标签；删库后该库
 *     独有的标签会被 `delete_library` 触发的 `cleanup_orphan_tag` 真正从 tags 表删除。
 *   - 跨库共享标签（被另一库的在线文件也关联）保留。
 *
 * 测试策略：
 *   - 创建一个带「独有子目录」的临时库（子目录名带随机串，确保 seeded 库不会有同名 path 标签）。
 *   - 扫描后断言：临时库独有 path 标签出现，seeded 库的 path 标签仍在。
 *   - 删除临时库后断言：临时库独有 path 标签消失（被 cleanup_orphan_tag 清掉），
 *     seeded 库的 path 标签仍健在（跨库共享标签保留语义）。
 *
 * 隔离：临时库用后即删，不污染 seeded 库；断言失败也走 finally 清理。
 */

/** 触发扫描并容忍与 scheduler 的瞬时 409 冲突（与 library-scan.spec.ts 同策略）。 */
async function triggerScanAcceptingSchedulerConflict(
  ctx: Parameters<typeof triggerScan>[0],
  libraryId: number,
): ReturnType<typeof triggerScan> {
  const maxAttempts = 3
  let resp: Awaited<ReturnType<typeof triggerScan>> | null = null
  for (let i = 0; i < maxAttempts; i++) {
    resp = await triggerScan(ctx, libraryId)
    if (resp.status() !== 409) return resp
    await new Promise((r) => setTimeout(r, 300))
  }
  return resp!
}

test.describe('标签树清理（删库孤儿清理 + 跨库共享保留）', () => {
  test('删库后独有标签消失，跨库共享标签保留', async () => {
    const ctx = await newAuthedContext()

    // 临时库 base_path：os tmpdir 下建一个带「独有子目录」的目录结构。
    // uniqueFolder 名带随机串 → 临时库独有 path 标签，seeded 库不可能重名。
    const rand = Date.now().toString(36) + Math.random().toString(36).slice(2, 6)
    const tmpBase = fs.mkdtempSync(path.join(os.tmpdir(), 'tagflow-tagclean-'))
    const uniqueFolder = `UniqueTagFolder-${rand}`
    const subDir = path.join(tmpBase, uniqueFolder)
    fs.mkdirSync(subDir, { recursive: true })

    // 复制一个 jpg 进子目录，扫描后生成 path:UniqueTagFolder-xxx → ext:jpg → type:image 等关联。
    const fixtureSrc = path.resolve(
      __dirname,
      '..',
      'fixtures',
      'library',
      'Photos',
      'sunset.jpg',
    )
    fs.copyFileSync(fixtureSrc, path.join(subDir, 'tagclean.jpg'))

    let tmpId: number | null = null
    try {
      // 1. 创建临时库 + 扫描
      const createResp = await createLibrary(ctx, {
        name: 'e2e-tagclean',
        protocol: 'local',
        base_path: tmpBase,
      })
      expect(createResp.status()).toBe(201)

      const libs = await fetchLibraries(ctx)
      const hit = libs.find((l) => l.name === 'e2e-tagclean')
      expect(hit, '刚创建的临时库应能列出').toBeDefined()
      tmpId = hit!.id

      const scanResp = await triggerScanAcceptingSchedulerConflict(ctx, tmpId)
      expect(scanResp.status()).toBe(202)

      // 等扫描完成：last_scanned_at 从 null 推进到非 null。
      await expect
        .poll(
          async () => {
            const list = await fetchLibraries(ctx)
            return list.find((l) => l.id === tmpId)?.last_scanned_at ?? null
          },
          { timeout: 15_000, intervals: [500, 1_000, 2_000] },
        )
        .not.toBeNull()

      // 2. 扫描后：临时库独有 path 标签应出现
      //    使用轮询：scheduler 可能还未扫到（last_scanned_at 已变也涵盖文件入库，
      //    但 path 标签写入是扫描链路内的子步骤，给个独立的短轮询余量更稳）。
      await expect
        .poll(
          async () => {
            const tree = await fetchTagTree(ctx)
            return findTagNodeByName(tree, uniqueFolder) !== undefined
          },
          { timeout: 10_000, intervals: [500, 1_000] },
        )
        .toBe(true)

      // 3. 记录 seeded 库一个 path 标签（作为跨库共享对照：seeded 库的 Photos/Projects/Reports
      //    仅 seeded 库有，删临时库不应影响）。这里用 seeded 库的 Photos 节点作参照。
      const beforeTree = await fetchTagTree(ctx)
      const seededPhotos = findTagNodeByName(beforeTree, 'Photos')
      expect(seededPhotos, 'seeded 库的 Photos path 标签扫描后应存在').toBeDefined()

      // 4. 删除临时库
      const delResp = await deleteLibrary(ctx, tmpId)
      expect(delResp.status()).toBe(204)
      tmpId = null

      // 5. 删库后：临时库独有 path 标签应消失（cleanup_orphan_tag 真清理 tags 表）
      //    给短轮询余量：cleanup_orphan_tag 是 delete_library 同步逻辑内的，删库
      //    返回 204 即已完成；但为防 scheduler 并发扫描带来的瞬时态，包一层轮询。
      await expect
        .poll(
          async () => {
            const tree = await fetchTagTree(ctx)
            return findTagNodeByName(tree, uniqueFolder)
          },
          { timeout: 10_000, intervals: [500, 1_000] },
        )
        .toBeUndefined()

      // 6. seeded 库的 Photos 标签仍健在（跨库共享保留，未受牵连）
      const afterTree = await fetchTagTree(ctx)
      const seededPhotosAfter = findTagNodeByName(afterTree, 'Photos')
      expect(seededPhotosAfter, '删临时库后 seeded 库的 Photos 标签应仍存在').toBeDefined()

      // 7. seeded 库本身仍存在
      const seededId = getSeededLibraryId()
      const after = await fetchLibraries(ctx)
      expect(after.find((l) => l.id === seededId), 'seeded 库应仍存在').toBeDefined()
      expect(after.find((l) => l.name === 'e2e-tagclean'), '临时库应已被删除').toBeUndefined()
    } finally {
      // 兜底 cleanup：即便断言失败也尽量不留垃圾。
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

  test('扫描删文件后离线文件的独有标签隐藏（软删语义保留）', async () => {
    /**
     * 后端契约（tagflow-core/src/engine/scanner/mod.rs::mark_as_lost + api/tag.rs::get_tag_tree）：
     *   - 文件物理消失 → files.status=0（软删，不真删，保留以支持移动检测/恢复）。
     *   - 标签树过滤 status=1，离线文件的标签若没有其它在线文件关联则隐藏。
     *   - 文件恢复后 status→1，标签自动回归。
     *
     * 稳定性说明（详见 prd.md 与 library-scan.spec.ts 的 409 注释）：
     *   scheduler 每 2s 扫描所有库，删除 fixture 文件后会在 2s 内被 mark_as_lost。
     *   本用例用「临时库 + 临时文件」隔离，避免影响 seeded 库的 fixtures。
     *   用 expect.poll 轮询直到 tag tree 收敛到目标状态（覆盖 2s 扫描周期）。
     */
    const ctx = await newAuthedContext()

    // 临时库结构：<tmpBase>/<uniqueFolder>/<file>.jpg
    const rand = Date.now().toString(36) + Math.random().toString(36).slice(2, 6)
    const tmpBase = fs.mkdtempSync(path.join(os.tmpdir(), 'tagflow-softdel-'))
    const uniqueFolder = `SoftDelFolder-${rand}`
    const subDir = path.join(tmpBase, uniqueFolder)
    fs.mkdirSync(subDir, { recursive: true })

    const fixtureSrc = path.resolve(
      __dirname,
      '..',
      'fixtures',
      'library',
      'Photos',
      'sunset.jpg',
    )
    const tmpFile = path.join(subDir, 'softdel.jpg')
    fs.copyFileSync(fixtureSrc, tmpFile)

    let tmpId: number | null = null
    try {
      // 1. 创建临时库 + 扫描
      const createResp = await createLibrary(ctx, {
        name: 'e2e-softdel',
        protocol: 'local',
        base_path: tmpBase,
      })
      expect(createResp.status()).toBe(201)

      const libs = await fetchLibraries(ctx)
      const hit = libs.find((l) => l.name === 'e2e-softdel')
      expect(hit, '刚创建的临时库应能列出').toBeDefined()
      tmpId = hit!.id

      await triggerScanAcceptingSchedulerConflict(ctx, tmpId)

      // 等扫描完成 + 独有 path 标签出现
      await expect
        .poll(
          async () => {
            const list = await fetchLibraries(ctx)
            return list.find((l) => l.id === tmpId)?.last_scanned_at ?? null
          },
          { timeout: 15_000, intervals: [500, 1_000, 2_000] },
        )
        .not.toBeNull()
      await expect
        .poll(
          async () => {
            const tree = await fetchTagTree(ctx)
            return findTagNodeByName(tree, uniqueFolder) !== undefined
          },
          { timeout: 10_000, intervals: [500, 1_000] },
        )
        .toBe(true)

      // 2. 物理删除文件（scheduler 下一轮会 mark_as_lost status=0）
      fs.unlinkSync(tmpFile)

      // 3. 轮询：独有 path 标签应消失（离线文件的标签被标签树过滤）
      //    scheduler 2s 一轮 + tag tree 过滤即时生效，给 15s 余量。
      await expect
        .poll(
          async () => {
            const tree = await fetchTagTree(ctx)
            return findTagNodeByName(tree, uniqueFolder)
          },
          { timeout: 15_000, intervals: [500, 1_000, 2_000] },
        )
        .toBeUndefined()

      // 4. 软删语义验证：恢复文件后独有标签回归（status→1）
      fs.copyFileSync(fixtureSrc, tmpFile)
      await expect
        .poll(
          async () => {
            const tree = await fetchTagTree(ctx)
            return findTagNodeByName(tree, uniqueFolder) !== undefined
          },
          { timeout: 15_000, intervals: [500, 1_000, 2_000] },
        )
        .toBe(true)
    } finally {
      if (tmpId !== null) {
        try {
          await deleteLibrary(ctx, tmpId)
        } catch {
          // 忽略
        }
      }
      try {
        fs.rmSync(tmpBase, { recursive: true, force: true })
      } catch {
        // 忽略
      }
      await ctx.dispose()
    }
  })
})
