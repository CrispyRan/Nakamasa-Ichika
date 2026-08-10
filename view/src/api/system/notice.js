import { request } from '@/utils/request.js'

/**
 * 公告管理 API
 * 后端请求格式:
 * - 列表: { page, size }
 * - 添加: { content, all }
 * - 编辑: { id, content }
 * 响应格式: { code, msg, data: { list, currentPage, pageTotal, dataTotal } }
 */
export default {
  /**
   * 获取公告列表
   * @param {Object} params - 查询参数
   * @param {number} params.page - 页码
   * @param {number} params.size - 每页数量
   */
  getList(params = {}) {
    return request({
      url: '/admin/notice/list',
      method: 'post',
      data: {
        page: params.page || 1,
        size: params.size || params.page_size || 20
      }
    })
  },

  /**
   * 添加公告
   * @param {Object} data - 公告数据
   * @param {string} data.content - 公告内容
   * @param {string} data.all - 全局公告 "y" 表示全局
   */
  add(data = {}) {
    return request({
      url: '/admin/notice/add',
      method: 'post',
      data: {
        content: data.content,
        all: data.all || ''
      }
    })
  },

  /**
   * 编辑公告
   * @param {Object} data - 公告数据
   * @param {number} data.id - 公告ID
   * @param {string} data.content - 公告内容
   */
  edit(data = {}) {
    return request({
      url: '/admin/notice/edit',
      method: 'post',
      data: {
        id: data.id,
        content: data.content
      }
    })
  },

  /**
   * 删除公告
   */
  del(id) {
    return request({
      url: '/admin/notice/del',
      method: 'post',
      data: { id }
    })
  }
}