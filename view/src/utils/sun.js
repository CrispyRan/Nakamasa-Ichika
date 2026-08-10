/**
 * 时间单位转换工具
 * 用于 VIP 时长的秒数与时间单位之间的转换
 */

// 时间单位到秒的转换系数
const UNIT_SECONDS = {
  s: 1,        // 秒
  i: 60,       // 分 (minute)
  h: 3600,     // 时 (hour)
  d: 86400,    // 天 (day)
}

/**
 * 从秒数转换到指定单位的值
 * @param {number} seconds - 总秒数
 * @param {number} returnType - 返回类型: 1=值, 2=单位
 * @returns {number|string} 值或单位
 */
export function parseVipTime(seconds, returnType = 1) {
  if (!seconds || seconds <= 0) {
    return returnType === 1 ? 0 : 'd'
  }
  
  // 永久会员
  if (seconds >= 9999999999) {
    return returnType === 1 ? 9999999999 : 's'
  }
  
  // 找到最合适的单位
  if (seconds % 86400 === 0 && seconds >= 86400) {
    // 天
    return returnType === 1 ? seconds / 86400 : 'd'
  } else if (seconds % 3600 === 0 && seconds >= 3600) {
    // 时
    return returnType === 1 ? seconds / 3600 : 'h'
  } else if (seconds % 60 === 0 && seconds >= 60) {
    // 分
    return returnType === 1 ? seconds / 60 : 'i'
  } else {
    // 秒
    return returnType === 1 ? seconds : 's'
  }
}

/**
 * 将值和单位转换为总秒数
 * @param {number} value - 数值
 * @param {string} unit - 单位 (s/i/h/d)
 * @returns {number} 总秒数
 */
export function toSeconds(value, unit) {
  const multiplier = UNIT_SECONDS[unit] || 1
  return Math.floor(value * multiplier)
}

/**
 * 格式化显示 VIP 时长
 * @param {number} seconds - 总秒数
 * @returns {string} 格式化后的字符串
 */
export function formatVipTime(seconds) {
  if (!seconds || seconds <= 0) return '0秒'
  if (seconds >= 9999999999) return '永久'
  
  const units = [
    { key: 'd', label: '天', seconds: 86400 },
    { key: 'h', label: '时', seconds: 3600 },
    { key: 'i', label: '分', seconds: 60 },
    { key: 's', label: '秒', seconds: 1 },
  ]
  
  for (const unit of units) {
    if (seconds % unit.seconds === 0 && seconds >= unit.seconds) {
      return `${seconds / unit.seconds}${unit.label}`
    }
  }
  
  return `${seconds}秒`
}

// 默认导出
export default {
  parseVipTime,
  toSeconds,
  formatVipTime,
  UNIT_SECONDS
}
