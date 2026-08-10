import { request } from '@/utils/request.js'

export default {
  /**
   * 获取发送配置信息
   */
  getInfo() {
    return request({
      url: '/admin/send/getInfo',
      method: 'post'
    })
  },

  /**
   * 编辑发送配置
   */
  edit(data = {}) {
    return request({
      url: '/admin/send/edit',
      method: 'post',
      data
    })
  }
}
