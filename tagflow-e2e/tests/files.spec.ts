import { test, expect } from '@playwright/test'
import { loginViaUi } from '../lib/auth'
import { EXPECTED_FILE_COUNT } from '../lib/env'

/**
 * 文件列表 / 文件名搜索 / 视图切换 / 标签树 e2e（PR2）。
 *
 * 前置：globalSetup 已 seed 一个指向 fixtures/library 的本地资源库，5 张图片已入库。
 * 文件清单（见 prd.md）：
 *   Photos/sunset.jpg、Photos/风景.jpg、
 *   Projects/2024/report.png、Projects/2024/设计稿.png、
 *   Reports/季度总结.png
 *
 * 注意：FileGrid/FileList 用 RecycleScroller，只渲染可见项。fixtures 5 个文件在网格
 * 单行 / 列表 5 行都完全可见（不触发回收），故 getByTestId 可拿到全部。不要写硬 sleep、
 * 不要依赖固定索引。
 */

test.describe('文件浏览', () => {
  test.beforeEach(async ({ page }) => {
    await loginViaUi(page)
  })

  test('文件列表渲染全部 5 个文件（卡片视图）', async ({ page }) => {
    // 等 store.fetchFiles() 完成；文件卡片出现即说明列表已挂载。
    // Playwright 定位器自带自动重试，覆盖 300ms 防抖 + 网络往返。
    const cards = page.getByTestId('file-card')
    await expect(cards).toHaveCount(EXPECTED_FILE_COUNT, { timeout: 15_000 })

    // 5 个文件名都能在 DOM 中找到（虚拟滚动已渲染全部，过滤定位器会重试）。
    for (const filename of [
      'sunset.jpg',
      '风景.jpg',
      'report.png',
      '设计稿.png',
      '季度总结.png',
    ]) {
      await expect(
        page.getByTestId('file-card').filter({ hasText: filename }),
      ).toBeVisible()
    }

    // 底部文件计数文案："共 5 / 5 个文件"
    await expect(page.getByText(/共\s*5\s*\/\s*5\s*个文件/)).toBeVisible()
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

    // 列表视图同样渲染 5 行（FileList 单行 item，全部可见）
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
})
