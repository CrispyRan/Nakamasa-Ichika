import { request } from '@/utils/request.js'

export default {
  /**
   * 获取黑名单列表
   */
  getList(params = {}) {
    return request({
      url: '/admin/blocklist/list',
      method: 'post',
      data: params
    })
  },

  /**
   * 添加黑名单
   */
  add(data = {}) {
    return request({
      url: '/admin/blocklist/add',
      method: 'post',
      data
    })
  },

  /**
   * 删除黑名单
   */
  del(id) {
    return request({
      url: '/admin/blocklist/del',
      method: 'post',
      data: { id }
    })
  }
}
