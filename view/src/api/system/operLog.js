import { request } from '@/utils/request.js'

/**
 * 操作日志接口
 */
export default {
  /**
   * 数据列表
   * @returns
   */
  getPageList(params = {}) {
    return request({
      url: '/admin/logs/getOperLogPageList',
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
      url: '/admin/logs/deleteOperLog',
      method: 'delete',
      data
    })
  }
}
