import { request } from '@/utils/request.js'

export default {
  /**
   * 获取积分事件列表
   */
  getEventList(params = {}) {
    return request({
      url: '/admin/fenEvent/list',
      method: 'post',
      data: params
    })
  },

  /**
   * 添加积分事件
   */
  addEvent(data = {}) {
    return request({
      url: '/admin/fenEvent/add',
      method: 'post',
      data
    })
  },

  /**
   * 编辑积分事件
   */
  editEvent(data = {}) {
    return request({
      url: '/admin/fenEvent/edit',
      method: 'post',
      data
    })
  },

  /**
   * 编辑积分事件状态
   */
  editEventState(data = {}) {
    return request({
      url: '/admin/fenEvent/editState',
      method: 'post',
      data
    })
  },

  /**
   * 删除积分事件
   */
  delEvent(id) {
    return request({
      url: '/admin/fenEvent/del',
      method: 'post',
      data: { id }
    })
  },

  /**
   * 批量删除积分事件
   */
  delEventAll(ids) {
    return request({
      url: '/admin/fenEvent/delall',
      method: 'post',
      data: { ids }
    })
  },

  /**
   * 获取积分订单列表
   */
  getOrderList(params = {}) {
    return request({
      url: '/admin/fenOrder/list',
      method: 'post',
      data: params
    })
  },

  /**
   * 编辑积分订单
   */
  editOrder(data = {}) {
    return request({
      url: '/admin/fenOrder/edit',
      method: 'post',
      data
    })
  },

  /**
   * 删除积分订单
   */
  delOrder(id) {
    return request({
      url: '/admin/fenOrder/del',
      method: 'post',
      data: { id }
    })
  },

  /**
   * 批量删除积分订单
   */
  delOrderAll(ids) {
    return request({
      url: '/admin/fenOrder/delall',
      method: 'post',
      data: { ids }
    })
  }
}
