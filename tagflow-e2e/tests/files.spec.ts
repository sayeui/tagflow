import { test, expect } from '@playwright/test'
import { loginViaUi } from '../lib/auth'
import { EXPECTED_FILE_COUNT } from '../lib/env'
import { fetchFiles, newAuthedContext } from '../lib/api'

/**
 * 文件列表 / 文件名搜索 / 视图切换 / 标签树 e2e（PR2）。
 *
 * 前置：globalSetup 已 seed 一个指向 fixtures/library 的本地资源库，6 个文件入库
 * （5 张图片 + 1 个非媒体 notes.txt）。
 * 文件清单（见 prd.md）：
 *   Photos/sunset.jpg、Photos/风景.jpg、notes.txt、
 *   Projects/2024/report.png、Projects/2024/设计稿.png、
 *   Reports/季度总结.png
 *
 * 注意：FileGrid/FileList 用 RecycleScroller，只渲染可见项。fixtures 6 个文件在网格
 * 单行 / 列表 6 行都完全可见（不触发回收），故 getByTestId 可拿到全部。不要写硬 sleep、
 * 不要依赖固定索引。
 */

test.describe('文件浏览', () => {
  test.beforeEach(async ({ page }) => {
    await loginViaUi(page)
  })

  test('文件列表渲染全部 6 个文件（卡片视图）', async ({ page }) => {
    // 等 store.fetchFiles() 完成；文件卡片出现即说明列表已挂载。
    // Playwright 定位器自带自动重试，覆盖 300ms 防抖 + 网络往返。
    const cards = page.getByTestId('file-card')
    await expect(cards).toHaveCount(EXPECTED_FILE_COUNT, { timeout: 15_000 })

    // 6 个文件名都能在 DOM 中找到（虚拟滚动已渲染全部，过滤定位器会重试）。
    for (const filename of [
      'sunset.jpg',
      '风景.jpg',
      'report.png',
      '设计稿.png',
      '季度总结.png',
      'notes.txt',
    ]) {
      await expect(
        page.getByTestId('file-card').filter({ hasText: filename }),
      ).toBeVisible()
    }

    // 底部文件计数文案："共 6 / 6 个文件"
    await expect(page.getByText(/共\s*6\s*\/\s*6\s*个文件/)).toBeVisible()
  })

  test('文件名搜索：中文关键词收窄到匹配项，清空后恢复', async ({ page }) => {
    // 先确认列表已渲染（避免在 0 项时断言过滤结果）
    await expect(page.getByTestId('file-card')).toHaveCount(EXPECTED_FILE_COUNT, {
      timeout: 15_000,
    })

    const search = page.getByTestId('search-input')

    // 中文关键词「设计」→ 仅「设计稿.png」
    await search.fill('设计')
    await expect(page.getByTestId('file-card')).toHaveCount(1, { timeout: 10_000 })
    await expect(
      page.getByTestId('file-card').filter({ hasText: '设计稿.png' }),
    ).toBeVisible()

    // 英文关键词「report」→ 仅「report.png」（后端 LIKE 不区分 ASCII 大小写）
    await search.fill('report')
    await expect(page.getByTestId('file-card')).toHaveCount(1, { timeout: 10_000 })
    await expect(
      page.getByTestId('file-card').filter({ hasText: 'report.png' }),
    ).toBeVisible()

    // 拼音/中文公共子串「.png」→ 命中 3 个 png
    await search.fill('.png')
    await expect(page.getByTestId('file-card')).toHaveCount(3, { timeout: 10_000 })

    // 无匹配
    await search.fill('不存在的文件名-xxx')
    await expect(page.getByTestId('file-card')).toHaveCount(0, { timeout: 10_000 })
    // 空列表提示
    await expect(page.getByText('暂无文件')).toBeVisible()

    // 清空 → 恢复全部
    await search.fill('')
    await expect(page.getByTestId('file-card')).toHaveCount(EXPECTED_FILE_COUNT, {
      timeout: 10_000,
    })
  })

  test('视图切换：卡片 ↔ 列表', async ({ page }) => {
    await expect(page.getByTestId('file-card')).toHaveCount(EXPECTED_FILE_COUNT, {
      timeout: 15_000,
    })

    // 初始：卡片视图（file-area data-view-mode="grid"）
    const fileArea = page.getByTestId('file-area')
    await expect(fileArea).toHaveAttribute('data-view-mode', 'grid')

    // 切到列表视图
    await page.getByTestId('view-list-button').click()
    await expect(fileArea).toHaveAttribute('data-view-mode', 'list')

    // 列表视图同样渲染 6 行（FileList 单行 item，全部可见）
    await expect(page.getByTestId('file-card')).toHaveCount(EXPECTED_FILE_COUNT, {
      timeout: 10_000,
    })

    // 切回卡片视图
    await page.getByTestId('view-grid-button').click()
    await expect(fileArea).toHaveAttribute('data-view-mode', 'grid')
    await expect(page.getByTestId('file-card')).toHaveCount(EXPECTED_FILE_COUNT, {
      timeout: 10_000,
    })
  })

  test('标签树渲染路径标签的嵌套结构', async ({ page }) => {
    // 标签树容器
    const tree = page.getByTestId('tag-tree')
    await expect(tree).toBeVisible({ timeout: 15_000 })

    // 「路径」分区标题可见（Home.vue 用 v-for 渲染分组标题 "路径"）。
    // 用 group 容器的直接子（>）限定，避免误命中 tag-node 自身（tag-node 也带
    // data-tag-category）。
    const pathGroup = tree.locator(':scope > [data-tag-category="path"]')
    await expect(pathGroup).toBeVisible()

    // 三个一级路径段（path 分区下作为根节点出现）
    for (const name of ['Photos', 'Projects', 'Reports']) {
      await expect(
        pathGroup.locator(`[data-testid="tag-node"][data-tag-name="${name}"]`),
      ).toBeVisible()
    }

    // Projects 下应有子节点 2024（嵌套结构，递归组件）。
    // collapsed 默认 false（展开），故子节点在 DOM 中可见。
    const projectsNode = pathGroup.locator(
      '[data-testid="tag-node"][data-tag-name="Projects"]',
    )
    await expect(projectsNode).toBeVisible()
    await expect(
      tree.locator('[data-testid="tag-node"][data-tag-name="2024"]'),
    ).toBeVisible()
  })

  test('非媒体文件不发起缩略图请求（按 MEDIA_EXTENSIONS 白名单过滤）', async ({ page }) => {
    // 先从 API 拿到 notes.txt 的 file id（非媒体）与所有媒体文件 id（对照）。
    const ctx = await newAuthedContext()
    let notesTxtId: number | undefined
    let mediaIds: number[] = []
    try {
      const { items } = await fetchFiles(ctx, { limit: 50 })
      const notes = items.find((f) => f.filename === 'notes.txt')
      expect(notes, 'fixtures 应包含 notes.txt 作为非媒体夹具').toBeDefined()
      notesTxtId = notes!.id
      // 媒体文件：扩展名在后端 MEDIA_EXTENSIONS 集合内（jpg/jpeg/png/gif/webp/bmp/
      // mp4/mov/m4v/mkv/avi/webm）。fixtures 全部媒体都是 jpg/png。
      const mediaExt = ['jpg', 'jpeg', 'png', 'gif', 'webp', 'bmp', 'mp4', 'mov', 'm4v', 'mkv', 'avi', 'webm']
      mediaIds = items
        .filter((f) => !!f.extension && mediaExt.includes(f.extension.toLowerCase()))
        .map((f) => f.id)
    } finally {
      await ctx.dispose()
    }
    expect(notesTxtId, 'notes.txt id 必须已解析').toBeDefined()
    expect(mediaIds.length, 'fixtures 至少有一个媒体文件作为对照').toBeGreaterThan(0)

    // 拦截缩略图请求：page.on('request') 收集所有命中 /thumbnail 的 URL。
    // beforeEach 已 loginViaUi 并跳到 Home；Home 挂载后 store.fetchFiles 会触发
    // FileGrid 渲染 <img> → 缩略图请求。为确保从一个「干净的请求起点」开始捕获，
    // 先挂监听，再 page.reload() 重新加载 Home（保留 localStorage 中的 token，
    // 路由守卫放行，FileGrid 重新挂载并发出全部缩略图请求）。
    const thumbnailFileIds = new Set<number>()
    page.on('request', (req) => {
      const m = req.url().match(/\/api\/v1\/files\/(\d+)\/thumbnail/)
      if (m) thumbnailFileIds.add(Number(m[1]))
    })

    await page.reload()

    // 等文件列表渲染稳定（全部 6 张卡片可见 = store.fetchFiles 已完成、<img> 已挂载/触发请求）。
    await expect(page.getByTestId('file-card')).toHaveCount(EXPECTED_FILE_COUNT, {
      timeout: 15_000,
    })
    // 再让出一小段时间，让 RecycleScroller 内 lazy <img> 的请求有机会发出（lazy img
    // 进入视口后才发，本场景 6 个文件全部首屏可见，几毫秒即可）。
    await page.waitForLoadState('networkidle')

    // 断言：notes.txt 的 id 绝不应出现在 thumbnail 请求里（前端 v-if 拦在渲染层）。
    expect(
      thumbnailFileIds.has(notesTxtId!),
      `notes.txt (id=${notesTxtId}) 不应被请求缩略图，实际请求 id：[${[...thumbnailFileIds].join(', ')}]`,
    ).toBe(false)

    // 对照：至少有一个媒体文件被请求过缩略图（证明请求通路正常，不是被全站屏蔽）。
    const mediaRequested = mediaIds.some((id) => thumbnailFileIds.has(id))
    expect(
      mediaRequested,
      `至少一个媒体文件应被请求缩略图（mediaIds=${mediaIds.join(',')}，实际请求=[${[...thumbnailFileIds].join(', ')}]）`,
    ).toBe(true)
  })
})
