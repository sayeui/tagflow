import { test, expect } from '@playwright/test'
import { ADMIN_PASSWORD, ADMIN_USERNAME } from '../lib/env'

/**
 * 登录 smoke：走 Login.vue 的 #username / #password，断言登录成功跳到 Home。
 *
 * 隔离前提（由 playwright.config.ts 保证）：后端用 TAGFLOW_ADMIN_PASSWORD 等测试 env
 * 拉起，DB / cache 都在 OS 临时目录。本用例不直接 seed 任何数据。
 */

test.describe('登录 smoke', () => {
  test('正确凭据登录后跳到 Home', async ({ page }) => {
    // 起点强制到 /login（路由守卫对未登录请求会重定向，但显式导航更稳）
    await page.goto('/login')

    // Login.vue 用 #username / #password（已在 PRD 探查中确认）
    await page.locator('#username').fill(ADMIN_USERNAME)
    await page.locator('#password').fill(ADMIN_PASSWORD)

    // 提交登录。等待导航完成到 Home（路由名 'Home'，path '/'）
    await Promise.all([
      page.waitForURL('**/', { timeout: 15_000 }),
      page.locator('button[type="submit"]').click(),
    ])

    // 登录成功的稳定信号：localStorage 写入 auth_token（auth store 约定）
    // 用 evaluate 读取，避免直接访问 localStorage 走禁用路径（本文件是测试，不在此约束内）
    const token = await page.evaluate(() => localStorage.getItem('auth_token'))
    expect(token, 'auth_token 应在登录后写入 localStorage').toBeTruthy()

    const username = await page.evaluate(() => localStorage.getItem('username'))
    expect(username).toBe(ADMIN_USERNAME)

    // Home 页面已挂载：侧边栏 "全部文件" 按钮存在（Home.vue 顶层结构）
    await expect(page.getByRole('button', { name: '全部文件' })).toBeVisible()
  })

  test('错误密码显示错误提示且不跳转', async ({ page }) => {
    await page.goto('/login')

    await page.locator('#username').fill(ADMIN_USERNAME)
    await page.locator('#password').fill('wrong-password-xxx')

    await page.locator('button[type="submit"]').click()

    // Login.vue 对 401 显示「用户名或密码错误」Toast
    await expect(page.getByText('用户名或密码错误')).toBeVisible({ timeout: 10_000 })

    // 仍停留在 /login（未跳到 Home）
    await expect(page).toHaveURL(/\/login/)

    // 未写入 token
    const token = await page.evaluate(() => localStorage.getItem('auth_token'))
    expect(token).toBeNull()
  })
})
