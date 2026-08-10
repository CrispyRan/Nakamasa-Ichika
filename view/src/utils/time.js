/**
 * 时间工具函数
 */
import dayjs from 'dayjs'

const time = {
  /**
   * 获取当前 Unix 时间戳（秒）
   */
  get() {
    return Math.floor(Date.now() / 1000)
  },

  /**
   * 将 Unix 时间戳转换为日期字符串
   * @param {number} timestamp - Unix 时间戳（秒）
   * @param {boolean} showTime - 是否显示时间
   * @returns {string} 格式化的日期字符串
   */
  toDate(timestamp, showTime = false) {
    if (!timestamp || timestamp <= 0) {
      return '-'
    }
    const format = showTime ? 'YYYY-MM-DD HH:mm:ss' : 'YYYY-MM-DD HH:mm'
    // 支持秒和毫秒两种时间戳
    const ts = timestamp.toString().length === 10 ? timestamp * 1000 : timestamp
    return dayjs(ts).format(format)
  },

  /**
   * 将时间戳转换为毫秒
   * @param {number} timestamp - Unix 时间戳（秒或毫秒）
   * @returns {number} 毫秒时间戳
   */
  toMillis(timestamp) {
    if (!timestamp) return 0
    return timestamp.toString().length === 10 ? timestamp * 1000 : timestamp
  },

  /**
   * 将毫秒时间戳转换为秒
   * @param {number} timestamp - 毫秒时间戳
   * @returns {number} 秒时间戳
   */
  toSeconds(timestamp) {
    if (!timestamp) return 0
    return timestamp.toString().length === 13 ? Math.floor(timestamp / 1000) : timestamp
  }
}

export default time
