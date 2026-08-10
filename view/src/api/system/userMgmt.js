import { request } from '@/utils/request.js'

/**
 * 用户管理 API
 * 严格按照静态文件 index-CDuwvIoe.js 中的 U 对象实现
 * 
 * 原始静态文件 API 定义：
 * U={
 *   list(e,s,i){return t.post("/api/"+r+"/user/list",{page:e,size:s,so:i})},
 *   add(e){return t.post("/api/"+r+"/user/add",e)},
 *   award(e){return t.post("/api/"+r+"/user/award",e)},
 *   get(e){return t.post("/api/"+r+"/user/get",{id:e})},
 *   edit(e){return t.post("/api/"+r+"/user/edit",e)},
 *   editExtend(e,s){return t.post("/api/"+r+"/user/editExtend",{id:e,extend:s})},
 *   del(e){return t.post("/api/"+r+"/user/del",{id:e})},
 *   delall(e){return t.post("/api/"+r+"/user/delall",{ids:e})},
 *   unbindSn(e,s){return t.post("/api/"+r+"/user/unbindSn",{id:e,udid:s})}
 * }
 */

/**
 * 获取用户列表
 * 支持两种调用方式：
 * 1. list(page, size, so) - 与静态文件一致
 * 2. getList({ keyword, keywordType, status, ug, page, size }) - 兼容现有代码
 */
export function list(page, size, so = {}) {
  return request({
    url: '/admin/user/list',
    method: 'post',
    data: { page, size, so }
  })
}

/**
 * 获取用户列表（兼容方法）
 * @param {object} params - 参数对象
 */
export function getList(params = {}) {
  const { keyword, keywordType, status, ug, page = 1, size = 20 } = params
  const so = {}
  if (keyword) {
    so.keyword = keyword
    so.keywordType = keywordType || 'acctno'
  }
  if (status) so.status = status
  if (ug) so.ug = ug
  
  return request({
    url: '/admin/user/list',
    method: 'post',
    data: { page, size, so }
  })
}

/**
 * 获取用户详情
 * @param {number} id - 用户ID
 */
export function get(id) {
  return request({
    url: '/admin/user/get',
    method: 'post',
    data: { id }
  })
}

/**
 * 添加用户
 * @param {object} data - 用户数据
 */
export function add(data) {
  return request({
    url: '/admin/user/add',
    method: 'post',
    data
  })
}

/**
 * 编辑用户
 * @param {object} data - 用户数据
 */
export function edit(data) {
  return request({
    url: '/admin/user/edit',
    method: 'post',
    data
  })
}

/**
 * 编辑用户扩展信息
 * @param {number} id - 用户ID
 * @param {object} extend - 扩展信息对象
 */
export function editExtend(id, extend) {
  return request({
    url: '/admin/user/editExtend',
    method: 'post',
    data: { id, extend }
  })
}

/**
 * 发送奖励
 * @param {object} data - 奖励数据
 */
export function award(data) {
  return request({
    url: '/admin/user/award',
    method: 'post',
    data
  })
}

/**
 * 删除用户
 * @param {number} id - 用户ID
 */
export function del(id) {
  return request({
    url: '/admin/user/del',
    method: 'post',
    data: { id }
  })
}

/**
 * 批量删除用户
 * @param {array} ids - 用户ID数组
 */
export function delall(ids) {
  return request({
    url: '/admin/user/delall',
    method: 'post',
    data: { ids }
  })
}

/**
 * 解绑设备
 * @param {number} id - 用户ID
 * @param {string} udid - 设备码
 */
export function unbindSn(id, udid) {
  return request({
    url: '/admin/user/unbindSn',
    method: 'post',
    data: { id, udid }
  })
}

/**
 * 删除用户人脸
 * @param {number} id - 用户ID
 */
export function delFace(id) {
  return request({
    url: '/admin/user/delFace',
    method: 'post',
    data: { id }
  })
}

/**
 * 上传人脸覆盖特征（强制覆盖，不比对旧脸）
 * @param {number} id - 用户ID
 * @param {File} file - 人脸图片文件
 */
export function faceReg(id, file) {
  const formData = new FormData()
  formData.append('id', id)
  formData.append('file', file)
  return request({
    url: '/admin/user/faceReg',
    method: 'post',
    data: formData,
    // 不设置 Content-Type，让浏览器自动处理 multipart 的 boundary
    headers: {
      'Content-Type': undefined
    }
  })
}

// 默认导出所有方法
export default {
  list,
  getList,
  get,
  add,
  edit,
  editExtend,
  award,
  del,
  delall,
  unbindSn,
  delFace,
  faceReg
}
