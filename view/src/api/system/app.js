import { request } from '@/utils/request.js'

/**
 * 应用管理 API
 * 后端请求格式:
 * - 列表: { pg, so: { type, keyword } }
 * - 添加: { app_name, app_type, app_inherit, app_logo }
 * - 响应格式: { code, msg, data: { list, currentPage, pageTotal, dataTotal } }
 */
export default {
  /**
   * 获取应用列表
   * @param {Object} params - 查询参数
   * @param {number} params.pg - 页码 (默认1, 范围1-11)
   * @param {Object} params.so - 搜索条件
   * @param {string} params.so.type - 应用类型过滤
   * @param {string} params.so.keyword - 搜索关键词
   */
  getList(params = {}) {
    const backendParams = {
      pg: params.pg || params.page || 1
    }
    
    // 构建搜索条件
    const so = {}
    if (params.type || params.app_type) {
      so.type = params.type || params.app_type
    }
    if (params.keyword || params.name) {
      so.keyword = params.keyword || params.name
    }
    
    if (Object.keys(so).length > 0) {
      backendParams.so = so
    }
    
    return request({
      url: '/admin/app/list',
      method: 'post',
      data: backendParams
    })
  },

  /**
   * 获取所有应用（下拉选择用）
   */
  getAll() {
    return request({
      url: '/admin/app/all',
      method: 'get'
    })
  },

  /**
   * 获取应用详情
   * 注意: 后端需要在请求头传递 appid
   * @param {number} id - 应用ID
   * @param {Array} field - 指定返回字段 (可选)
   */
  get(id, field = null) {
    const data = {}
    if (field && Array.isArray(field)) {
      data.field = field
    }
    // appid 通过请求头传递，由 request.js 自动处理
    return request({
      url: '/admin/app/get',
      method: 'post',
      data: { ...data, id },
      headers: { appid: id }
    })
  },

  /**
   * 添加应用
   * @param {Object} data - 应用数据
   * @param {string} data.app_name - 应用名称 (2-64位)
   * @param {string} data.app_type - 应用类型 "user" | "kami"
   * @param {number} data.app_inherit - 继承应用ID (可选)
   * @param {string} data.app_logo - 应用Logo (可选)
   */
  add(data = {}) {
    return request({
      url: '/admin/app/add',
      method: 'post',
      data: {
        app_name: data.app_name || data.name,
        app_type: data.app_type || data.type || 'user',
        app_inherit: data.app_inherit || data.inherit,
        app_logo: data.app_logo || data.logo || ''
      }
    })
  },

  /**
   * 编辑应用
   * 注意: 后端需要在请求头传递 appid
   * @param {Object} data - 应用数据，支持任意字段
   */
  edit(data = {}) {
    const { id, ...rest } = data
    const config = {
      url: '/admin/app/edit',
      method: 'post',
      data: rest
    }
    // 只有 id 有效时才设置 headers.appid，否则使用默认值
    if (id !== undefined && id !== null && id !== '') {
      config.headers = { appid: id }
    }
    return request(config)
  },

  /**
   * 删除应用
   */
  del(id) {
    return request({
      url: '/admin/app/del',
      method: 'post',
      data: { id }
    })
  },

  /**
   * 获取继承信息
   * 返回: { kami: [{ id, app_name }], user: [{ id, app_name }] }
   */
  getInherit() {
    return request({
      url: '/admin/app/getInherit',
      method: 'post',
      data: {}
    })
  },

  /**
   * 获取应用URL
   */
  getUrl() {
    return request({
      url: '/admin/app/getUrl',
      method: 'post',
      data: {}
    })
  },

  /**
   * 获取应用信息
   * @param {string|Array} field - 字段名数组，如果第一个参数是数组则直接使用
   */
  getInfo(field) {
    // 兼容 getInfo(['field1', 'field2']) 和 getInfo(id, ['field1']) 两种调用方式
    const fieldArray = Array.isArray(field) ? field : arguments[1]
    return request({
      url: '/admin/app/getInfo',
      method: 'post',
      data: { field: fieldArray }
    })
  }
}
