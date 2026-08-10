import { request } from '@/utils/request.js'

/**
 * 卡密版卡密管理 API
 * 对应后端路由: /admin/cdkKami/*
 * 数据表: u_cdk_kami
 * 与用户版(cdkUser)的区别：
 * - 卡密版卡密有邮箱、手机号绑定功能
 * - 有设备绑定管理功能(sn_max, sn_list)
 * - 有封禁功能(ban, ban_msg)
 * - 有发送奖励功能
 */
export default {
  /**
   * 获取卡密分组列表（下拉选择用）
   */
  getGroupList() {
    return request({
      url: '/admin/cdkGroup/get',
      method: 'get'
    })
  },

  /**
   * 获取卡密列表
   * @param {number} page - 页码
   * @param {number} size - 每页数量
   * @param {object} so - 搜索条件
   */
  getList(params = {}) {
    const backendParams = {
      page: params.page || 1,
      size: params.size || params.page_size || 20
    }

    // 构建搜索条件
    const so = {}

    // 卡密状态
    if (params.state) {
      so.state = params.state
    }

    // 使用状态
    if (params.use_state) {
      so.use_state = params.use_state
    }

    // 导出状态
    if (params.out_state) {
      so.out_state = params.out_state
    }

    // 创建人角色
    if (params.add_role) {
      so.add_role = params.add_role
    }

    // 卡密类型
    if (params.type) {
      so.type = params.type
    }

    // 分组ID
    if (params.gid) {
      so.gid = params.gid
    }

    // 创建时间范围
    if (params.add_time && params.add_time.length === 2) {
      so.add_time = params.add_time
    }

    // 使用时间范围
    if (params.use_time && params.use_time.length === 2) {
      so.use_time = params.use_time
    }

    // 关键词搜索
    if (params.keyword) {
      so.keyword = params.keyword
      so.keywordType = params.keywordType || 'cdk'
    }

    if (Object.keys(so).length > 0) {
      backendParams.so = so
    }

    return request({
      url: '/admin/cdkKami/list',
      method: 'post',
      data: backendParams
    })
  },

  /**
   * 添加/生成卡密
   * @param {object} data
   * @param {number} data.gid - 卡密组ID（必填）
   * @param {string} data.note - 备注
   * @param {number} data.length - 卡密长度（16-32）
   * @param {string} data.pre - 卡密前缀
   * @param {number} data.num - 生成数量（最多10000）
   * @param {string} data.out - 导出格式（txt/csv，可选）
   */
  add(data = {}) {
    return request({
      url: '/admin/cdkKami/add',
      method: 'post',
      data: {
        gid: data.gid,
        note: data.note || '',
        length: data.length || 16,
        pre: data.pre || '',
        num: data.num || 1,
        out: data.out || ''
      }
    })
  },

  /**
   * 编辑卡密
   * @param {object} data
   * @param {number} data.id - 卡密ID（必填）
   * @param {string} data.note - 备注
   * @param {string} data.password - 卡密密码
   * @param {number} data.vip - VIP到期时间戳
   * @param {number} data.val - 值（积分/增绑数）
   * @param {number} data.sn_max - 最大设备数
   * @param {number} data.ban - 封禁时间戳
   * @param {string} data.ban_msg - 封禁原因
   */
  edit(data = {}) {
    const backendData = { id: data.id, val: data.val || 0 }
    if (data.note !== undefined) backendData.note = data.note
    if (data.password !== undefined) backendData.password = data.password
    if (data.vip !== undefined) backendData.vip = data.vip
    if (data.sn_max !== undefined) backendData.sn_max = data.sn_max
    if (data.ban !== undefined) backendData.ban = data.ban
    if (data.ban_msg !== undefined) backendData.ban_msg = data.ban_msg

    return request({
      url: '/admin/cdkKami/edit',
      method: 'post',
      data: backendData
    })
  },

  /**
   * 删除单个卡密
   * @param {number} id - 卡密ID
   */
  del(id) {
    return request({
      url: '/admin/cdkKami/del',
      method: 'post',
      data: { id }
    })
  },

  /**
   * 批量删除卡密
   * @param {array} ids - 卡密ID数组
   */
  delAll(ids) {
    return request({
      url: '/admin/cdkKami/delall',
      method: 'post',
      data: { ids }
    })
  },

  /**
   * 批量导出卡密
   * @param {array} ids - 卡密ID数组
   * @param {string} out - 导出格式（txt/csv）
   */
  outAll(ids, out = 'txt') {
    return request({
      url: '/admin/cdkKami/outall',
      method: 'post',
      data: { ids, out }
    })
  },

  /**
   * 获取卡密使用日志
   * @param {number} id - 卡密ID
   */
  getLog(id) {
    return request({
      url: '/admin/cdkKami/getLog',
      method: 'post',
      data: { id }
    })
  },

  /**
   * 发送奖励
   * @param {object} data
   * @param {string} data.object - 奖励对象（vip/fen）
   * @param {number} data.val - 奖励值
   */
  award(data = {}) {
    return request({
      url: '/admin/cdkKami/award',
      method: 'post',
      data: {
        object: data.object || 'vip',
        val: data.val || 0
      }
    })
  },

  /**
   * 解绑设备
   * @param {number} id - 卡密ID
   * @param {string} udid - 设备UDID
   */
  unbindSn(id, udid) {
    return request({
      url: '/admin/cdkKami/unbindSn',
      method: 'post',
      data: { id, udid }
    })
  },

  /**
   * 切换卡密状态
   * @param {number} id - 卡密ID
   * @param {string} state - 状态 y/n
   */
  editState(id, state) {
    return request({
      url: '/admin/cdkKami/editState',
      method: 'post',
      data: { id, state }
    })
  },

  /**
   * 清理已使用的卡密
   */
  clear() {
    return request({
      url: '/admin/cdkKami/clear',
      method: 'get'
    })
  },

  /**
   * 下载导出内容
   * @param {string} content - 文件内容
   * @param {string} format - 文件格式（txt/csv）
   * @param {string} filename - 文件名
   */
  downloadContent(content, format = 'txt', filename = 'kami') {
    const mimeType = format === 'csv' ? 'text/csv' : 'text/plain'
    const blob = new Blob([content], { type: `${mimeType};charset=utf-8` })
    const url = window.URL.createObjectURL(blob)
    const link = document.createElement('a')
    link.href = url
    link.download = `${filename}.${format}`
    document.body.appendChild(link)
    link.click()
    document.body.removeChild(link)
    window.URL.revokeObjectURL(url)
  },

  /**
   * 下载导出文件
   * @param {string} url - 文件路径
   */
  downloadFile(url) {
    return request({
      url: `/api/admin/${url}`,
      method: 'get',
      responseType: 'blob'
    }).then(res => {
      try {
        const contentDisposition = res.headers['content-disposition']
        const match = /filename[^;=\n]*=((['"]).*?\2|[^;\n]*)/.exec(contentDisposition)
        let filename = 'kami'
        if (match && match[1]) {
          filename = match[1].replace(/['"]/g, '')
        }

        const blobUrl = window.URL.createObjectURL(new Blob([res.data]))
        const link = document.createElement('a')
        link.href = blobUrl
        link.download = filename
        document.body.appendChild(link)
        link.click()
        document.body.removeChild(link)
        window.URL.revokeObjectURL(blobUrl)
      } catch (e) {
        console.error('下载文件失败:', e)
      }
    })
  }
}