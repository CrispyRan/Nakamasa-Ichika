import { request } from '@/utils/request.js'

export default {
  /**
   * 获取数据
   * @returns
   */
  getList(params = {}) {
    return request({
      url: '/admin/menu/index',
      method: 'get',
      params
    })
  },

  /**
   * 可操作菜单
   * @returns
   */
  accessMenu(params = {}) {
    return request({
      url: '/admin/menu/accessMenu',
      method: 'get',
      params
    })
  },

  /**
   * 添加数据
   * @returns
   */
  save(params = {}) {
    return request({
      url: '/admin/menu/save',
      method: 'post',
      data: params
    })
  },

  /**
   * 删除数据
   * @returns
   */
  destroy(data) {
    return request({
      url: '/admin/menu/destroy',
      method: 'delete',
      data
    })
  },

  /**
   * 更新数据
   * @returns
   */
  update(id, data = {}) {
    return request({
      url: '/admin/menu/update?id=' + id,
      method: 'put',
      data
    })
  }
}
