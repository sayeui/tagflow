<script setup lang="ts">
import { ref } from 'vue'
import { useRouter } from 'vue-router'
import { useAuthStore } from '@/stores/auth'
import { authApi } from '@/api/http'
import { ArrowLeft, Shield } from 'lucide-vue-next'

const router = useRouter()
const authStore = useAuthStore()

const oldPassword = ref('')
const newPassword = ref('')
const confirmPassword = ref('')
const error = ref('')
const success = ref('')
const loading = ref(false)

const validateForm = (): boolean => {
  error.value = ''

  if (!oldPassword.value) {
    error.value = '请输入当前密码'
    return false
  }

  if (!newPassword.value) {
    error.value = '请输入新密码'
    return false
  }

  if (newPassword.value.length < 6) {
    error.value = '新密码长度至少为 6 位'
    return false
  }

  if (newPassword.value !== confirmPassword.value) {
    error.value = '两次输入的新密码不一致'
    return false
  }

  return true
}

const handleSubmit = async () => {
  success.value = ''

  if (!validateForm()) {
    return
  }

  loading.value = true

  try {
    await authApi.updatePassword(oldPassword.value, newPassword.value)
    success.value = '密码修改成功！2 秒后将自动退出登录，请使用新密码重新登录。'

    // 2 秒后退出登录
    setTimeout(() => {
      authStore.logout()
      router.push('/login')
    }, 2000)
  } catch (err: any) {
    if (err.response?.status === 403) {
      error.value = '当前密码错误'
    } else {
      error.value = '密码修改失败，请稍后重试'
    }
  } finally {
    loading.value = false
  }
}
</script>

<template>
  <div class="max-w-2xl mx-auto p-6">
    <!-- 页面标题 -->
    <div class="mb-6">
      <button
        @click="router.push('/')"
        class="flex items-center text-gray-600 hover:text-gray-900 mb-4 transition-colors"
      >
        <ArrowLeft class="w-5 h-5 mr-1" />
        返回首页
      </button>
      <div class="flex items-center">
        <Shield class="w-8 h-8 text-blue-600 mr-3" />
        <h1 class="text-2xl font-bold text-gray-900">安全设置</h1>
      </div>
      <p class="text-gray-500 mt-1">修改您的登录密码</p>
    </div>

    <!-- 修改密码表单 -->
    <div class="bg-white rounded-xl shadow-lg p-8">
      <form @submit.prevent="handleSubmit" class="space-y-6">
        <!-- 成功提示 -->
        <div v-if="success" class="bg-green-50 text-green-700 px-4 py-3 rounded-lg flex items-start">
          <svg class="w-5 h-5 mr-2 mt-0.5 flex-shrink-0" fill="currentColor" viewBox="0 0 20 20">
            <path fill-rule="evenodd" d="M10 18a8 8 0 100-16 8 8 0 000 16zm3.707-9.293a1 1 0 00-1.414-1.414L9 10.586 7.707 9.293a1 1 0 00-1.414 1.414l2 2a1 1 0 001.414 0l4-4z" clip-rule="evenodd" />
          </svg>
          <span>{{ success }}</span>
        </div>

        <!-- 错误提示 -->
        <div v-if="error" class="bg-red-50 text-red-600 px-4 py-3 rounded-lg flex items-start">
          <svg class="w-5 h-5 mr-2 mt-0.5 flex-shrink-0" fill="currentColor" viewBox="0 0 20 20">
            <path fill-rule="evenodd" d="M10 18a8 8 0 100-16 8 8 0 000 16zM8.707 7.293a1 1 0 00-1.414 1.414L8.586 10l-1.293 1.293a1 1 0 101.414 1.414L10 11.414l1.293 1.293a1 1 0 001.414-1.414L11.414 10l1.293-1.293a1 1 0 00-1.414-1.414L10 8.586 8.707 7.293z" clip-rule="evenodd" />
          </svg>
          <span>{{ error }}</span>
        </div>

        <!-- 当前密码 -->
        <div>
          <label for="old-password" class="block text-sm font-medium text-gray-700 mb-2">
            当前密码
          </label>
          <input
            id="old-password"
            v-model="oldPassword"
            type="password"
            required
            class="w-full px-4 py-2 border border-gray-300 rounded-lg focus:ring-2 focus:ring-blue-500 focus:border-blue-500 outline-none transition-colors"
            placeholder="请输入当前密码"
          />
        </div>

        <!-- 新密码 -->
        <div>
          <label for="new-password" class="block text-sm font-medium text-gray-700 mb-2">
            新密码
          </label>
          <input
            id="new-password"
            v-model="newPassword"
            type="password"
            required
            minlength="6"
            class="w-full px-4 py-2 border border-gray-300 rounded-lg focus:ring-2 focus:ring-blue-500 focus:border-blue-500 outline-none transition-colors"
            placeholder="请输入新密码（至少 6 位）"
          />
          <p class="mt-1 text-sm text-gray-500">密码长度至少为 6 位</p>
        </div>

        <!-- 确认新密码 -->
        <div>
          <label for="confirm-password" class="block text-sm font-medium text-gray-700 mb-2">
            确认新密码
          </label>
          <input
            id="confirm-password"
            v-model="confirmPassword"
            type="password"
            required
            class="w-full px-4 py-2 border border-gray-300 rounded-lg focus:ring-2 focus:ring-blue-500 focus:border-blue-500 outline-none transition-colors"
            placeholder="请再次输入新密码"
          />
        </div>

        <!-- 提交按钮 -->
        <div class="flex gap-3">
          <button
            type="button"
            @click="router.push('/')"
            class="flex-1 px-4 py-2.5 border border-gray-300 text-gray-700 rounded-lg font-medium hover:bg-gray-50 focus:ring-4 focus:ring-gray-200 transition-colors"
          >
            取消
          </button>
          <button
            type="submit"
            :disabled="loading"
            class="flex-1 bg-blue-600 text-white py-2.5 rounded-lg font-medium hover:bg-blue-700 focus:ring-4 focus:ring-blue-200 disabled:opacity-50 disabled:cursor-not-allowed transition-colors flex items-center justify-center"
          >
            <svg v-if="loading" class="animate-spin rounded-full h-5 w-5 border-2 border-white border-t-transparent mr-2" viewBox="0 0 24 24">
              <circle class="opacity-25" cx="12" cy="12" r="10" stroke="currentColor" stroke-width="4" fill="none" />
              <path class="opacity-75" fill="currentColor" d="M4 12a8 8 0 018-8V0C5.373 0 0 5.373 0 12h4zm2 5.291A7.962 7.962 0 014 12H0c0 3.042 1.135 5.824 3 7.938l3-2.647z" />
            </svg>
            {{ loading ? '修改中...' : '修改密码' }}
          </button>
        </div>
      </form>

      <!-- 安全提示 -->
      <div class="mt-6 p-4 bg-yellow-50 rounded-lg border border-yellow-200">
        <h3 class="text-sm font-medium text-yellow-800 mb-1">安全提示</h3>
        <ul class="text-sm text-yellow-700 space-y-1 list-disc list-inside">
          <li>建议使用包含字母、数字和特殊字符的强密码</li>
          <li>不要使用与其他网站相同的密码</li>
          <li>定期更换密码以提高账户安全性</li>
          <li>修改密码后需要重新登录</li>
        </ul>
      </div>
    </div>
  </div>
</template>
