import { request } from '@/utils/request.js'

/**
 * 数据可视化 API
 */

// 舆情分析数据
export function queryPublicOpinionAnalysis(data) {
  return request({
    url: '/api/public-opinion-analysis',
    method: 'post',
    data
  })
}

// 内容时段分析
export function queryContentPeriodAnalysis() {
  return request({
    url: '/api/content-period-analysis',
    method: 'post'
  })
}

// 内容发布比例
export function queryContentPublish() {
  return request({
    url: '/api/content-publish',
    method: 'get'
  })
}

// 热门作者榜单
export function queryPopularAuthor() {
  return request({
    url: '/api/popular-author/list',
    method: 'get'
  })
}

// 环比增长数据
export function queryDataChainGrowth(data) {
  return request({
    url: '/api/data-chain-growth',
    method: 'post',
    data
  })
}

// 数据总览
export function queryDataOverview() {
  return request({
    url: '/api/data-overview',
    method: 'post'
  })
}

export default {
  queryPublicOpinionAnalysis,
  queryContentPeriodAnalysis,
  queryContentPublish,
  queryPopularAuthor,
  queryDataChainGrowth,
  queryDataOverview
}
