import { request } from '@/utils/request.js'

/**
 * 系统设置接口
 */
export default {
  /**
   * 获取配置列表
   * @returns
   */
  getConfigList(params) {
    return request({
      url: '/admin/config/index',
      method: 'get',
      params
    })
  },

  /**
   * 删除配置
   * @returns
   */
  destroy(data) {
    return request({
      url: '/admin/config/destroy',
      method: 'delete',
      data
    })
  },

  /**
   * 保存配置
   * @returns
   */
  save(data = {}) {
    return request({
      url: '/admin/config/save',
      method: 'post',
      data
    })
  },

  /**
   * 修改配置
   * @returns
   */
  update(id, data = {}) {
    return request({
      url: '/admin/config/update?id=' + id,
      method: 'put',
      data
    })
  },

  /**
   * 按 keys 更新配置
   * @returns
   */
  updateByKeys(data) {
    return request({
      url: '/admin/config/updateByKeys',
      method: 'post',
      data
    })
  },

  /**
   * 批量修改配置值
   * @returns
   */
  batchUpdate(data) {
    return request({
      url: '/admin/config/batchUpdate',
      method: 'post',
      data
    })
  },

  /**
   * 获取组列表
   * @returns
   */
  getConfigGroupList(params = {}) {
    return request({
      url: '/admin/configGroup/index',
      method: 'get',
      params
    })
  },

  /**
   * 保存配置组
   * @returns
   */
  saveConfigGroup(data = {}) {
    return request({
      url: '/admin/configGroup/save',
      method: 'post',
      data
    })
  },

  /**
   * 更新配置组
   * @returns
   */
  updateConfigGroup(id, data = {}) {
    return request({
      url: '/admin/configGroup/update?id=' + id,
      method: 'put',
      data
    })
  },

  /**
   * 删除配置组
   * @returns
   */
  deleteConfigGroup(data = {}) {
    return request({
      url: '/admin/configGroup/destroy',
      method: 'delete',
      data
    })
  },

  /**
   * 邮箱测试
   * @returns
   */
  testEmail(data = {}) {
    return request({
      url: '/admin/configGroup/email',
      method: 'post',
      data
    })
  }
}
