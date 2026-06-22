/**
 * UI 登录助手：填表单登录到 Home，并等待登录后的稳定信号出现。
 *
 * 复用 globalSetup 已 seed 的同一组凭据；每个 spec 默认拿到干净页面（Playwright
 * fixtures 的 isolated context），因此需各自走一次登录流程。
 */

import type { Page } from '@playwright/test'
import { expect } from '@playwright/test'
import { ADMIN_PASSWORD, ADMIN_USERNAME } from './env'

/**
 * 走 Login.vue 表单登录，等待跳到 Home。
 *
 * 与直接注入 localStorage 相比，走真实表单更接近用户路径，也覆盖登录请求/路由守卫
 * 的回归；登录失败会显式断言失败，方便定位。
 */
export async function loginViaUi(page: Page): Promise<void> {
  await page.goto('/login')

  await page.locator('#username').fill(ADMIN_USERNAME)
  await page.locator('#password').fill(ADMIN_PASSWORD)

  await Promise.all([
    page.waitForURL('**/', { timeout: 15_000 }),
    page.locator('button[type="submit"]').click(),
  ])

  // 登录成功的稳定信号：侧栏「全部文件」按钮可见（Home 已挂载）。
  await expect(page.getByTestId('all-files-button')).toBeVisible({ timeout: 10_000 })
}
