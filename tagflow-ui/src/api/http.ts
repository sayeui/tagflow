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

/** 构造带 JWT token 的媒体 URL（供 <img>/<video>/<iframe src> 用，绕过 Authorization 头限制）。
 *  后端 auth_middleware 接受 ?token=<jwt> 兜底；token 从 auth store 取。 */
export function mediaUrl(
  path: string,
  params?: Record<string, string | number | boolean>,
): string {
  const token = useAuthStore().token
  const qs = new URLSearchParams()
  if (token) qs.set('token', token)
  if (params) for (const [k, v] of Object.entries(params)) qs.set(k, String(v))
  const query = qs.toString()
  return query ? `${path}?${query}` : path
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

  /** 文件详情（元数据 + 标签） */
  detail: (id: number) => instance.get(`/v1/files/${id}`),

  /** 文本内容（axios 走 Authorization 头，无需 token query） */
  contentText: (id: number) =>
    instance
      .get(`/v1/files/${id}/content`, {
        responseType: 'text',
        transformResponse: [(v) => v],
      })
      .then((res) => res.data as string),

  /** 媒体内容 URL（含 token），供 <img>/<video>/<iframe src> 与下载使用 */
  contentUrl: (id: number, opts?: { download?: boolean }) =>
    mediaUrl(`/api/v1/files/${id}/content`, opts?.download ? { download: '1' } : undefined),

  /** 缩略图 URL（含 token，修复历史 401） */
  thumbnailUrl: (id: number) => mediaUrl(`/api/v1/files/${id}/thumbnail`),
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
