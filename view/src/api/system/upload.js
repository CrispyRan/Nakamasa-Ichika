import { request } from '@/utils/request.js'

/**
 * 上传 API
 * 
 * 注意：上传文件使用 FormData，浏览器会自动设置正确的 Content-Type（包含 boundary）
 * 不要手动设置 Content-Type: multipart/form-data，否则会丢失 boundary
 */
export default {
  /**
   * 上传图片
   * @param {FormData} formData - 包含文件的 FormData 对象
   * @param {Function} onProgress - 上传进度回调 (可选)
   * @returns {Promise}
   */
  img(formData, onProgress) {
    return request({
      url: '/admin/upload/img',
      method: 'post',
      data: formData,
      // 不设置 Content-Type，让浏览器自动处理 multipart/form-data 的 boundary
      headers: {
        'Content-Type': undefined
      },
      onUploadProgress: onProgress
    })
  },

  /**
   * 上传文件（通用）
   * @param {FormData} formData - 包含文件的 FormData 对象
   * @param {Function} onProgress - 上传进度回调 (可选)
   * @returns {Promise}
   */
  file(formData, onProgress) {
    return request({
      url: '/admin/upload/index',
      method: 'post',
      data: formData,
      headers: {
        'Content-Type': undefined
      },
      onUploadProgress: onProgress
    })
  }
}
