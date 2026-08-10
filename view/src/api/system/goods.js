import { request } from '@/utils/request.js'

/**
 * 商品管理 API
 * 后端请求格式:
 * - 列表: { page, size, so: { keyword } }
 * - 添加: { name, type, val, money, blurb }
 * - 编辑: { id, name, type, val, money, blurb, state }
 * - 编辑状态: { id, state }
 * 响应格式: { code, msg, data: { list, currentPage, pageTotal, dataTotal } }
 */
export default {
  /**
   * 获取商品列表
   * @param {Object} params - 查询参数
   * @param {number} params.page - 页码
   * @param {number} params.size - 每页数量
   * @param {Object} params.so - 搜索条件
   * @param {string} params.so.keyword - 搜索关键词
   */
  getList(params = {}) {
    const backendParams = {
      page: params.page || 1,
      size: params.size || params.page_size || 20
    }
    
    if (params.keyword || params.name) {
      backendParams.so = {
        keyword: params.keyword || params.name
      }
    }
    
    return request({
      url: '/admin/goods/list',
      method: 'post',
      data: backendParams
    })
  },

  /**
   * 添加商品
   * @param {Object} data - 商品数据
   * @param {string} data.name - 商品名称
   * @param {string} data.type - 类型 "vip"|"fen"|"agent"|"addsn"
   * @param {number} data.val - 值
   * @param {number} data.money - 价格
   * @param {string} data.blurb - 简介
   */
  add(data = {}) {
    return request({
      url: '/admin/goods/add',
      method: 'post',
      data: {
        name: data.name,
        type: data.type || 'vip',
        val: data.val || data.days || 0,
        money: data.money || data.price || 0,
        blurb: data.blurb || data.description || ''
      }
    })
  },

  /**
   * 编辑商品
   * @param {Object} data - 商品数据
   * @param {number} data.id - 商品ID
   * @param {string} data.name - 商品名称
   * @param {string} data.type - 类型
   * @param {number} data.val - 值
   * @param {number} data.money - 价格
   * @param {string} data.blurb - 简介
   * @param {string} data.state - 状态 "y"|"n"
   */
  edit(data = {}) {
    return request({
      url: '/admin/goods/edit',
      method: 'post',
      data: {
        id: data.id,
        name: data.name,
        type: data.type,
        val: data.val,
        money: data.money,
        blurb: data.blurb || '',
        state: data.state
      }
    })
  },

  /**
   * 编辑商品状态
   * @param {Object} data - 状态数据
   * @param {number} data.id - 商品ID
   * @param {string} data.state - 状态 "y"|"n"
   */
  editState(data = {}) {
    return request({
      url: '/admin/goods/editState',
      method: 'post',
      data: {
        id: data.id,
        state: data.state || (data.status === 1 ? 'y' : 'n')
      }
    })
  },

  /**
   * 删除商品
   */
  del(id) {
    return request({
      url: '/admin/goods/del',
      method: 'post',
      data: { id }
    })
  }
}
