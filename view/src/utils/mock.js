/**
 * Mock 核心系统
 * 
 * 功能特性：
 * - 三种模式: off(关闭) / demo(只读演示) / develop(完整开发)
 * - 数据持久化到 localStorage
 * - 支持增删改查操作
 * - 可配置延迟模拟
 * - 支持错误率模拟
 * - 支持网络状态模拟
 * - 支持自定义数据生成器
 */

// ========== 类型定义 ==========
/**
 * @typedef {Object} MockConfig
 * @property {string} url - 匹配的 URL（支持正则）
 * @property {string} [method='*'] - HTTP 方法 (* 表示任意)
 * @property {Function} handler - 处理函数 ({ url, method, data, params, headers }) => result
 * @property {Object} [options] - 额外选项
 * @property {number} [options.delay] - 自定义延迟(ms)
 * @property {number} [options.errorRate] - 错误率 (0-1)
 * @property {boolean} [options.persist] - 是否持久化数据
 * @property {string} [options.persistKey] - 持久化键名
 */

// ========== 常量 ==========
const STORAGE_PREFIX = '__mock_data__'
const MODE_OFF = 'off'
const MODE_DEMO = 'demo'
const MODE_DEVELOP = 'develop'

// ========== 全局状态 ==========
let currentMode = MODE_OFF
let globalConfig = {
  delay: { min: 100, max: 500 },  // 默认延迟范围
  errorRate: 0,                    // 全局错误率
  logEnabled: true,                // 是否启用日志
  persistEnabled: true             // 是否启用持久化
}

// Mock 注册表
const mockRegistry = new Map()

// 数据存储（内存）
const dataStore = new Map()

// 事件监听器
const eventListeners = {
  beforeRequest: [],
  afterRequest: [],
  onError: []
}

// ========== 工具函数 ==========
function log(type, ...args) {
  if (!globalConfig.logEnabled) return
  const prefix = `[Mock:${type}]`
  console.log(prefix, ...args)
}

export function generateId() {
  return Date.now().toString(36) + Math.random().toString(36).substr(2, 9)
}

function randomDelay(min, max) {
  return Math.floor(min + Math.random() * (max - min))
}

function shouldTriggerError(errorRate) {
  return Math.random() < errorRate
}

export function deepClone(obj) {
  return JSON.parse(JSON.stringify(obj))
}

// ========== 持久化存储 ==========
export const persist = {
  save(key, data) {
    if (!globalConfig.persistEnabled) return
    try {
      localStorage.setItem(STORAGE_PREFIX + key, JSON.stringify(data))
    } catch (e) {
      log('error', '持久化保存失败:', e.message)
    }
  },

  load(key) {
    if (!globalConfig.persistEnabled) return null
    try {
      const data = localStorage.getItem(STORAGE_PREFIX + key)
      return data ? JSON.parse(data) : null
    } catch (e) {
      return null
    }
  },

  remove(key) {
    localStorage.removeItem(STORAGE_PREFIX + key)
  },

  clear() {
    Object.keys(localStorage)
      .filter(k => k.startsWith(STORAGE_PREFIX))
      .forEach(k => localStorage.removeItem(k))
  }
}

// ========== 数据存储管理 ==========
export const store = {
  /**
   * 获取数据集合
   */
  getCollection(key) {
    if (!dataStore.has(key)) {
      // 尝试从持久化加载
      const saved = persist.load(key)
      dataStore.set(key, saved || [])
    }
    return dataStore.get(key)
  },

  /**
   * 保存数据集合
   */
  saveCollection(key, data) {
    dataStore.set(key, data)
    persist.save(key, data)
  },

  /**
   * 查询列表 - 返回格式匹配后端
   */
  list(key, filter = null, page = 1, pageSize = 20) {
    let data = this.getCollection(key)
    
    if (filter) {
      data = data.filter(item => {
        return Object.keys(filter).every(k => {
          if (filter[k] === undefined || filter[k] === '') return true
          return item[k] === filter[k] || String(item[k]).includes(String(filter[k]))
        })
      })
    }

    const total = data.length
    const start = (page - 1) * pageSize
    const list = data.slice(start, start + pageSize)

    return { 
      list, 
      total, 
      currentPage: page, 
      pageTotal: Math.ceil(total / pageSize) || 1,
      dataTotal: total,
      pageSize 
    }
  },

  /**
   * 根据 ID 获取
   */
  getById(key, id) {
    const data = this.getCollection(key)
    return data.find(item => item.id === id) || null
  },

  /**
   * 添加数据
   */
  add(key, item) {
    const data = this.getCollection(key)
    const newItem = {
      id: generateId(),
      create_time: new Date().toISOString(),
      update_time: new Date().toISOString(),
      ...item
    }
    data.push(newItem)
    this.saveCollection(key, data)
    return newItem
  },

  /**
   * 更新数据
   */
  update(key, id, updates) {
    const data = this.getCollection(key)
    const index = data.findIndex(item => item.id === id)
    if (index === -1) return null
    
    data[index] = {
      ...data[index],
      ...updates,
      update_time: new Date().toISOString()
    }
    this.saveCollection(key, data)
    return data[index]
  },

  /**
   * 删除数据
   */
  delete(key, id) {
    const data = this.getCollection(key)
    const index = data.findIndex(item => item.id === id)
    if (index === -1) return false
    
    data.splice(index, 1)
    this.saveCollection(key, data)
    return true
  },

  /**
   * 批量删除
   */
  deleteMany(key, ids) {
    const data = this.getCollection(key)
    const newData = data.filter(item => !ids.includes(item.id))
    this.saveCollection(key, newData)
    return ids.length
  },

  /**
   * 切换状态
   */
  toggleStatus(key, id, field = 'status') {
    const data = this.getCollection(key)
    const item = data.find(i => i.id === id)
    if (!item) return null
    
    item[field] = item[field] === 1 ? 0 : 1
    item.update_time = new Date().toISOString()
    this.saveCollection(key, data)
    return item
  },

  /**
   * 初始化种子数据
   */
  seed(key, items) {
    const existing = this.getCollection(key)
    if (existing.length === 0) {
      const seeded = items.map(item => ({
        create_time: new Date().toISOString(),
        update_time: new Date().toISOString(),
        ...item,
        // 如果原始数据没有 id，才生成新的
        id: item.id || generateId()
      }))
      this.saveCollection(key, seeded)
    }
  }
}

// ========== 核心功能 ==========

/**
 * 设置 Mock 模式
 * @param {string} mode - off | demo | develop
 */
export function setMockMode(mode) {
  const validModes = [MODE_OFF, MODE_DEMO, MODE_DEVELOP]
  if (!validModes.includes(mode)) {
    log('error', `无效的模式: ${mode}, 有效值: ${validModes.join(', ')}`)
    return
  }
  currentMode = mode
  log('system', `模式切换为: ${mode}`)
}

/**
 * 获取当前模式
 */
export function getMockMode() {
  return currentMode
}

/**
 * 检查是否启用
 */
export function isMockEnabled() {
  return currentMode !== MODE_OFF
}

/**
 * 是否为开发模式（允许写操作）
 */
export function isDevelopMode() {
  return currentMode === MODE_DEVELOP
}

/**
 * 是否为演示模式（只读）
 */
export function isDemoMode() {
  return currentMode === MODE_DEMO
}

/**
 * 设置全局配置
 */
export function setMockConfig(config) {
  globalConfig = { ...globalConfig, ...config }
  log('system', '全局配置已更新:', globalConfig)
}

/**
 * 注册单个 Mock
 * @param {MockConfig} config
 */
export function registerMock(config) {
  const { url, method = '*', handler, options = {} } = config
  
  const key = `${method.toUpperCase()}:${url}`
  mockRegistry.set(key, {
    url,
    method: method.toLowerCase(),
    handler,
    options: {
      delay: null,
      errorRate: globalConfig.errorRate,
      persist: false,
      persistKey: null,
      ...options
    }
  })
  
  log('registry', `注册: ${key}`)
}

/**
 * 批量注册 Mock
 * @param {MockConfig[]} configs
 */
export function registerMocks(configs) {
  configs.forEach(config => registerMock(config))
  log('registry', `共注册 ${mockRegistry.size} 个 Mock`)
}

/**
 * 取消注册
 */
export function unregisterMock(url, method = '*') {
  const key = `${method.toUpperCase()}:${url}`
  mockRegistry.delete(key)
}

/**
 * 清空所有注册
 */
export function clearMocks() {
  mockRegistry.clear()
  log('registry', '已清空所有注册')
}

/**
 * 匹配 Mock
 */
function matchMock(url, method) {
  const m = method.toLowerCase()
  
  for (const [key, config] of mockRegistry) {
    // 方法检查
    if (config.method !== '*' && config.method !== m) continue
    
    // URL 匹配（支持字符串精确匹配和正则）
    const urlPattern = config.url
    
    if (urlPattern instanceof RegExp) {
      if (urlPattern.test(url)) return config
    } else if (typeof urlPattern === 'string') {
      // 精确匹配或路径匹配（确保完整路径匹配，避免部分匹配）
      if (url === urlPattern) {
        return config
      }
      // 检查是否是完整路径匹配（url 应该以 urlPattern 结尾，或者是完整匹配）
      const pathParts = url.split('?')
      const cleanUrl = pathParts[0]
      if (cleanUrl === urlPattern) {
        return config
      }
    }
  }
  
  return null
}

/**
 * 触发事件
 */
function emit(event, data) {
  const listeners = eventListeners[event] || []
  listeners.forEach(fn => fn(data))
}

/**
 * 添加事件监听
 */
export function onMockEvent(event, listener) {
  if (!eventListeners[event]) {
    eventListeners[event] = []
  }
  eventListeners[event].push(listener)
}

/**
 * 移除事件监听
 */
export function offMockEvent(event, listener) {
  const listeners = eventListeners[event]
  if (listeners) {
    const index = listeners.indexOf(listener)
    if (index > -1) listeners.splice(index, 1)
  }
}

/**
 * Mock 拦截器主函数
 */
export async function mockInterceptor(axiosConfig) {
  if (!isMockEnabled()) return false

  const url = axiosConfig.url || ''
  const method = axiosConfig.method || 'get'
  const data = axiosConfig.data
  const params = axiosConfig.params

  // 触发前置事件
  emit('beforeRequest', { url, method, data, params })

  // 匹配 Mock
  const mock = matchMock(url, method)
  
  if (!mock) {
    log('miss', `${method.toUpperCase()} ${url}`)
    return false
  }

  log('hit', `${method.toUpperCase()} ${url}`)

  // 计算延迟
  const delayConfig = mock.options.delay || globalConfig.delay
  const delay = typeof delayConfig === 'number' 
    ? delayConfig 
    : randomDelay(delayConfig.min, delayConfig.max)

  // 应用延迟
  await new Promise(resolve => setTimeout(resolve, delay))

  // 错误模拟
  const errorRate = mock.options.errorRate ?? globalConfig.errorRate
  if (shouldTriggerError(errorRate)) {
    const error = { code: 500, message: '模拟服务器错误', data: null }
    emit('onError', { url, method, error })
    log('error', '触发模拟错误')
    return error
  }

  // 执行处理器
  try {
    const result = await mock.handler({
      url,
      method,
      data,
      params,
      headers: axiosConfig.headers,
      store,
      mockMode: currentMode,
      isDemoMode: isDemoMode(),
      isDevelopMode: isDevelopMode()
    })

    // 触发后置事件
    emit('afterRequest', { url, method, result })

    log('response', result)
    return result

  } catch (error) {
    const result = { code: 500, message: error.message, data: null }
    emit('onError', { url, method, error: result })
    log('error', '处理器异常:', error.message)
    return result
  }
}

// ========== 响应构建器 ==========

/**
 * 成功响应
 */
export function ok(data, message = 'success') {
  return { code: 200, message, data }
}

/**
 * 错误响应
 */
export function fail(message, code = 500, data = null) {
  return { code, message, data }
}

/**
 * 分页响应 - 匹配后端格式
 * 后端返回格式: { list, currentPage, pageTotal, dataTotal }
 */
export function page(list, total, currentPage = 1, pageSize = 20) {
  const pageTotal = Math.ceil(total / pageSize) || 1
  return ok({
    list,
    currentPage,
    pageTotal,
    dataTotal: total
  })
}

/**
 * 未授权响应
 */
export function unauthorized(message = '未授权访问') {
  return { code: 401, message, data: null }
}

/**
 * 禁止访问响应（演示模式写操作）
 */
export function forbidden(message = '演示模式下禁止此操作') {
  return { code: 403, message, data: null }
}

// ========== 导出 ==========
export default {
  // 模式管理
  setMockMode,
  getMockMode,
  isMockEnabled,
  isDevelopMode,
  isDemoMode,
  
  // 配置
  setMockConfig,
  
  // 注册
  registerMock,
  registerMocks,
  unregisterMock,
  clearMocks,
  
  // 拦截
  mockInterceptor,
  
  // 事件
  onMockEvent,
  offMockEvent,
  
  // 数据存储
  store,
  
  // 持久化
  persist,
  
  // 响应构建
  ok,
  fail,
  page,
  unauthorized,
  forbidden,
  
  // 工具
  generateId,
  deepClone
}
