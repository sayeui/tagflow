<script setup lang="ts">
import { ref } from 'vue'
import { useRouter } from 'vue-router'
import { useAuthStore } from '@/stores/auth'
import { authApi } from '@/api/http'
import { FolderOpen } from 'lucide-vue-next'
import Toast from '@/components/Toast.vue'

const router = useRouter()
const authStore = useAuthStore()

const username = ref('')
const password = ref('')
const error = ref('')
const loading = ref(false)

const handleLogin = async () => {
  loading.value = true

  try {
    const response = await authApi.login(username.value, password.value)
    const { token } = response.data
    authStore.setToken(token, username.value)
    router.push('/')
  } catch (err: any) {
    if (err.response?.status === 401) {
      error.value = '用户名或密码错误'
    } else {
      error.value = '登录失败，请稍后重试'
    }
  } finally {
    loading.value = false
  }
}
</script>

<template>
  <!-- Toast 提示 -->
  <Toast
    v-if="error"
    :message="error"
    type="error"
    :duration="4000"
    @close="error = ''"
  />

  <div class="min-h-screen flex items-center justify-center bg-gray-50 px-4">
    <div class="max-w-md w-full">
      <!-- Logo -->
      <div class="text-center mb-8">
        <div class="flex items-center justify-center text-blue-600 mb-2">
          <FolderOpen class="w-12 h-12" />
        </div>
        <h1 class="text-3xl font-bold text-gray-900">TagFlow</h1>
        <p class="text-gray-500 mt-2">登录以访问您的文件管理系统</p>
      </div>

      <!-- 登录卡片 -->
      <div class="bg-white rounded-xl shadow-lg p-8">
        <form @submit.prevent="handleLogin" class="space-y-6">
          <!-- 用户名 -->
          <div>
            <label for="username" class="block text-sm font-medium text-gray-700 mb-2">
              用户名
            </label>
            <input
              id="username"
              v-model="username"
              type="text"
              required
              autofocus
              class="w-full px-4 py-2 border border-gray-300 rounded-lg focus:ring-2 focus:ring-blue-500 focus:border-blue-500 outline-none transition-colors"
              placeholder="请输入用户名"
            />
          </div>

          <!-- 密码 -->
          <div>
            <label for="password" class="block text-sm font-medium text-gray-700 mb-2">
              密码
            </label>
            <input
              id="password"
              v-model="password"
              type="password"
              required
              class="w-full px-4 py-2 border border-gray-300 rounded-lg focus:ring-2 focus:ring-blue-500 focus:border-blue-500 outline-none transition-colors"
              placeholder="请输入密码"
            />
          </div>

          <!-- 登录按钮 -->
          <button
            type="submit"
            :disabled="loading"
            class="w-full bg-blue-600 text-white py-2.5 rounded-lg font-medium hover:bg-blue-700 focus:ring-4 focus:ring-blue-200 disabled:opacity-50 disabled:cursor-not-allowed transition-colors flex items-center justify-center"
          >
            <svg v-if="loading" class="animate-spin rounded-full h-5 w-5 border-2 border-white border-t-transparent mr-2" viewBox="0 0 24 24">
              <circle class="opacity-25" cx="12" cy="12" r="10" stroke="currentColor" stroke-width="4" fill="none" />
              <path class="opacity-75" fill="currentColor" d="M4 12a8 8 0 018-8V0C5.373 0 0 5.373 0 12h4zm2 5.291A7.962 7.962 0 014 12H0c0 3.042 1.135 5.824 3 7.938l3-2.647z" />
            </svg>
            {{ loading ? '登录中...' : '登录' }}
          </button>
        </form>

        <!-- 默认凭据提示 -->
        <div class="mt-6 p-4 bg-blue-50 rounded-lg">
          <p class="text-sm text-blue-800">
            <span class="font-medium">默认凭据：</span>
          </p>
          <p class="text-sm text-blue-600 mt-1">
            用户名: <code class="bg-blue-100 px-1.5 py-0.5 rounded">admin</code>
            &nbsp;&nbsp;密码: <code class="bg-blue-100 px-1.5 py-0.5 rounded">PhVENfYaWv</code>
          </p>
        </div>
      </div>

      <!-- 页脚 -->
      <p class="text-center text-sm text-gray-400 mt-8">
        TagFlow - 轻量级文件管理系统
      </p>
    </div>
  </div>
</template>
