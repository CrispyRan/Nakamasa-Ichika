import { request } from '@/utils/request.js'

/**
 * 加密方案 API
 * 严格按照静态文件中的 E 对象实现
 */

/**
 * 获取加密方案列表
 * @param {number} page - 页码
 * @param {object} so - 搜索条件 { keyword }
 */
export function get(page, so = {}) {
  return request({
    url: '/admin/encryption/list',
    method: 'post',
    data: { page, size: 12, so }
  })
}

/**
 * 获取加密方案列表（兼容方法）
 */
export function getList(params = {}) {
  const { page = 1, size = 12, keyword } = params
  const so = {}
  if (keyword) so.keyword = keyword
  return request({
    url: '/admin/encryption/list',
    method: 'post',
    data: { page, size, so }
  })
}

/**
 * 提交加密方案（添加/编辑）
 * @param {object} data - 方案数据
 */
export function submit(data) {
  return request({
    url: '/admin/encryption/submit',
    method: 'post',
    data
  })
}

/**
 * 添加加密方案（兼容方法）
 */
export function add(data) {
  return submit(data)
}

/**
 * 编辑加密方案（兼容方法）
 */
export function edit(data) {
  return submit(data)
}

/**
 * 删除加密方案
 * @param {number} id - 方案ID
 */
export function del(id) {
  return request({
    url: '/admin/encryption/del',
    method: 'post',
    data: { id }
  })
}

/**
 * 获取加密插件列表
 */
export function getPlug() {
  return request({
    url: '/admin/encryption/plug',
    method: 'get'
  })
}

/**
 * 编辑签名状态
 * @param {number} id - 方案ID
 * @param {string} sign - 签名状态 'y' | 'n'
 */
export function editSignStatus(id, sign) {
  return request({
    url: '/admin/encryption/editSign',
    method: 'post',
    data: { id, sign }
  })
}

// 默认导出所有方法
export default {
  get,
  getList,
  submit,
  add,
  edit,
  del,
  getPlug,
  editSignStatus
}
