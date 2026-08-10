import { request } from '@/utils/request.js'

/**
 * 日志管理 API
 */
export default {
  /**
   * 获取日志列表
   * @param {number} page - 页码
   * @param {number} size - 每页数量
   * @param {Object} form - 搜索条件 { type, keyword }
   * @param {string} logType - 日志类型 (user, admin)
   */
  getList(page = 1, size = 10, form = {}, logType = 'user') {
    // 根据日志类型选择不同的 API
    const url = logType === 'user' 
      ? '/admin/logs/list/user' 
      : '/admin/logs/list/admin'
    
    return request({
      url,
      method: 'post',
      data: { 
        page, 
        size, 
        so: {
          type: form.type,
          keyword: form.keyword
        }
      }
    })
  },

  /**
   * 获取日志类型
   * @param {string} logType - 日志类型 (user, admin)
   */
  getType(logType = 'user') {
    const url = logType === 'user'
      ? '/admin/logs/type/user'
      : '/admin/logs/type/admin'
    
    return request({
      url,
      method: 'get'
    })
  },

  /**
   * 删除单条日志
   * @param {number} id - 日志ID
   */
  del(id) {
    return request({
      url: '/admin/logs/del',
      method: 'post',
      data: { id }
    })
  },

  /**
   * 批量删除日志
   * @param {Array} ids - 日志ID数组
   */
  delAll(ids) {
    return request({
      url: '/admin/logs/delall',
      method: 'post',
      data: { ids }
    })
  },

  /**
   * 清理日志
   * @param {Object} params - 清理参数
   * @param {number} params.time - 时间天数 (7, 15, 30, 90)
   */
  clean(params = {}) {
    return request({
      url: '/admin/logs/clean',
      method: 'post',
      data: params
    })
  },

  /**
   * 获取用户日志列表
   */
  getUserLogs(params = {}) {
    return this.getList(params.page || 1, params.size || 10, params.so || {}, 'user')
  },

  /**
   * 获取管理员日志列表
   */
  getAdminLogs(params = {}) {
    return this.getList(params.page || 1, params.size || 10, params.so || {}, 'admin')
  },

  /**
   * 获取用户日志类型
   */
  getUserLogTypes() {
    return this.getType('user')
  },

  /**
   * 获取管理员日志类型
   */
  getAdminLogTypes() {
    return this.getType('admin')
  }
}
