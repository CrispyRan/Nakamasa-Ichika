import { request } from '@/utils/request.js'

export default {
  /**
   * 获取管理员列表
   */
  getList(params = {}) {
    return request({
      url: '/admin/admList/list',
      method: 'post',
      data: params
    })
  },

  /**
   * 添加管理员
   */
  add(data = {}) {
    return request({
      url: '/admin/admList/add',
      method: 'post',
      data
    })
  },

  /**
   * 编辑管理员
   */
  edit(data = {}) {
    return request({
      url: '/admin/admList/edit',
      method: 'post',
      data
    })
  },

  /**
   * 删除管理员
   */
  del(id) {
    return request({
      url: '/admin/admList/del',
      method: 'post',
      data: { id }
    })
  },

  /**
   * 设置头像
   */
  setAvatar(data = {}) {
    return request({
      url: '/admin/admin/setAvatars',
      method: 'post',
      data
    })
  }
}
