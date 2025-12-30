import { createRouter, createWebHistory, RouteRecordRaw } from 'vue-router'
import { useAuthStore } from '@/stores/auth'

// 导入页面组件
const Login = () => import('@/views/Login.vue')
const Home = () => import('@/views/Home.vue')
const SecuritySettings = () => import('@/views/settings/Security.vue')
const LibrariesSettings = () => import('@/views/settings/Libraries.vue')

const routes: RouteRecordRaw[] = [
  {
    path: '/login',
    name: 'Login',
    component: Login,
  },
  {
    path: '/',
    name: 'Home',
    component: Home,
  },
  {
    path: '/settings/security',
    name: 'SecuritySettings',
    component: SecuritySettings,
  },
  {
    path: '/settings/libraries',
    name: 'LibrariesSettings',
    component: LibrariesSettings,
  },
]

const router = createRouter({
  history: createWebHistory(),
  routes,
})

// 路由守卫
router.beforeEach((to, _from, next) => {
  const authStore = useAuthStore()

  if (to.name !== 'Login' && !authStore.isLoggedIn) {
    // 未登录且不是前往登录页，重定向到登录页
    next({ name: 'Login' })
  } else if (to.name === 'Login' && authStore.isLoggedIn) {
    // 已登录且前往登录页，重定向到首页
    next({ name: 'Home' })
  } else {
    next()
  }
})

export default router
