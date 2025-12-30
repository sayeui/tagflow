<script setup lang="ts">
import { ref, onMounted } from 'vue'
import { useRouter } from 'vue-router'
import { libraryApi } from '@/api/http'
import { Trash2, CheckCircle, XCircle, Plus, RefreshCw, AlertCircle, ArrowLeft } from 'lucide-vue-next'
import Toast from '@/components/Toast.vue'

const router = useRouter()

interface Library {
  id: number
  name: string
  protocol: string
  base_path: string
  last_scanned_at: string | null
}

const libraries = ref<Library[]>([])
const newLib = ref({
  name: '',
  protocol: 'local',
  base_path: '',
  config_json: ''
})
const testResult = ref<{ reachable: boolean; message: string } | null>(null)
const isTesting = ref(false)
const isCreating = ref(false)

// Toast 提示状态
const toastMessage = ref('')
const toastType = ref<'error' | 'success' | 'warning' | 'info'>('info')

// 表单字段错误提示
const formErrors = ref({
  name: '',
  base_path: ''
})

const showToast = (message: string, type: 'error' | 'success' | 'warning' | 'info' = 'info') => {
  toastMessage.value = message
  toastType.value = type
}

const fetchLibraries = async () => {
  try {
    const response = await libraryApi.list()
    libraries.value = response.data
  } catch (error: any) {
    showToast('加载资源库列表失败', 'error')
  }
}

// 清除字段错误
const clearFieldError = (field: 'name' | 'base_path') => {
  formErrors.value[field] = ''
}

// 验证表单
const validateForm = (): boolean => {
  let isValid = true

  // 验证名称
  if (!newLib.value.name.trim()) {
    formErrors.value.name = '请输入资源库名称'
    isValid = false
  } else if (newLib.value.name.trim().length < 2) {
    formErrors.value.name = '资源库名称至少需要2个字符'
    isValid = false
  } else {
    formErrors.value.name = ''
  }

  // 验证路径
  const pathValidation = validatePath(newLib.value.base_path.trim())
  if (!pathValidation.valid) {
    formErrors.value.base_path = pathValidation.error || '路径验证失败'
    isValid = false
  } else {
    formErrors.value.base_path = ''
  }

  return isValid
}

// 路径安全验证（防止路径遍历攻击）
const validatePath = (path: string): { valid: boolean; error?: string } => {
  if (!path) {
    return { valid: false, error: '请输入物理路径' }
  }

  // 检测路径遍历攻击
  if (path.includes('..')) {
    return { valid: false, error: '路径不能包含 ".."（路径遍历检测）' }
  }

  if (path.includes('./') || path.includes('.\\')) {
    return { valid: false, error: '路径不能包含 "./" 或 ".\\"' }
  }

  // 检查是否为绝对路径
  const isUnixPath = path.startsWith('/')
  const isWindowsPath = /^[a-zA-Z]:\\/.test(path)

  if (!isUnixPath && !isWindowsPath) {
    return { valid: false, error: '必须使用绝对路径（如 /mnt/data 或 C:\\Data）' }
  }

  return { valid: true }
}

const testConnection = async () => {
  // 先验证路径字段
  const pathValidation = validatePath(newLib.value.base_path.trim())
  if (!pathValidation.valid) {
    formErrors.value.base_path = pathValidation.error || '路径验证失败'
    return
  }
  clearFieldError('base_path')

  isTesting.value = true
  testResult.value = null

  try {
    const response = await libraryApi.testConnection({
      name: newLib.value.name || '测试',
      protocol: newLib.value.protocol,
      base_path: newLib.value.base_path,
    })
    testResult.value = response.data

    if (response.data.reachable) {
      showToast('连接测试成功', 'success')
    } else {
      showToast(`连接测试失败: ${response.data.message}`, 'error')
    }
  } catch (error: any) {
    showToast('连接测试失败', 'error')
  } finally {
    isTesting.value = false
  }
}

const addLibrary = async () => {
  // 执行表单验证
  if (!validateForm()) {
    return
  }

  isCreating.value = true

  try {
    await libraryApi.create({
      name: newLib.value.name,
      protocol: newLib.value.protocol,
      base_path: newLib.value.base_path,
      config_json: newLib.value.config_json || undefined,
    })

    // 重置表单
    newLib.value = {
      name: '',
      protocol: 'local',
      base_path: '',
      config_json: ''
    }
    testResult.value = null
    // 重置错误提示
    formErrors.value = { name: '', base_path: '' }

    showToast('资源库添加成功', 'success')
    fetchLibraries()
  } catch (error: any) {
    const errorMsg = error.response?.data?.error || '添加资源库失败'
    showToast(errorMsg, 'error')
  } finally {
    isCreating.value = false
  }
}

const removeLibrary = async (id: number, name: string) => {
  if (!confirm(`确定要删除资源库 "${name}" 吗？`)) {
    return
  }

  try {
    await libraryApi.delete(id)
    showToast('资源库已删除', 'success')
    fetchLibraries()
  } catch (error: any) {
    showToast('删除资源库失败', 'error')
  }
}

const triggerScan = async (id: number, name: string) => {
  try {
    await libraryApi.triggerScan(id)
    showToast(`已启动资源库 "${name}" 的扫描`, 'success')
  } catch (error: any) {
    if (error.response?.status === 501) {
      showToast('扫描功能尚未实现', 'warning')
    } else {
      showToast('启动扫描失败', 'error')
    }
  }
}

const formatDate = (dateStr: string | null) => {
  if (!dateStr) return '从未扫描'
  return new Date(dateStr).toLocaleString('zh-CN')
}

const goBack = () => {
  router.push('/')
}

onMounted(fetchLibraries)
</script>

<template>
  <!-- Toast 提示 -->
  <Toast
    v-if="toastMessage"
    :message="toastMessage"
    :type="toastType"
    :duration="3000"
    @close="toastMessage = ''"
  />

  <div class="p-8 max-w-4xl mx-auto">
    <!-- 页面头部 -->
    <div class="flex items-center gap-4 mb-6">
      <button
        @click="goBack"
        class="p-2 text-gray-600 hover:bg-gray-100 rounded-lg transition-colors"
        title="返回主页"
      >
        <ArrowLeft class="w-6 h-6" />
      </button>
      <h1 class="text-2xl font-bold text-gray-900">存储库管理</h1>
    </div>

    <!-- 添加资源库表单 -->
    <div class="bg-white p-6 rounded-lg shadow-sm border mb-8">
      <h2 class="text-lg font-semibold mb-4 text-gray-800">添加新资源库</h2>

      <div class="grid grid-cols-2 gap-4 mb-4">
        <div>
          <label class="block text-sm font-medium text-gray-700 mb-2">资源库名称</label>
          <input
            v-model="newLib.name"
            @input="clearFieldError('name')"
            type="text"
            placeholder="例如: 我的照片"
            :class="[
              'w-full px-3 py-2 border rounded-lg focus:ring-2 focus:border-blue-500 outline-none transition-colors',
              formErrors.name
                ? 'border-red-300 focus:ring-red-500 focus:border-red-500'
                : 'border-gray-300 focus:ring-blue-500 focus:border-blue-500'
            ]"
          />
          <!-- 字段错误提示 -->
          <p v-if="formErrors.name" class="mt-1.5 text-sm text-red-600 flex items-center gap-1">
            <AlertCircle class="w-4 h-4" />
            {{ formErrors.name }}
          </p>
        </div>

        <div>
          <label class="block text-sm font-medium text-gray-700 mb-2">协议类型</label>
          <select
            v-model="newLib.protocol"
            class="w-full px-3 py-2 border border-gray-300 rounded-lg focus:ring-2 focus:ring-blue-500 focus:border-blue-500 outline-none"
          >
            <option value="local">本地目录</option>
            <option value="webdav" disabled>WebDAV (暂不支持)</option>
          </select>
        </div>

        <div class="col-span-2">
          <label class="block text-sm font-medium text-gray-700 mb-2">物理路径</label>
          <input
            v-model="newLib.base_path"
            @input="clearFieldError('base_path')"
            type="text"
            placeholder="/mnt/photos 或 C:\Photos"
            :class="[
              'w-full px-3 py-2 border rounded-lg focus:ring-2 focus:border-blue-500 outline-none font-mono text-sm transition-colors',
              formErrors.base_path
                ? 'border-red-300 focus:ring-red-500 focus:border-red-500'
                : 'border-gray-300 focus:ring-blue-500 focus:border-blue-500'
            ]"
          />
          <!-- 字段错误提示 -->
          <p v-if="formErrors.base_path" class="mt-1.5 text-sm text-red-600 flex items-center gap-1">
            <AlertCircle class="w-4 h-4" />
            {{ formErrors.base_path }}
          </p>
        </div>
      </div>

      <div class="flex items-center gap-4">
        <button
          @click="testConnection"
          :disabled="isTesting"
          class="flex items-center gap-2 px-4 py-2 bg-gray-100 text-gray-700 rounded-lg hover:bg-gray-200 disabled:opacity-50 disabled:cursor-not-allowed transition-colors"
        >
          <RefreshCw :class="{ 'animate-spin': isTesting }" class="w-4 h-4" />
          {{ isTesting ? '测试中...' : '测试连接' }}
        </button>

        <button
          @click="addLibrary"
          :disabled="isCreating || !testResult?.reachable"
          class="flex items-center gap-2 px-4 py-2 bg-blue-600 text-white rounded-lg hover:bg-blue-700 disabled:opacity-50 disabled:cursor-not-allowed transition-colors"
        >
          <Plus class="w-4 h-4" />
          {{ isCreating ? '添加中...' : '保存资源库' }}
        </button>

        <!-- 测试结果提示 -->
        <div v-if="testResult" class="flex items-center gap-2 text-sm">
          <span v-if="testResult.reachable" class="flex items-center gap-1 text-green-600">
            <CheckCircle class="w-4 h-4" />
            {{ testResult.message }}
          </span>
          <span v-else class="flex items-center gap-1 text-red-600">
            <XCircle class="w-4 h-4" />
            {{ testResult.message }}
          </span>
        </div>
      </div>
    </div>

    <!-- 资源库列表 -->
    <div class="bg-white rounded-lg shadow-sm border">
      <div class="p-4 border-b">
        <h2 class="text-lg font-semibold text-gray-800">已配置的资源库</h2>
      </div>

      <div v-if="libraries.length === 0" class="p-8 text-center text-gray-500">
        暂无资源库，请添加您的第一个资源库
      </div>

      <div v-else class="divide-y">
        <div
          v-for="lib in libraries"
          :key="lib.id"
          class="p-4 flex items-center justify-between hover:bg-gray-50 transition-colors"
        >
          <div class="flex-1">
            <div class="flex items-center gap-3">
              <span class="font-bold text-gray-900">{{ lib.name }}</span>
              <span class="px-2 py-0.5 text-xs rounded-full bg-blue-100 text-blue-700">
                {{ lib.protocol === 'local' ? '本地' : lib.protocol }}
              </span>
            </div>
            <div class="mt-1 text-sm text-gray-500 font-mono">
              {{ lib.base_path }}
            </div>
            <div class="mt-1 text-xs text-gray-400">
              最后扫描: {{ formatDate(lib.last_scanned_at) }}
            </div>
          </div>

          <div class="flex items-center gap-2">
            <button
              @click="triggerScan(lib.id, lib.name)"
              class="p-2 text-gray-600 hover:bg-gray-100 rounded-lg transition-colors"
              title="触发扫描"
            >
              <RefreshCw class="w-5 h-5" />
            </button>
            <button
              @click="removeLibrary(lib.id, lib.name)"
              class="p-2 text-red-500 hover:bg-red-50 rounded-lg transition-colors"
              title="删除资源库"
            >
              <Trash2 class="w-5 h-5" />
            </button>
          </div>
        </div>
      </div>
    </div>

    <!-- 提示信息 -->
    <div class="mt-6 p-4 bg-blue-50 rounded-lg border border-blue-200">
      <h3 class="font-medium text-blue-900 mb-2">使用提示</h3>
      <ul class="text-sm text-blue-800 space-y-1">
        <li>• 添加路径前，请先点击"测试连接"确保路径可访问</li>
        <li>• 删除资源库仅删除配置信息，不会删除实际文件</li>
        <li>• 扫描功能将自动创建标签层级结构</li>
      </ul>
    </div>
  </div>
</template>
