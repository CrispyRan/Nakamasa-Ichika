import { request } from '@/utils/request.js'

/**
 * 版本管理 API
 * 严格按照静态文件 index-CDuwvIoe.js 中的 V 对象实现
 * 
 * 原始静态文件 API 定义：
 * V={
 *   get(e,s,i){return t.post("/api/"+r+"/ver/get",{page:e,size:s,so:i})},
 *   submit(e){return t.post("/api/"+r+"/ver/submit",e)},
 *   del(e){return t.post("/api/"+r+"/ver/del",{id:e})},
 *   delall(e){return t.post("/api/"+r+"/ver/delall",{ids:e})},
 *   discard(e,s){return t.post("/api/"+r+"/ver/discard",{id:e,discard:s})},
 *   getMilist(){return t.get("/api/"+r+"/ver/milist")},
 *   getGroup(){return t.get("/api/"+r+"/ver/group")}
 * }
 */

/**
 * 获取版本列表
 * @param {number} page - 页码
 * @param {number} size - 每页数量
 * @param {object} so - 搜索条件 { ver_key, keyword }
 */
export function get(page, size, so = {}) {
  return request({
    url: '/admin/ver/get',
    method: 'post',
    data: { pg: page, size, so }
  })
}

/**
 * 获取版本列表（兼容方法）
 */
export function getList(params = {}) {
  const { page = 1, size = 10, ver_key, keyword } = params
  const so = {}
  if (ver_key) so.ver_key = ver_key
  if (keyword) so.keyword = keyword
  return get(page, size, so)
}

/**
 * 提交版本（添加/编辑）
 * @param {object} data - 版本数据
 */
export function submit(data) {
  return request({
    url: '/admin/ver/submit',
    method: 'post',
    data
  })
}

/**
 * 添加版本（兼容方法）
 */
export function add(data) {
  return submit(data)
}

/**
 * 编辑版本（兼容方法）
 */
export function edit(data) {
  return submit(data)
}

/**
 * 删除版本
 * @param {number} id - 版本ID
 */
export function del(id) {
  return request({
    url: '/admin/ver/del',
    method: 'post',
    data: { id }
  })
}

/**
 * 批量删除版本
 * @param {array} ids - 版本ID数组
 */
export function delall(ids) {
  return request({
    url: '/admin/ver/delall',
    method: 'post',
    data: { ids }
  })
}

/**
 * 设置弃用状态
 * @param {number} id - 版本ID
 * @param {boolean} discard - 是否弃用
 */
export function discard(id, discard) {
  return request({
    url: '/admin/ver/discard',
    method: 'post',
    data: { id, discard }
  })
}

/**
 * 获取加密方案列表
 */
export function getMilist() {
  return request({
    url: '/admin/ver/milist',
    method: 'get'
  })
}

/**
 * 获取版本分组列表
 */
export function getGroup() {
  return request({
    url: '/admin/ver/group',
    method: 'get'
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
  delall,
  discard,
  getMilist,
  getGroup
}
