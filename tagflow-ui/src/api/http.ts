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
    tag_ids?: number[]
    recursive?: boolean
    page?: number
    limit?: number
  }) => {
    // axum serde_urlencoded 不支持重复 key 成 Vec，改用逗号分隔（tag_ids=3,7）。
    const { tag_ids, ...rest } = params ?? {}
    return instance.get('/v1/files', {
      params: {
        ...rest,
        ...(tag_ids && tag_ids.length ? { tag_ids: tag_ids.join(',') } : {}),
      },
    })
  },
}

export const libraryApi = {
  // 获取所有资源库
  list: () => instance.get('/v1/libraries'),

  // 创建资源库
  create: (data: {
    name: string
    protocol: string
    base_path: string
    config_json?: string
  }) => instance.post('/v1/libraries', data),

  // 删除资源库
  delete: (id: number) => instance.delete(`/v1/libraries/${id}`),

  // 测试连接
  testConnection: (data: {
    name: string
    protocol: string
    base_path: string
    config_json?: string
  }) => instance.post('/v1/libraries/test', data),

  // 触发扫描
  triggerScan: (id: number) => instance.post(`/v1/libraries/${id}/scan`),
}
