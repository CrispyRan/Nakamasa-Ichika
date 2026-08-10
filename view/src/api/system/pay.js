import { request } from '@/utils/request.js'

export default {
  /**
   * 获取支付插件列表和配置信息
   * 后端返回格式 (v2 - 动态通道数组):
   * {
   *   channels: [{ id, label, icon, state, type, config }],
   *   plugins: [{ id, type, name, extra, form }]
   * }
   */
  getInfo() {
    return request({
      url: '/admin/app/pay',
      method: 'get'
    })
  },

  /**
   * 编辑支付配置 (v2 - 通道数组格式)
   * @param {Object} params
   * @param {number} params.id - 应用ID
   * @param {Array} params.channels - 通道配置数组
   * @param {string} params.channels[].id - 通道ID (如 "ali", "wx")
   * @param {string} params.channels[].state - "on"|"off"
   * @param {string} params.channels[].type - 支付引擎ID (如 "jie", "ali", "wx")
   * @param {Object} params.channels[].config - 引擎配置
   */
  edit({ id, channels } = {}) {
    return request({
      url: '/admin/app/pay/edit',
      method: 'post',
      data: { id, channels }
    })
  }
}