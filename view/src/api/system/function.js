import { request } from '@/utils/request.js'

export default {
  /**
   * 获取云函数列表
   * @param {number} page - 页码
   * @param {number} size - 每页数量
   */
  getList(page = 1, size = 10) {
    return request({
      url: '/admin/functions/list',
      method: 'post',
      data: { pg: page, size }
    })
  },

  /**
   * 添加云函数
   * @param {Object} data - { name, code, notes, allow, fen, state }
   */
  add(data) {
    return request({
      url: '/admin/functions/add',
      method: 'post',
      data
    })
  },

  /**
   * 编辑云函数
   * @param {Object} data - { id, name, code, notes, allow, fen, state }
   */
  edit(data) {
    return request({
      url: '/admin/functions/edit',
      method: 'post',
      data
    })
  },

  /**
   * 提交（添加或编辑）
   * @param {Object} data - { id?, name, code, notes, allow, fen, state }
   */
  submit(data) {
    if (data.id) {
      return this.edit(data)
    }
    return this.add(data)
  },

  /**
   * 删除云函数
   * @param {number} id - 函数ID
   */
  del(id) {
    return request({
      url: '/admin/functions/del',
      method: 'post',
      data: { id }
    })
  },

  /**
   * 批量删除云函数
   * @param {number[]} ids - 函数ID数组
   */
  delAll(ids) {
    return request({
      url: '/admin/functions/delAll',
      method: 'post',
      data: { ids }
    })
  },

  /**
   * 获取函数代码（Base64编码）
   * @param {number} id - 函数ID
   */
  getCode(id) {
    return request({
      url: '/admin/functions/getCode',
      method: 'post',
      data: { id }
    })
  },

  /**
   * 获取函数详细信息
   * @param {number} id - 函数ID
   */
  getInfo(id) {
    return request({
      url: '/admin/functions/getInfo',
      method: 'post',
      data: { id }
    })
  },

  /**
   * 修改函数状态
   * @param {Object} data - { id, state }
   */
  editState(data) {
    return request({
      url: '/admin/functions/editState',
      method: 'post',
      data
    })
  }
}
