<template>
  <div class="app-info-page">
    <!-- 未选择应用提示 -->
    <a-card v-if="!hasAppId" class="mb-4" :bordered="false">
      <a-result status="warning" title="请先选择应用">
        <template #extra>
          <a-button type="primary" @click="router.push('/apps')">
            前往应用列表
          </a-button>
        </template>
      </a-result>
    </a-card>
    
    <!-- 应用基本信息 -->
    <a-card v-else class="mb-4" :bordered="false">
      <p class="font-semibold text-lg mb-4">APPID：{{ appInfo.id }}</p>
      <a-divider />
      <a-form :model="appInfo" layout="vertical" @submit="handleSubmit">
        <a-row :gutter="16">
          <a-col :span="12">
            <a-form-item label="应用名称">
              <a-input v-model="appInfo.app_name" placeholder="请设置应用名称" />
            </a-form-item>
          </a-col>
          <a-col :span="12">
            <a-form-item label="APPKEY">
              <a-input-group>
                <a-input v-model="appInfo.app_key" readonly placeholder="请设置APPKEY" />
                <a-button @click="generateKey">更换</a-button>
                <a-button type="primary" @click="copyKey">复制</a-button>
              </a-input-group>
            </a-form-item>
          </a-col>
        </a-row>
        
        <a-row :gutter="16">
          <a-col :span="12">
            <a-form-item label="运营模式">
              <a-radio-group v-model="appInfo.app_mode">
                <a-radio value="y">收费</a-radio>
                <a-radio value="n">免费</a-radio>
              </a-radio-group>
            </a-form-item>
          </a-col>
          <a-col :span="12">
            <a-form-item label="应用状态">
              <a-radio-group v-model="appInfo.app_state">
                <a-radio value="on">正常</a-radio>
                <a-radio value="off">关闭</a-radio>
              </a-radio-group>
            </a-form-item>
          </a-col>
        </a-row>

        <a-form-item v-if="appInfo.app_state === 'off'" label="关闭消息">
          <a-textarea v-model="appInfo.app_off_msg" placeholder="如：系统维护中" :auto-size="{ minRows: 2, maxRows: 4 }" />
        </a-form-item>

        <a-form-item label="应用图标">
          <a-upload
            :custom-request="customUpload"
            list-type="picture-card"
            :file-list="fileList"
            :limit="1"
            @change="handleFileChange"
          >
            <template #upload-button>
              <div class="upload-trigger">
                <icon-plus />
                <div class="text-sm mt-1">上传图标</div>
              </div>
            </template>
          </a-upload>
        </a-form-item>

        <a-form-item>
          <a-button type="primary" html-type="submit" :loading="loading">
            提交
          </a-button>
        </a-form-item>
      </a-form>
    </a-card>
    
    <!-- AI 配置 -->
    <a-card v-if="hasAppId" class="mb-4" :bordered="false" title="AI 配置">
      <a-form :model="appInfo" layout="vertical">
        <a-row :gutter="16">
          <a-col :span="12">
            <a-form-item label="AI 功能状态">
              <a-radio-group v-model="appInfo.ai_state">
                <a-radio value="on">开启</a-radio>
                <a-radio value="off">关闭</a-radio>
              </a-radio-group>
            </a-form-item>
          </a-col>
          <a-col :span="12">
            <a-form-item label="AI 提供商">
              <a-select v-model="appInfo.ai_provider" placeholder="请选择AI提供商">
                <a-option value="openai">OpenAI</a-option>
                <a-option value="claude">Claude</a-option>
                <a-option value="gemini">Gemini</a-option>
                <a-option value="vllm">vLLM</a-option>
                <a-option value="sglang">SGLang</a-option>
                <a-option value="ollama">Ollama</a-option>
                <a-option value="lm_studio">LM Studio</a-option>
                <a-option value="llama_cpp">Llama.cpp</a-option>
                <a-option value="mistral_rust">Mistral Rust</a-option>
              </a-select>
            </a-form-item>
          </a-col>
        </a-row>

        <a-row :gutter="16">
          <a-col :span="12">
            <a-form-item label="API 地址">
              <a-input v-model="appInfo.ai_api_base" placeholder="如：https://api.openai.com/v1" />
            </a-form-item>
          </a-col>
          <a-col :span="12">
            <a-form-item label="API 密钥">
              <a-input-password v-model="appInfo.ai_api_key" placeholder="留空则使用默认值" />
            </a-form-item>
          </a-col>
        </a-row>

        <!-- 模型选择：支持手动输入或自动获取 -->
        <a-row :gutter="16">
          <a-col :span="8">
            <a-form-item label="模型选择方式">
              <a-switch v-model="autoFetchModels" checked-value="auto" unchecked-value="manual">
                <template #checked>自动获取</template>
                <template #unchecked>手动输入</template>
              </a-switch>
            </a-form-item>
          </a-col>
          <a-col :span="8" v-if="autoFetchModels === 'auto'">
            <a-form-item label="模型列表">
              <a-select 
                v-model="appInfo.ai_model" 
                :placeholder="modelOptions.length > 0 ? '请选择模型' : '请先获取模型列表'"
                :loading="loadingModels"
                allow-search
                allow-clear
              >
                <a-option v-for="m in modelOptions" :key="m.id" :value="m.id">
                  {{ m.id }}
                </a-option>
              </a-select>
              <div v-if="modelOptions.length > 0" style="margin-top: 4px; color: var(--color-text-3); font-size: 12px;">
                共 {{ modelOptions.length }} 个模型
              </div>
              <div v-else-if="!loadingModels && appInfo.ai_api_base" style="margin-top: 4px; color: var(--color-text-3); font-size: 12px;">
                暂无模型数据，请点击刷新
              </div>
            </a-form-item>
          </a-col>
          <a-col :span="8" v-else>
            <a-form-item label="模型名称">
              <a-input v-model="appInfo.ai_model" placeholder="如：gpt-3.5-turbo" />
            </a-form-item>
          </a-col>
          <a-col :span="8" v-if="autoFetchModels === 'auto'">
            <a-form-item label="操作">
              <a-button 
                type="outline" 
                :loading="loadingModels" 
                @click="fetchModels"
                :disabled="!appInfo.ai_api_base"
              >
                <template #icon><icon-refresh /></template>
                手动刷新
              </a-button>
              <a-button 
                v-if="modelOptions.length > 0"
                type="text" 
                @click="clearModels"
                style="margin-left: 8px;"
              >
                清空列表
              </a-button>
            </a-form-item>
          </a-col>
        </a-row>

        <a-row :gutter="16">
          <a-col :span="8">
            <a-form-item label="温度参数">
              <a-input-number 
                v-model="appInfo.ai_temperature" 
                :min="0" 
                :max="2" 
                :step="0.1" 
                placeholder="0.7" 
                style="width: 100%" 
              />
            </a-form-item>
          </a-col>
          <a-col :span="8">
            <a-form-item label="最大Token数">
              <a-input-number 
                v-model="appInfo.ai_max_tokens" 
                :min="1" 
                :max="32000" 
                placeholder="4096" 
                style="width: 100%" 
              />
            </a-form-item>
          </a-col>
        </a-row>

        <a-form-item>
          <a-button type="primary" html-type="submit" :loading="loading" @click="handleSubmit">
            保存 AI 配置
          </a-button>
        </a-form-item>
      </a-form>
    </a-card>
  </div>
</template>

<script setup>
import { ref, reactive, onMounted, watch } from 'vue'
import { Message } from '@arco-design/web-vue'
import { useRouter } from 'vue-router'
import appApi from '@/api/system/app'
import uploadApi from '@/api/system/upload'
import tool from '@/utils/tool'

const router = useRouter()
const loading = ref(false)
const loadingModels = ref(false)
const fileList = ref([])
const hasAppId = ref(true)
const autoFetchModels = ref('manual') // 'manual' 或 'auto'
const modelOptions = ref([]) // 存储获取的模型列表
const searchQuery = ref('') // 搜索过滤词

const appInfo = reactive({
  id: '',
  app_name: '',
  app_key: '',
  app_logo: '',
  app_mode: 'y',
  app_state: 'on',
  app_off_msg: '',
  // AI 配置
  ai_state: 'off',
  ai_provider: '',
  ai_api_base: '',
  ai_api_key: '',
  ai_model: '',
  ai_temperature: null,
  ai_max_tokens: null
})

// 加载应用信息
const loadAppInfo = async () => {
  // 检查 currentAppId 是否存在
  const currentAppId = tool.local.get('currentAppId')
  const currentApp = tool.local.get('currentApp')
  
  if (!currentAppId && !currentApp?.id) {
    hasAppId.value = false
    Message.error('请先选择应用')
    setTimeout(() => {
      router.push('/apps')
    }, 1500)
    return
  }
  
  try {
    const res = await appApi.getInfo(['id', 'app_name', 'app_key', 'app_logo', 'app_mode', 'app_state', 'app_off_msg',
      'ai_state', 'ai_provider', 'ai_api_base', 'ai_api_key', 'ai_model', 'ai_temperature', 'ai_max_tokens'])
    if (res.code === 200) {
      Object.assign(appInfo, res.data)
      if (appInfo.app_logo) {
        fileList.value = [{ url: tool.attachUrl(appInfo.app_logo), name: 'logo' }]
      }
      // 如果已有 API 地址且是自动模式，则自动获取模型列表
      if (appInfo.ai_api_base && autoFetchModels.value === 'auto') {
        fetchModels()
      }
    } else {
      Message.error(res.msg)
    }
  } catch (e) {
    Message.error('加载应用信息失败：' + e)
  }
}

// 生成随机KEY
const generateKey = () => {
  const chars = 'ABCDEFGHJKLMNPQRSTUVWXYZabcdefghjkmnpqrstuvwxyz23456789'
  let key = ''
  for (let i = 0; i < 32; i++) {
    key += chars.charAt(Math.floor(Math.random() * chars.length))
  }
  appInfo.app_key = key
  Message.success('已生成新的APPKEY')
}

// 复制KEY
const copyKey = async () => {
  try {
    await navigator.clipboard.writeText(appInfo.app_key)
    Message.success('复制成功')
  } catch (e) {
    Message.error('复制失败，请手动复制')
  }
}

// 自定义上传请求（参考旧版实现）
const customUpload = async (options) => {
  const { fileItem, onSuccess, onError, onProgress } = options
  
  // 创建 FormData
  const formData = new FormData()
  formData.append('file', fileItem.file)
  
  // 进度处理函数：将 ProgressEvent 转换为 percent (0-100)
  const handleProgress = (progressEvent) => {
    if (progressEvent.total && onProgress) {
      const percent = Math.round((progressEvent.loaded / progressEvent.total) * 100)
      onProgress(percent)
    }
  }
  
  try {
    const res = await uploadApi.img(formData, handleProgress)
    if (res.code === 200) {
      appInfo.app_logo = res.data.url
      Message.success(res.msg || '上传成功')
      onSuccess(res)
    } else {
      Message.error(res.msg || '上传失败')
      onError(res.msg)
    }
  } catch (e) {
    Message.error('上传失败：' + e)
    onError(e)
  }
}

// 文件列表变化处理
const handleFileChange = (fileListData) => {
  // 如果用户删除了文件，清空 app_logo
  if (!fileListData || fileListData.length === 0) {
    appInfo.app_logo = ''
  }
}

// 提取 API Base URL（智能处理各种输入格式，支持任意位置的版本路径如 /api/v1）
const extractBaseUrl = (url) => {
  if (!url) return ''
  
  try {
    let processedUrl = url.trim()
    
    // 如果没有协议前缀，自动添加 https://
    if (!processedUrl.startsWith('http://') && !processedUrl.startsWith('https://')) {
      processedUrl = 'https://' + processedUrl
    }
    
    const parsed = new URL(processedUrl)
    const pathParts = parsed.pathname.split('/').filter(Boolean)
    
    // 查找路径中的版本段（v1、v2、v3 等）
    const versionIndex = pathParts.findIndex(segment => /^v\d+$/.test(segment))
    
    if (versionIndex !== -1) {
      // 保留 origin + 从开始到版本段的所有路径
      const basePath = pathParts.slice(0, versionIndex + 1).join('/')
      return `${parsed.origin}/${basePath}`
    }
    
    // 没有找到版本段，返回 origin
    return parsed.origin
  } catch (e) {
    // URL 解析失败，返回原值（让后续 fetch 报错）
    console.warn('URL 解析失败:', e.message)
    return url
  }
}

// 从 API 地址获取模型列表（前端独立完成）
const fetchModels = async () => {
  if (!appInfo.ai_api_base) {
    Message.warning('请先填写 API 地址')
    return
  }
  
  loadingModels.value = true
  try {
    // 智能提取 base URL（自动补协议、保留版本路径）
    const baseUrl = extractBaseUrl(appInfo.ai_api_base)
    
    // 定义特殊提供商的模型列表地址
    const specialProviders = {
      'openrouter.ai': 'https://openrouter.ai/api/v1/models',
      // 可以继续添加其他特殊提供商
    }
    
    // 构造可能的 models 端点列表（按优先级尝试）
    const modelUrls = []
    
    // 检查是否为特殊提供商
    const specialUrl = Object.entries(specialProviders).find(([domain]) => 
      baseUrl.includes(domain)
    )?.[1]
    
    if (specialUrl) {
      // 特殊提供商，直接使用定义的地址
      modelUrls.push(specialUrl)
    } else {
      // 通用逻辑：尝试多个可能的 models 端点
      // 1. 标准 OpenAI 兼容格式：baseUrl + /models
      modelUrls.push(`${baseUrl}/models`)
      
      // 2. 如果 baseUrl 不包含版本路径，也尝试加 /v1/models
      if (!/\/v\d+/.test(baseUrl)) {
        modelUrls.push(`${baseUrl}/v1/models`)
      }
    }
    
    console.log('尝试获取模型列表:', modelUrls)
    
    // 构造请求头
    const headers = {
      'Content-Type': 'application/json'
    }
    
    // 如果有 API key，添加 Authorization 头
    if (appInfo.ai_api_key) {
      headers['Authorization'] = `Bearer ${appInfo.ai_api_key}`
    }
    
    // 尝试所有可能的模型列表地址
    let lastError = null
    let data = null
    
    for (const url of modelUrls) {
      try {
        console.log('正在尝试:', url)
        const response = await fetch(url, { headers })
        
        if (!response.ok) {
          lastError = `请求失败: ${response.status} ${response.statusText}`
          continue // 尝试下一个地址
        }
        
        data = await response.json()
        break // 成功获取，跳出循环
      } catch (e) {
        lastError = e.message
        continue // 尝试下一个地址
      }
    }
    
    if (!data) {
      throw new Error(lastError || '所有尝试的地址均失败')
    }
    
    // 解析模型列表（兼容多种返回格式）
    if (data.object === 'list' && Array.isArray(data.data)) {
      // OpenAI 格式: { object: 'list', data: [{ id: 'model-name', ... }, ...] }
      modelOptions.value = data.data.filter(m => m.id && typeof m.id === 'string')
    } else if (Array.isArray(data)) {
      // 有些 API 直接返回数组
      modelOptions.value = data.filter(m => m.id && typeof m.id === 'string')
    } else if (data.data && Array.isArray(data.data)) {
      // 其他包装格式
      modelOptions.value = data.data.filter(m => m.id && typeof m.id === 'string')
    } else {
      throw new Error('无法识别的模型列表格式')
    }
    
    if (modelOptions.value.length === 0) {
      Message.warning('未找到可用模型，请检查 API 地址和密钥')
    } else {
      Message.success(`成功获取 ${modelOptions.value.length} 个模型`)
      // 如果当前选择的模型不在列表中，清空
      if (appInfo.ai_model && !modelOptions.value.some(m => m.id === appInfo.ai_model)) {
        appInfo.ai_model = ''
      }
    }
  } catch (e) {
    console.error('获取模型列表失败:', e)
    
    // 友好错误提示
    if (e.message.includes('404') || e.message.includes('Not Found')) {
      Message.error('该提供商可能不支持通过 API 获取模型列表，请切换到手动模式输入模型名称')
      // 自动切换到手动模式
      autoFetchModels.value = 'manual'
    } else {
      Message.error('获取模型列表失败: ' + e.message)
    }
    
    modelOptions.value = []
  } finally {
    loadingModels.value = false
  }
}
const clearModels = () => {
  modelOptions.value = []
  appInfo.ai_model = ''
  Message.info('已清空模型列表')
}

// 提交
const handleSubmit = async () => {
  loading.value = true
  try {
    const res = await appApi.edit({ id: appInfo.id, ...appInfo })
    if (res.code === 200) {
      Message.success(res.msg || '保存成功')
    } else {
      Message.error(res.msg)
    }
  } catch (e) {
    Message.error('保存失败：' + e)
  } finally {
    loading.value = false
  }
}

// 监听 API 地址变化，自动获取模型（如果开启了自动获取）
watch(() => appInfo.ai_api_base, (newVal) => {
  // 只在自动模式下自动刷新
  if (autoFetchModels.value === 'auto' && newVal) {
    fetchModels()
  }
})

// 监听自动获取开关，切换到自动模式时如果有 API 地址就自动获取
watch(autoFetchModels, (newVal) => {
  if (newVal === 'auto' && appInfo.ai_api_base) {
    fetchModels()
  }
})

onMounted(() => {
  loadAppInfo()
})
</script>

<style scoped>
.app-info-page {
  padding: 16px;
}

.upload-trigger {
  width: 100px;
  height: 100px;
  display: flex;
  flex-direction: column;
  align-items: center;
  justify-content: center;
  color: var(--color-text-3);
}
</style>
