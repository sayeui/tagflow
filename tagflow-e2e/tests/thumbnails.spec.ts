import { test, expect } from '@playwright/test'
import { fetchFiles, fetchThumbnail, newAuthedContext } from '../lib/api'
import { isFfmpegAvailable } from '../lib/env'

/**
 * 缩略图懒加载 e2e（PR3）。
 *
 * 链路：扫描器为媒体文件入列 thumb 任务 → worker（5s 轮询）→ ffmpeg 生成
 * → 落盘 cache_dir/{file_id}.webp → GET /files/:id/thumbnail 从 404 转 200。
 *
 * 稳定性要点（详见 prd.md「异步等待策略」）：
 *   globalSetup 只等"文件入库"，不等缩略图。但 worker 5s 一轮，从 globalSetup
 *   结束到本用例跑起来这期间缩略图可能已被生成并缓存（甚至 PR2 那次运行就已缓存）。
 *   所以本用例**先探测当前态**：
 *     - 若已是 200：断言"缩略图稳定可访问"（content-type 正确、连续两次 200）。
 *     - 若是 404：用 expect.poll 轮询直到 200（覆盖 worker 一轮 + ffmpeg 处理余量）。
 *   两条路径都让用例在任意缓存状态下稳定通过，不依赖精确时序。
 *
 * ffmpeg skip 兜底：ffmpeg 不可用时 worker 永远生成不出 webp，缩略图会一直 404，
 *   用例会超时挂死。故 isFfmpegAvailable() === false 时整组用例 skip，其余 spec 照跑。
 */

test.describe('缩略图懒加载', () => {
  // ffmpeg 缺失时缩略图用例无意义（worker 永远 404），整组跳过并给明确原因。
  test.beforeAll(() => {
    test.skip(!isFfmpegAvailable(), 'ffmpeg 不可用，缩略图相关用例跳过（其余 spec 照跑）')
  })

  test('GET /files/:id/thumbnail 最终返回 200 + image/webp（覆盖 404→200 转换或稳定 200）', async () => {
    const ctx = await newAuthedContext()
    try {
      // 取一个已入库的文件 id（用 seeded 库，globalSetup 已 seed 5 个图片）。
      const { items } = await fetchFiles(ctx, { limit: 10 })
      expect(items.length, 'seeded 库应有文件可取').toBeGreaterThan(0)
      const fileId = items[0].id

      // 探测当前态：决定走"404→200 转换"还是"稳定 200"路径。
      const probe = await fetchThumbnail(ctx, fileId)
      const probeStatus = probe.status()
      // 读完 body 释放连接（404 可能带短 body，200 走流式也需消费）
      await probe.body()

      if (probeStatus === 404) {
        // 路径 A：尚未生成 → 轮询直到 200。
        // 超时 20s 覆盖 worker 一轮 5s + ffmpeg 处理（小图 < 1s）+ 余量。
        await expect.poll(
          async () => {
            const r = await fetchThumbnail(ctx, fileId)
            const s = r.status()
            await r.body()
            return s
          },
          { timeout: 20_000, intervals: [1_000, 2_000, 2_000] },
        ).toBe(200)
      } else {
        // 路径 B：已是 200（或其它非 404 不预期）→ 断言稳定 200。
        expect(probeStatus, '已生成的缩略图应稳定返回 200').toBe(200)
      }

      // 最终断言：状态码 200 + Content-Type 为 image/webp（后端 file.rs:297 固定返回）。
      const final = await fetchThumbnail(ctx, fileId)
      expect(final.status()).toBe(200)
      expect(final.headers()['content-type']).toContain('image/webp')
      // body 非空（webp 头 0x52 0x49 0x46 0x46 = "RIFF"）
      const body = await final.body()
      expect(body.length, '缩略图 body 不应为空').toBeGreaterThan(0)
      expect(body.slice(0, 4)).toEqual(Buffer.from('RIFF'))
    } finally {
      await ctx.dispose()
    }
  })
})
