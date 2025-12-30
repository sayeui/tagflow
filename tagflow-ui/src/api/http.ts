import axios from 'axios'
import { useAuthStore } from '@/stores/auth'

const instance = axios.create({
  baseURL: '/api',
  timeout: 10000,
  headers: {
    'Content-Type': 'application/json',
  },
})

// 请求拦截：自动附加 Token
instance.interceptors.request.use(
  (config) => {
    const authStore = useAuthStore()
    if (authStore.token) {
      config.headers.Authorization = `Bearer ${authStore.token}`
    }
    return config
  },
  (error) => {
    return Promise.reject(error)
  }
)

// 响应拦截：处理 401 错误
instance.interceptors.response.use(
  (response) => response,
  (error) => {
    if (error.response?.status === 401) {
      const authStore = useAuthStore()
      authStore.logout()

      // 使用 Vue Router 导航而不是硬刷新，避免 Toast 被清空
      // 检查当前是否在登录页面，避免不必要的导航
      if (window.location.pathname !== '/login') {
        window.location.href = '/login'
      }
    }
    return Promise.reject(error)
  }
)

export default instance

// API 函数
export const authApi = {
  login: (username: string, password: string) =>
    instance.post('/auth/login', { username, password }),

  updatePassword: (oldPassword: string, newPassword: string) =>
    instance.post('/auth/update-password', {
      old_password: oldPassword,
      new_password: newPassword,
    }),
}

export const tagApi = {
  getTree: () => instance.get('/v1/tags/tree'),
}

export const fileApi = {
  list: (params?: {
    tag_id?: number
    recursive?: boolean
    page?: number
    limit?: number
  }) => instance.get('/v1/files', { params }),
}
