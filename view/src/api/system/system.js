import { request } from '@/utils/request.js'

export default {
  /**
   * 清理缓存
   */
  clearCache() {
    return request({
      url: '/admin/system/clearCache',
      method: 'post'
    })
  },

  /**
   * 获取系统设置
   */
  getSet() {
    return request({
      url: '/admin/system/getSet',
      method: 'post'
    })
  },

  /**
   * 编辑系统设置
   */
  editSet(data = {}) {
    return request({
      url: '/admin/system/editSet',
      method: 'post',
      data
    })
  },

  /**
   * 获取用户API路由
   */
  getUserApiRouter() {
    return request({
      url: '/admin/system/getUserApiRouter',
      method: 'post'
    })
  },

  /**
   * 编辑用户API路由
   */
  editUserApiRouter(data = {}) {
    return request({
      url: '/admin/system/editUserApiRouter',
      method: 'post',
      data
    })
  },

  /**
   * 切换用户API路由状态
   */
  switchUserApiRouter(data = {}) {
    return request({
      url: '/admin/system/switchUserApiRouter',
      method: 'post',
      data
    })
  },

  /**
   * 获取用户API代码
   */
  getUserApiCode() {
    return request({
      url: '/admin/system/getUserApiCode',
      method: 'post'
    })
  },

  /**
   * 编辑用户API代码
   */
  editUserApiCode(data = {}) {
    return request({
      url: '/admin/system/editUserApiCode',
      method: 'post',
      data
    })
  },

  /**
   * 切换用户API代码状态
   */
  switchUserApiCode(data = {}) {
    return request({
      url: '/admin/system/switchUserApiCode',
      method: 'post',
      data
    })
  },

  /**
   * 获取系统更新日志
   */
  getUplog() {
    return request({
      url: '/admin/uplog',
      method: 'get'
    })
  }
}
