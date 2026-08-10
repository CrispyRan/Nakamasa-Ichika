import axios from 'axios'
import { Message } from '@arco-design/web-vue'
import tool from '@/utils/tool'
import { get, isEmpty } from 'lodash'
import qs from 'qs'
import { h } from 'vue'
import { IconFaceFrownFill } from '@arco-design/web-vue/dist/arco-vue-icon'
import router from '@/router'
import { mockInterceptor, setMockMode, isMockEnabled, registerMocks } from '@/utils/mock'
import { allMocks, initMockData } from '@/mock'

// ========== Mock 系统初始化 ==========
const env = import.meta.env
const mockMode = (env.VITE_APP_MOCK_MODE || 'off').trim()

if (mockMode !== 'off') {
  // 设置模式
  setMockMode(mockMode)
  
  // 初始化数据
  initMockData()
  
  // 注册所有 Mock
  registerMocks(allMocks)
}

// ========== Axios 服务创建 ==========

function createExternalService() {
  const service = axios.create()
  service.interceptors.request.use(c => c, e => Promise.reject(e))
  service.interceptors.response.use(r => r, e => Promise.reject(e.response ?? null))
  return service
}

function createService() {
  const service = axios.create()

  /**
   * 提取错误信息
   */
  function extractErrorMsg(data) {
    if (!data) return '未知错误'
    if (typeof data === 'string') return data
    return data.message || data.msg || data.error || data.detail || '服务器返回了未知的错误格式'
  }

  // 请求拦截器
  service.interceptors.request.use(
    config => config,
    error => {
      console.error('[Request Error]', error)
      return Promise.reject(error)
    }
  )

  // 响应拦截器
  service.interceptors.response.use(
    async response => {
      const { data, headers, status } = response

      // 文件下载检测
      const isFileDownload = 
        (headers['content-disposition'] || !/^application\/json/.test(headers['content-type'])) && 
        status === 200

      if (isFileDownload) {
        if (data instanceof Blob && data.type?.includes('application/json')) {
          try {
            const text = await data.text()
            const json = JSON.parse(text)
            Message.error({ content: extractErrorMsg(json), icon: () => h(IconFaceFrownFill) })
            return { code: 500, message: extractErrorMsg(json), success: false }
          } catch (e) {}
        }
        return response
      }

      // 业务逻辑处理
      const code = data.code
      if (code === undefined || code === 200) return data

      const errorMsg = extractErrorMsg(data)

      // 401 处理
      if (code === 401) {
        throttle(() => {
          Message.error({ content: errorMsg || '登录状态已过期，请重新登录', icon: () => h(IconFaceFrownFill) })
          tool.local.clear()
          router.push({ name: 'login' })
        })()
      } else {
        Message.error({ content: errorMsg, icon: () => h(IconFaceFrownFill) })
      }

      return data
    },
    error => {
      const response = error.response
      let errorMessage = '请求发生错误'

      if (response?.data) {
        errorMessage = extractErrorMsg(response.data)
        if (response.status === 401) {
          throttle(() => {
            Message.error({ content: errorMessage || '登录状态已过期', icon: () => h(IconFaceFrownFill) })
            tool.local.clear()
            router.push({ name: 'login' })
          })()
        } else {
          Message.error({ content: errorMessage, icon: () => h(IconFaceFrownFill) })
        }
      } else if (error.code === 'ECONNABORTED') {
        errorMessage = '请求超时，服务器未响应'
        Message.error({ content: errorMessage, icon: () => h(IconFaceFrownFill) })
      } else if (!window.navigator.onLine) {
        errorMessage = '网络连接已断开'
        Message.error({ content: errorMessage, icon: () => h(IconFaceFrownFill) })
      } else {
        errorMessage = error.message || '未知错误'
        Message.error({ content: errorMessage, icon: () => h(IconFaceFrownFill) })
      }

      return Promise.resolve({
        code: response?.status || 500,
        message: errorMessage,
        success: false
      })
    }
  )

  return service
}

// ========== 工具函数 ==========

function throttle(fn, wait = 1500) {
  return function() {
    if (!throttle.timer) {
      fn.apply(this, arguments)
      throttle.timer = setTimeout(() => { throttle.timer = null }, wait)
    }
  }
}

function stringify(data) {
  return qs.stringify(data, { allowDots: true, encode: false })
}

function formatToken(token) {
  return token || null
}

// ========== 请求工厂 ==========

function createRequest(service, externalService) {
  return async function(config) {
    const env = import.meta.env
    const token = tool.session.get(env.VITE_APP_TOKEN_PREFIX)
    const setting = tool.local.get('setting')
    
    // 获取 appid：支持多种存储格式
    let appid = ''
    const currentApp = tool.local.get('currentApp')
    const currentAppId = tool.local.get('currentAppId')
    
    // 优先从 currentApp.id 获取
    if (currentApp && typeof currentApp === 'object' && currentApp.id) {
      appid = currentApp.id
    }
    // 兼容：currentApp 可能是双重编码的字符串
    else if (currentApp && typeof currentApp === 'string') {
      try {
        const parsed = JSON.parse(currentApp)
        if (parsed && parsed.id) {
          appid = parsed.id
        }
      } catch (e) {}
    }
    // 从 currentAppId 获取
    if (!appid && currentAppId) {
      appid = currentAppId
    }
    
    // 确保 appid 是字符串
    if (appid !== '') {
      appid = String(appid)
    }

    // 获取 Content-Type，支持显式设置 undefined 来跳过默认值
    const contentType = get(config, 'headers.Content-Type')
    const shouldSetContentType = contentType !== undefined && contentType !== null
    
    const configDefault = {
      headers: {
        Token: formatToken(token),
        'Accept-Language': setting?.language || 'zh_CN',
        appid: appid,
        ...config.headers
      },
      timeout: 10000,
      data: {}
    }
    
    // 只有在需要时才设置默认的 Content-Type
    if (shouldSetContentType) {
      configDefault.headers['Content-Type'] = contentType
    } else if (!config.headers?.hasOwnProperty('Content-Type')) {
      // 如果没有显式设置，使用默认值
      configDefault.headers['Content-Type'] = 'application/json;charset=UTF-8'
    }

    delete config.headers
    const option = Object.assign(configDefault, config)

    // 处理 params
    if (!isEmpty(option.params)) {
      option.url = option.url + '?' + stringify(option.params)
      option.params = {}
    }

    // Mock 拦截
    if (isMockEnabled()) {
      const mockResult = await mockInterceptor(option)
      if (mockResult !== false) {
        return mockResult
      }
    }

    // 实际请求
    if (!/^(http|https)/g.test(option.url)) {
      option.baseURL = env.VITE_APP_OPEN_PROXY === 'true' ? env.VITE_APP_PROXY_PREFIX : env.VITE_APP_BASE_URL
      return service(option)
    } else {
      return externalService(option)
    }
  }
}

// ========== 导出 ==========

export const service = createService()
export const externalService = createExternalService()
export const request = createRequest(service, externalService)
