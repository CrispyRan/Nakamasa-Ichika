import { request } from '@/utils/request.js'

/**
 * 登录日志接口
 */
export default {
  /**
   * 数据列表
   * @returns
   */
  getPageList(params = {}) {
    return request({
      url: '/admin/logs/getLoginLogPageList',
      method: 'get',
      params
    })
  },

  /**
   * 删除数据
   * @returns
   */
  destroy(data) {
    return request({
      url: '/admin/logs/deleteLoginLog',
      method: 'delete',
      data
    })
  }
}
