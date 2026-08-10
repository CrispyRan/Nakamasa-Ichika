import { request } from '@/utils/request.js'
import { toSeconds, parseVipTime } from '@/utils/sun.js'

/**
 * 卡密分组管理 API
 * 后端请求格式:
 * - 列表: { page, size, so: { keyword } }
 * - 添加: { name, type, val, price }
 * - 响应格式: { code, msg, data: { list, currentPage, pageTotal, dataTotal } }
 */
export default {
  /**
   * 获取卡密分组列表
   * @param {number} page - 页码
   * @param {number} size - 每页数量
   * @param {string} keyword - 搜索关键词
   */
  get(page = 1, size = 10, form = {}) {
    return request({
      url: '/admin/cdkGroup/list',
      method: 'post',
      data: {
        page,
        size,
        so: {
          keyword: form.keyword || ''
        }
      }
    })
  },

  /**
   * 获取所有分组（下拉选择用）
   * 返回: [{ id, name }]
   */
  getAll() {
    return request({
      url: '/admin/cdkGroup/get',
      method: 'get'
    })
  },

  /**
   * 提交（添加或编辑）
   * @param {Object} data - 分组数据
   * @param {string} data.id - 分组ID（编辑时必填）
   * @param {string} data.name - 分组名称
   * @param {string} data.type - 类型 "vip"|"fen"|"addsn"
   * @param {number} data.val - 值
   * @param {string} data.vipType - VIP类型单位 (s/i/h/d)
   * @param {number} data.price - 价格
   */
  submit(data = {}) {
    let val = data.val
    
    // 如果是VIP类型，需要根据单位转换为秒数
    if (data.type === 'vip' && data.vipType) {
      val = toSeconds(data.val, data.vipType)
    }
    
    const payload = {
      name: data.name,
      type: data.type || 'vip',
      val: val || 0,
      price: data.price || 1.0
    }
    
    // 编辑时添加ID
    if (data.id) {
      payload.id = data.id
    }
    
    return request({
      url: data.id ? '/admin/cdkGroup/edit' : '/admin/cdkGroup/add',
      method: 'post',
      data: payload
    })
  },

  /**
   * 添加卡密分组
   * @param {Object} data - 分组数据
   */
  add(data = {}) {
    return this.submit(data)
  },

  /**
   * 编辑卡密分组
   * @param {Object} data - 分组数据
   */
  edit(data = {}) {
    return this.submit(data)
  },

  /**
   * 删除单个卡密分组
   * @param {number} id - 分组ID
   */
  del(id) {
    return request({
      url: '/admin/cdkGroup/del',
      method: 'post',
      data: { id }
    })
  },

  /**
   * 批量删除卡密分组
   * @param {number[]} ids - 分组ID数组
   */
  delAll(ids) {
    return request({
      url: '/admin/cdkGroup/delAll',
      method: 'post',
      data: { ids }
    })
  }
}

// 导出工具函数供组件使用
export { toSeconds, parseVipTime }