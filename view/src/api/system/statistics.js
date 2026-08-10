import { request } from '@/utils/request.js'

/**
 * 数据统计 API
 * 后端响应格式:
 * {
 *   user: { count, onLine, onLine_token, sign_in, sign_in_yesterday, census },
 *   order: { count, money_sum, today_money, yesterday_money, today_count, yesterday_count, today_success_rate, census },
 *   kami: { count, use_count, census }
 * }
 */
export default {
  /**
   * 获取统计数据
   */
  get() {
    return request({
      url: '/admin/statistics/get',
      method: 'get'
    })
  },
  
  /**
   * 获取格式化后的统计数据（用于前端展示）
   * 将后端嵌套结构转换为扁平结构
   */
  async getFormatted() {
    const res = await this.get()
    if (res.code === 200 && res.data) {
      const { user, order, kami } = res.data
      return {
        code: 200,
        data: {
          userCount: user?.count || 0,
          onlineCount: user?.onLine || 0,
          signInToday: user?.sign_in || 0,
          signInYesterday: user?.sign_in_yesterday || 0,
          userCensus: user?.census || [],
          
          orderCount: order?.count || 0,
          totalAmount: order?.money_sum || 0,
          todayAmount: order?.today_money || 0,
          yesterdayAmount: order?.yesterday_money || 0,
          todayOrderCount: order?.today_count || 0,
          yesterdayOrderCount: order?.yesterday_count || 0,
          todaySuccessRate: order?.today_success_rate || 0,
          orderCensus: order?.census || [],
          
          kamiCount: kami?.count || 0,
          kamiUsedCount: kami?.use_count || 0,
          kamiCensus: kami?.census || []
        }
      }
    }
    return res
  }
}
