<script setup lang="ts">
import { computed } from 'vue'
import { useRouter } from 'vue-router'
import { useAuthStore } from '@/stores/auth'
import { FolderOpen, LogOut, Shield } from 'lucide-vue-next'

const router = useRouter()
const authStore = useAuthStore()

const isLoggedIn = computed(() => authStore.isLoggedIn)

const handleLogout = () => {
  authStore.logout()
  router.push('/login')
}
</script>

<template>
  <div class="min-h-screen bg-gray-50">
    <!-- 顶部导航栏 -->
    <header v-if="isLoggedIn" class="bg-white border-b border-gray-200 h-14 flex items-center justify-between px-6">
      <div class="flex items-center font-bold text-xl text-blue-600">
        <FolderOpen class="w-6 h-6 mr-2" />
        <span class="cursor-pointer" @click="router.push('/')">TagFlow</span>
      </div>
      <nav class="flex items-center gap-4">
        <span class="text-sm text-gray-600">欢迎, {{ authStore.username }}</span>
        <button
          @click="router.push('/settings/security')"
          class="flex items-center gap-1 px-3 py-1.5 text-sm text-gray-700 hover:bg-gray-100 rounded-lg transition-colors"
        >
          <Shield class="w-4 h-4" />
          安全设置
        </button>
        <button
          @click="handleLogout"
          class="flex items-center gap-1 px-3 py-1.5 text-sm text-red-600 hover:bg-red-50 rounded-lg transition-colors"
        >
          <LogOut class="w-4 h-4" />
          退出登录
        </button>
      </nav>
    </header>

    <!-- 路由视图 -->
    <router-view />
  </div>
</template>
