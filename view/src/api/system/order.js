import { request } from '@/utils/request.js'

/**
 * 订单管理 API
 * 后端请求格式: { page, size, so: { keyword_type, keyword, status, type, ptype, date } }
 * 后端响应格式: { code, msg, data: { list, currentPage, pageTotal, dataTotal } }
 */
export default {
  /**
   * 获取订单列表
   * @param {Object} params - 查询参数
   * @param {number} params.page - 页码
   * @param {number} params.size - 每页数量
   * @param {Object} params.so - 搜索条件
   * @param {string} params.so.keyword_type - 关键词类型 "order_no"|"trade_no"|"name"
   * @param {string} params.so.keyword - 搜索关键词
   * @param {string} params.so.status - 状态 "0"=未支付 "1"=已支付
   * @param {string} params.so.type - 商品类型
   * @param {string} params.so.ptype - 支付类型 "ali"|"wx"
   * @param {Array} params.so.date - 日期范围 [start, end]
   */
  getList(params = {}) {
    const backendParams = {
      page: params.page || 1,
      size: params.size || params.page_size || 20
    }
    
    // 构建搜索条件
    const so = {}
    if (params.order_no || params.keyword) {
      so.keyword_type = params.keyword_type || 'order_no'
      so.keyword = params.order_no || params.keyword
    }
    if (params.status !== undefined) {
      so.status = String(params.status)
    }
    if (params.type) {
      so.type = params.type
    }
    if (params.pay_type || params.ptype) {
      so.ptype = params.pay_type || params.ptype
    }
    if (params.date || params.dateRange) {
      so.date = params.date || params.dateRange
    }
    
    if (Object.keys(so).length > 0) {
      backendParams.so = so
    }
    
    return request({
      url: '/admin/order/list',
      method: 'post',
      data: backendParams
    })
  },

  /**
   * 订单统计
   * @param {string} time - 时间范围 "today"|"yesterday"
   */
  statistics(time = 'today') {
    return request({
      url: '/admin/order/statistics',
      method: 'post',
      data: { time }
    })
  },

  /**
   * 编辑订单
   */
  edit(data = {}) {
    return request({
      url: '/admin/order/edit',
      method: 'post',
      data
    })
  },

  /**
   * 关闭订单
   */
  close(id) {
    return request({
      url: '/admin/order/close',
      method: 'post',
      data: { id }
    })
  },

  /**
   * 订单退款
   */
  refund(data = {}) {
    return request({
      url: '/admin/order/refund',
      method: 'post',
      data
    })
  }
}
