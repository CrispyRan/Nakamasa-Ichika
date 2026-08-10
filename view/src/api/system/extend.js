import { request } from '@/utils/request.js'

export default {
  /**
   * 获取扩展字段列表
   */
  getList(params = {}) {
    return request({
      url: '/admin/extend/list',
      method: 'post',
      data: params
    })
  },

  /**
   * 添加扩展字段
   */
  add(data = {}) {
    return request({
      url: '/admin/extend/add',
      method: 'post',
      data
    })
  },

  /**
   * 编辑扩展字段
   */
  edit(data = {}) {
    return request({
      url: '/admin/extend/edit',
      method: 'post',
      data
    })
  },

  /**
   * 删除扩展字段
   */
  del(id) {
    return request({
      url: '/admin/extend/del',
      method: 'post',
      data: { id }
    })
  }
}
