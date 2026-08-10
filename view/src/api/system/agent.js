import { request } from '@/utils/request.js'

/**
 * 代理管理 API
 * 后端请求格式:
 * - 列表: { pg, size, so: { keyword } }
 * - 添加: { gid, note, user, utype }
 * 响应格式: { code, msg, data: { list, currentPage, pageTotal, dataTotal } }
 */
export default {
  /**
   * 获取代理列表
   * @param {Object} params - 查询参数
   * @param {number} params.pg - 页码
   * @param {number} params.size - 每页数量
   * @param {Object} params.so - 搜索条件
   * @param {string} params.so.keyword - 搜索关键词
   */
  getList(params = {}) {
    const backendParams = {
      pg: params.pg || params.page || 1,
      size: params.size || params.page_size || 20
    }
    
    if (params.keyword || params.username || params.name) {
      backendParams.so = {
        keyword: params.keyword || params.username || params.name
      }
    }
    
    return request({
      url: '/admin/agentList/list',
      method: 'post',
      data: backendParams
    })
  },

  /**
   * 添加代理
   * @param {Object} data - 代理数据
   * @param {number} data.gid - 代理分组ID
   * @param {string} data.note - 备注
   * @param {string} data.user - 用户账号
   * @param {string} data.utype - 用户类型
   */
  add(data = {}) {
    return request({
      url: '/admin/agentList/add',
      method: 'post',
      data: {
        gid: data.gid || data.group_id,
        note: data.note || '',
        user: data.user || data.username,
        utype: data.utype || 'acctno'
      }
    })
  },

  /**
   * 编辑代理
   * @param {Object} data - 代理数据
   */
  edit(data = {}) {
    return request({
      url: '/admin/agentList/edit',
      method: 'post',
      data
    })
  },

  /**
   * 删除代理
   */
  del(id) {
    return request({
      url: '/admin/agentList/del',
      method: 'post',
      data: { id }
    })
  },

  /**
   * 获取代理分组列表
   * @param {Object} params - 查询参数
   * @param {number} params.pg - 页码
   * @param {number} params.size - 每页数量
   * @param {Object} params.so - 搜索条件
   * @param {string} params.so.keyword - 搜索关键词
   */
  getGroupList(params = {}) {
    const backendParams = {
      pg: params.pg || params.page || 1,
      size: params.size || params.page_size || 20
    }
    
    if (params.keyword) {
      backendParams.so = { keyword: params.keyword }
    }
    
    return request({
      url: '/admin/agentGroup/list',
      method: 'post',
      data: backendParams
    })
  },

  /**
   * 获取代理提现列表
   */
  getCashList(params = {}) {
    return request({
      url: '/admin/agentCash/list',
      method: 'post',
      data: {
        page: params.page || 1,
        size: params.size || params.page_size || 20
      }
    })
  },

  /**
   * 编辑提现状态
   */
  editCash(data = {}) {
    return request({
      url: '/admin/agentCash/edit',
      method: 'post',
      data
    })
  }
}
