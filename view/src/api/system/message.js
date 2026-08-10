import { request } from '@/utils/request.js'

export default {
  /**
   * 获取留言列表
   */
  getList(params = {}) {
    return request({
      url: '/admin/message/list',
      method: 'post',
      data: params
    })
  },

  /**
   * 编辑留言
   */
  edit(data = {}) {
    return request({
      url: '/admin/message/edit',
      method: 'post',
      data
    })
  },

  /**
   * 删除留言
   */
  del(id) {
    return request({
      url: '/admin/message/del',
      method: 'post',
      data: { id }
    })
  }
}
