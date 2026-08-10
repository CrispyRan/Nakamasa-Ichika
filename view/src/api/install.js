import { request } from '@/utils/request.js'

export default {
  /**
   * 检查系统是否已安装
   * @returns {Promise} { code: 200 } 已安装, { code: -2 } 未安装
   */
  check() {
    return request({
      url: '/install/check',
      method: 'get'
    })
  },

  /**
   * 获取安装环境信息
   * @returns {Promise} 环境检测结果
   */
  env() {
    return request({
      url: '/install/env',
      method: 'get'
    })
  },

  /**
   * 执行安装
   * @param {Object} data 安装参数
   * @param {string} data.mysql_host MySQL主机
   * @param {number} data.mysql_port MySQL端口
   * @param {string} data.mysql_name 数据库名
   * @param {string} data.mysql_user 数据库用户
   * @param {string} data.mysql_pwd 数据库密码
   * @param {string} data.mysql_pre 表前缀
   * @param {string} data.redis_host Redis主机
   * @param {number} data.redis_port Redis端口
   * @param {string} [data.redis_pwd] Redis密码
   * @param {string} data.admin_user 管理员账号
   * @param {string} data.admin_pwd 管理员密码
   * @param {string} data.admin_authcode 授权码
   * @param {string} data.install_type 安装类型 new/upgrade
   * @param {string} [data.install_upgrade] 升级版本
   * @param {string} [data.adm_pwd] 管理员密码密钥(升级时需要)
   * @param {boolean} [data.tls_enabled=true] 是否启用TLS
   * @param {string} [data.cert_path] 证书路径
   * @param {string} [data.key_path] 私钥路径
   * @returns {Promise} 安装结果
   */
  install(data = {}) {
    return request({
      url: '/install',
      method: 'post',
      data,
      timeout: 60000 // 安装可能需要较长时间
    })
  }
}
