<template>
  <div class="statistics-page">
    <!-- 图片轮播区域 -->
    <a-row :gutter="[16, 16]">
      <a-col :xs="24" :sm="24" :md="16" :lg="16" :xl="16">
        <sa-carousel
          :images="carouselImages"
          height="180px"
          @click="handleCarouselClick"
        />
      </a-col>
      <a-col :xs="24" :sm="24" :md="8" :lg="8" :xl="8">
        <a-card title="系统概览" :bordered="false" class="overview-card">
          <a-descriptions :column="1" bordered size="small">
            <a-descriptions-item label="在线用户">
              <a-badge :count="stats.onlineCount" :dot="false" :max-count="99999">
                <span class="online-count">{{ stats.onlineCount }}</span>
              </a-badge>
            </a-descriptions-item>
            <a-descriptions-item label="今日签到">
              {{ stats.signInToday }} 人
            </a-descriptions-item>
            <a-descriptions-item label="订单成功率">
              <a-progress
                :percent="stats.todaySuccessRate"
                :status="stats.todaySuccessRate >= 90 ? 'success' : 'warning'"
                size="small"
              />
            </a-descriptions-item>
            <a-descriptions-item label="卡密使用率">
              {{ kamiUsageRate }}%
            </a-descriptions-item>
          </a-descriptions>
        </a-card>
      </a-col>
    </a-row>

    <!-- 数据卡片 - 带迷你图表 -->
    <div class="chart-cards-row">
      <div v-for="card in chartCards" :key="card.title" class="chart-card-item">
        <sa-chart-card
          :title="card.title"
          :value="card.value"
          :precision="card.precision || 0"
          :growth="card.growth"
          :chart-type="card.chartType || 'bar'"
          :chart-data="card.chartData"
          :colors="card.colors || ['#165DFF']"
          :loading="loading"
        >
          <template #prefix>
            <component :is="card.icon" />
          </template>
          <template v-if="card.suffix" #suffix>{{ card.suffix }}</template>
        </sa-chart-card>
      </div>
    </div>

    <!-- 主图表区域 -->
    <a-row :gutter="[16, 16]" class="chart-section">
      <a-col :xs="24" :sm="24" :md="12" :lg="12" :xl="12">
        <a-card title="用户增长趋势" :bordered="false" :loading="loading">
          <template #extra>
            <a-radio-group v-model="userChartType" type="button" size="small">
              <a-radio value="line">折线</a-radio>
              <a-radio value="bar">柱状</a-radio>
            </a-radio-group>
          </template>
          <sa-chart :options="userChartOption" :height="chartHeight" />
        </a-card>
      </a-col>
      <a-col :xs="24" :sm="24" :md="12" :lg="12" :xl="12">
        <a-card title="订单金额趋势" :bordered="false" :loading="loading">
          <template #extra>
            <a-radio-group v-model="orderChartType" type="button" size="small">
              <a-radio value="line">折线</a-radio>
              <a-radio value="bar">柱状</a-radio>
            </a-radio-group>
          </template>
          <sa-chart :options="orderChartOption" :height="chartHeight" />
        </a-card>
      </a-col>
    </a-row>

    <!-- 饼图区域 -->
    <a-row :gutter="[16, 16]" class="chart-section">
      <a-col :xs="24" :sm="8" :md="8" :lg="8" :xl="8">
        <a-card title="用户分布" :bordered="false" :loading="loading">
          <sa-chart :options="userDistributionOption" :height="pieChartHeight" />
        </a-card>
      </a-col>
      <a-col :xs="24" :sm="8" :md="8" :lg="8" :xl="8">
        <a-card title="订单类型" :bordered="false" :loading="loading">
          <sa-chart :options="orderTypeOption" :height="pieChartHeight" />
        </a-card>
      </a-col>
      <a-col :xs="24" :sm="8" :md="8" :lg="8" :xl="8">
        <a-card title="卡密状态" :bordered="false" :loading="loading">
          <sa-chart :options="kamiStatusOption" :height="pieChartHeight" />
        </a-card>
      </a-col>
    </a-row>

    <!-- 地图区域 - 世界地图与中国地图 -->
    <a-row :gutter="[16, 16]" class="chart-section">
      <!-- 世界地图 -->
      <a-col :xs="24" :sm="24" :md="12" :lg="12" :xl="12">
        <a-card title="全球用户分布" :bordered="false">
          <div class="map-container-compact">
            <div class="map-chart">
              <sa-world-map 
                :data="worldMapData" 
                :theme="isDarkMode ? 'dark' : 'light'"
                height="300px"
              />
            </div>
            <div class="map-legend-compact">
              <div class="legend-title">地区 TOP 5</div>
              <div class="legend-list">
                <div v-for="(item, idx) in regionRanking" :key="item.name" class="legend-item">
                  <span class="rank" :class="'rank-' + (idx + 1)">{{ idx + 1 }}</span>
                  <span class="name">{{ item.name }}</span>
                  <span class="value">{{ item.value.toLocaleString() }}</span>
                  <span class="percent">{{ item.percent }}%</span>
                </div>
              </div>
            </div>
          </div>
        </a-card>
      </a-col>
      <!-- 中国地图 -->
      <a-col :xs="24" :sm="24" :md="12" :lg="12" :xl="12">
        <a-card title="中国用户分布" :bordered="false">
          <div class="map-container-compact">
            <div class="map-chart">
              <sa-china-map 
                :data="chinaMapData" 
                :theme="isDarkMode ? 'dark' : 'light'"
                height="300px"
              />
            </div>
            <div class="map-legend-compact">
              <div class="legend-title">省份 TOP 5</div>
              <div class="legend-list">
                <div v-for="(item, idx) in provinceRanking" :key="item.name" class="legend-item">
                  <span class="rank" :class="'rank-' + (idx + 1)">{{ idx + 1 }}</span>
                  <span class="name">{{ item.name }}</span>
                  <span class="value">{{ item.value.toLocaleString() }}</span>
                  <span class="percent">{{ item.percent }}%</span>
                </div>
              </div>
            </div>
          </div>
        </a-card>
      </a-col>
    </a-row>
  </div>
</template>

<script setup>
import { ref, reactive, computed, onMounted, onUnmounted } from 'vue'
import useChartOption from '@/hooks/chart-option.js'
import useThemes from '@/hooks/themes'
import statisticsApi from '@/api/system/statistics'

const loading = ref(true)
const userChartType = ref('line')
const orderChartType = ref('line')
const { isDark } = useThemes()

// 响应式图表高度
const chartHeight = ref('280px')
const pieChartHeight = ref('240px')

// 深色模式
const isDarkMode = computed(() => isDark.value)

// 地区排名数据
const regionRanking = ref([
  { name: '中国', value: 8562, percent: 68.0 },
  { name: '美国', value: 1256, percent: 10.0 },
  { name: '日本', value: 892, percent: 7.1 },
  { name: '韩国', value: 654, percent: 5.2 },
  { name: '德国', value: 423, percent: 3.4 }
])

// 世界地图数据（ECharts 国家名称）
const worldMapData = ref([
  { name: 'China', value: 8562 },
  { name: 'United States', value: 1256 },
  { name: 'Japan', value: 892 },
  { name: 'South Korea', value: 654 },
  { name: 'Germany', value: 423 },
  { name: 'United Kingdom', value: 312 },
  { name: 'France', value: 287 },
  { name: 'Singapore', value: 198 },
  { name: 'Australia', value: 156 },
  { name: 'Canada', value: 134 },
  { name: 'Russia', value: 89 },
  { name: 'Brazil', value: 67 },
  { name: 'India', value: 56 },
  { name: 'Italy', value: 45 },
  { name: 'Spain', value: 38 }
])

// 中国省份排名数据
const provinceRanking = ref([
  { name: '广东', value: 1523, percent: 17.8 },
  { name: '江苏', value: 1089, percent: 12.7 },
  { name: '浙江', value: 956, percent: 11.2 },
  { name: '北京', value: 812, percent: 9.5 },
  { name: '上海', value: 734, percent: 8.6 }
])

// 中国地图数据（省份名称）
const chinaMapData = ref([
  { name: '广东', value: 1523 },
  { name: '江苏', value: 1089 },
  { name: '浙江', value: 956 },
  { name: '北京', value: 812 },
  { name: '上海', value: 734 },
  { name: '山东', value: 623 },
  { name: '四川', value: 567 },
  { name: '河南', value: 489 },
  { name: '湖北', value: 423 },
  { name: '福建', value: 389 },
  { name: '湖南', value: 356 },
  { name: '河北', value: 312 },
  { name: '安徽', value: 287 },
  { name: '陕西', value: 256 },
  { name: '辽宁', value: 234 },
  { name: '重庆', value: 212 },
  { name: '天津', value: 189 },
  { name: '云南', value: 156 },
  { name: '广西', value: 145 },
  { name: '江西', value: 134 },
  { name: '山西', value: 123 },
  { name: '贵州', value: 112 },
  { name: '黑龙江', value: 98 },
  { name: '吉林', value: 87 },
  { name: '甘肃', value: 76 },
  { name: '海南', value: 65 },
  { name: '内蒙古', value: 54 },
  { name: '新疆', value: 43 },
  { name: '宁夏', value: 32 },
  { name: '青海', value: 21 },
  { name: '西藏', value: 12 }
])

const stats = reactive({
  userCount: 0,
  todayUser: 0,
  onlineCount: 0,
  signInToday: 0,
  signInYesterday: 0,
  userCensus: [],
  
  orderCount: 0,
  totalAmount: 0,
  todayAmount: 0,
  yesterdayAmount: 0,
  todayOrderCount: 0,
  yesterdayOrderCount: 0,
  todaySuccessRate: 0,
  orderCensus: [],
  
  kamiCount: 0,
  kamiUsedCount: 0,
  kamiCensus: []
})

// 轮播图数据
const carouselImages = ref([
  { src: 'https://p3-armor.byteimg.com/tos-cn-i-49unhts6dw/5cc3cd1d994b7ef9db6a1f619a22addd.jpg~tplv-49unhts6dw-image.image', title: '数据统计', description: '实时监控业务数据' },
  { src: 'https://p3-armor.byteimg.com/tos-cn-i-49unhts6dw/f256cbcc287139e191fecea9d255a1f0.jpg~tplv-49unhts6dw-image.image', title: '用户增长', description: '用户持续增长中' },
  { src: 'https://p3-armor.byteimg.com/tos-cn-i-49unhts6dw/b557ff0cd44146a2e471b477af2f30d0.jpg~tplv-49unhts6dw-image.image', title: '订单趋势', description: '订单量稳步上升' }
])

// 计算属性
const kamiUsageRate = computed(() => {
  if (stats.kamiCount === 0) return 0
  return ((stats.kamiUsedCount / stats.kamiCount) * 100).toFixed(1)
})

// 数据卡片配置
const chartCards = computed(() => [
  {
    title: '用户总数',
    value: stats.userCount,
    growth: stats.signInToday > 0 && stats.signInYesterday > 0 
      ? (((stats.signInToday - stats.signInYesterday) / stats.signInYesterday) * 100).toFixed(1)
      : 0,
    chartType: 'line',
    chartData: stats.userCensus.slice(-7),
    colors: ['#165DFF'],
    icon: 'icon-user'
  },
  {
    title: '订单总数',
    value: stats.orderCount,
    growth: stats.todayOrderCount > 0 && stats.yesterdayOrderCount > 0
      ? (((stats.todayOrderCount - stats.yesterdayOrderCount) / stats.yesterdayOrderCount) * 100).toFixed(1)
      : 0,
    chartType: 'bar',
    chartData: stats.orderCensus.slice(-7),
    colors: ['#14C9C9'],
    icon: 'icon-file'
  },
  {
    title: '累计金额',
    value: stats.totalAmount,
    precision: 2,
    growth: stats.todayAmount > 0 && stats.yesterdayAmount > 0
      ? (((stats.todayAmount - stats.yesterdayAmount) / stats.yesterdayAmount) * 100).toFixed(1)
      : 0,
    chartType: 'line',
    chartData: stats.orderCensus.slice(-7),
    colors: ['#F7BA1E'],
    icon: 'icon-heart',
    suffix: '元'
  },
  {
    title: '卡密总数',
    value: stats.kamiCount,
    growth: 0,
    chartType: 'pie',
    chartData: [
      { name: '已使用', value: stats.kamiUsedCount },
      { name: '未使用', value: stats.kamiCount - stats.kamiUsedCount }
    ],
    colors: ['#722ED1', '#E5E6EB'],
    icon: 'icon-code-square'
  }
])

// 用户趋势图表配置
const { chartOption: userChartOption } = useChartOption((isDark) => ({
  tooltip: {
    trigger: 'axis',
    backgroundColor: isDark ? '#1D1D1D' : '#fff',
    borderColor: isDark ? '#3D3D3D' : '#E5E6EB',
    textStyle: { color: isDark ? '#fff' : '#1D2129', fontSize: 12 }
  },
  grid: {
    left: '2%',
    right: '2%',
    bottom: '8%',
    top: '8%',
    containLabel: true
  },
  xAxis: {
    type: 'category',
    boundaryGap: userChartType.value === 'bar',
    data: ['周一', '周二', '周三', '周四', '周五', '周六', '周日'],
    axisLine: { lineStyle: { color: isDark ? '#3D3D3D' : '#E5E6EB' } },
    axisLabel: { color: isDark ? '#A6ADB5' : '#86909C', fontSize: 11 }
  },
  yAxis: {
    type: 'value',
    splitLine: { lineStyle: { color: isDark ? '#3D3D3D' : '#E5E6EB' } },
    axisLabel: { color: isDark ? '#A6ADB5' : '#86909C', fontSize: 11 }
  },
  series: [{
    name: '用户数',
    type: userChartType.value,
    smooth: true,
    data: stats.userCensus.length >= 7 ? stats.userCensus.slice(-7) : [120, 150, 180, 200, 160, 220, 280],
    itemStyle: { color: '#165DFF' },
    areaStyle: userChartType.value === 'line' ? {
      color: {
        type: 'linear',
        x: 0, y: 0, x2: 0, y2: 1,
        colorStops: [
          { offset: 0, color: 'rgba(22, 93, 255, 0.3)' },
          { offset: 1, color: 'rgba(22, 93, 255, 0.05)' }
        ]
      }
    } : undefined
  }]
}))

// 订单趋势图表配置
const { chartOption: orderChartOption } = useChartOption((isDark) => ({
  tooltip: {
    trigger: 'axis',
    backgroundColor: isDark ? '#1D1D1D' : '#fff',
    borderColor: isDark ? '#3D3D3D' : '#E5E6EB',
    textStyle: { color: isDark ? '#fff' : '#1D2129', fontSize: 12 },
    formatter: (params) => {
      const data = params[0]
      return `${data.name}<br/>金额: ¥${data.value.toLocaleString()}`
    }
  },
  grid: {
    left: '2%',
    right: '2%',
    bottom: '8%',
    top: '8%',
    containLabel: true
  },
  xAxis: {
    type: 'category',
    boundaryGap: orderChartType.value === 'bar',
    data: ['周一', '周二', '周三', '周四', '周五', '周六', '周日'],
    axisLine: { lineStyle: { color: isDark ? '#3D3D3D' : '#E5E6EB' } },
    axisLabel: { color: isDark ? '#A6ADB5' : '#86909C', fontSize: 11 }
  },
  yAxis: {
    type: 'value',
    splitLine: { lineStyle: { color: isDark ? '#3D3D3D' : '#E5E6EB' } },
    axisLabel: {
      color: isDark ? '#A6ADB5' : '#86909C',
      fontSize: 11,
      formatter: '¥{value}'
    }
  },
  series: [{
    name: '金额',
    type: orderChartType.value,
    smooth: true,
    data: stats.orderCensus.length >= 7 ? stats.orderCensus.slice(-7) : [1200, 1800, 1500, 2200, 1900, 2500, 3200],
    itemStyle: { color: '#14C9C9' },
    areaStyle: orderChartType.value === 'line' ? {
      color: {
        type: 'linear',
        x: 0, y: 0, x2: 0, y2: 1,
        colorStops: [
          { offset: 0, color: 'rgba(20, 201, 201, 0.3)' },
          { offset: 1, color: 'rgba(20, 201, 201, 0.05)' }
        ]
      }
    } : undefined
  }]
}))

// 用户分布饼图
const { chartOption: userDistributionOption } = useChartOption((isDark) => ({
  tooltip: { trigger: 'item', textStyle: { fontSize: 12 } },
  legend: {
    orient: 'vertical',
    left: 'left',
    top: 'center',
    textStyle: { color: isDark ? '#A6ADB5' : '#4E5969', fontSize: 11 }
  },
  series: [{
    type: 'pie',
    radius: ['35%', '60%'],
    center: ['60%', '50%'],
    avoidLabelOverlap: false,
    itemStyle: { borderRadius: 4, borderColor: isDark ? '#1D1D1D' : '#fff', borderWidth: 2 },
    label: { show: false },
    emphasis: { label: { show: true, fontSize: 12, fontWeight: 'bold' } },
    labelLine: { show: false },
    data: [
      { value: 1048, name: '新用户', itemStyle: { color: '#165DFF' } },
      { value: 735, name: '活跃用户', itemStyle: { color: '#14C9C9' } },
      { value: 580, name: '回流用户', itemStyle: { color: '#F7BA1E' } },
      { value: 484, name: '沉默用户', itemStyle: { color: '#722ED1' } }
    ]
  }]
}))

// 订单类型饼图
const { chartOption: orderTypeOption } = useChartOption((isDark) => ({
  tooltip: { trigger: 'item', textStyle: { fontSize: 12 } },
  legend: {
    orient: 'vertical',
    left: 'left',
    top: 'center',
    textStyle: { color: isDark ? '#A6ADB5' : '#4E5969', fontSize: 11 }
  },
  series: [{
    type: 'pie',
    radius: ['35%', '60%'],
    center: ['60%', '50%'],
    avoidLabelOverlap: false,
    itemStyle: { borderRadius: 4, borderColor: isDark ? '#1D1D1D' : '#fff', borderWidth: 2 },
    label: { show: false },
    emphasis: { label: { show: true, fontSize: 12, fontWeight: 'bold' } },
    labelLine: { show: false },
    data: [
      { value: 2340, name: '实物订单', itemStyle: { color: '#165DFF' } },
      { value: 1350, name: '虚拟订单', itemStyle: { color: '#14C9C9' } },
      { value: 958, name: '充值订单', itemStyle: { color: '#F7BA1E' } }
    ]
  }]
}))

// 卡密状态饼图
const { chartOption: kamiStatusOption } = useChartOption((isDark) => ({
  tooltip: { trigger: 'item', textStyle: { fontSize: 12 } },
  legend: {
    orient: 'vertical',
    left: 'left',
    top: 'center',
    textStyle: { color: isDark ? '#A6ADB5' : '#4E5969', fontSize: 11 }
  },
  series: [{
    type: 'pie',
    radius: ['35%', '60%'],
    center: ['60%', '50%'],
    avoidLabelOverlap: false,
    itemStyle: { borderRadius: 4, borderColor: isDark ? '#1D1D1D' : '#fff', borderWidth: 2 },
    label: { show: false },
    emphasis: { label: { show: true, fontSize: 12, fontWeight: 'bold' } },
    labelLine: { show: false },
    data: [
      { value: stats.kamiUsedCount, name: '已使用', itemStyle: { color: '#165DFF' } },
      { value: stats.kamiCount - stats.kamiUsedCount, name: '未使用', itemStyle: { color: '#E5E6EB' } }
    ]
  }]
}))

// 响应式调整
const updateChartHeight = () => {
  const width = window.innerWidth
  if (width < 576) {
    chartHeight.value = '220px'
    pieChartHeight.value = '200px'
  } else if (width < 768) {
    chartHeight.value = '250px'
    pieChartHeight.value = '220px'
  } else {
    chartHeight.value = '280px'
    pieChartHeight.value = '240px'
  }
}

// 加载统计数据
const loadStats = async () => {
  loading.value = true
  try {
    const res = await statisticsApi.getFormatted()
    if (res.code === 200 && res.data) {
      Object.assign(stats, res.data)
    }
  } catch (e) {
    // 使用模拟数据
    Object.assign(stats, {
      userCount: 12580,
      todayUser: 126,
      onlineCount: 356,
      signInToday: 126,
      signInYesterday: 98,
      userCensus: [120, 150, 180, 200, 160, 220, 280],
      
      orderCount: 8960,
      totalAmount: 158900,
      todayAmount: 1580,
      yesterdayAmount: 1320,
      todayOrderCount: 89,
      yesterdayOrderCount: 76,
      todaySuccessRate: 96,
      orderCensus: [1200, 1800, 1500, 2200, 1900, 2500, 3200],
      
      kamiCount: 5200,
      kamiUsedCount: 3800,
      kamiCensus: [100, 150, 200, 180, 220, 280, 300]
    })
  } finally {
    loading.value = false
  }
}

const handleCarouselClick = ({ item, index }) => {
  console.log('Carousel clicked:', item, index)
}

onMounted(() => {
  loadStats()
  updateChartHeight()
  window.addEventListener('resize', updateChartHeight)
})

onUnmounted(() => {
  window.removeEventListener('resize', updateChartHeight)
})
</script>

<script>
export default { name: 'Statistics' }
</script>

<style scoped lang="less">
.statistics-page {
  padding: 16px;
  
  @media (min-width: 576px) {
    padding: 20px;
  }
}

// 统一的图表区域间距
.chart-section {
  margin-top: 32px;
  
  @media (min-width: 576px) {
    margin-top: 40px;
  }
}

.overview-card {
  height: 180px;
  
  @media (min-width: 576px) {
    height: 200px;
  }
}

.online-count {
  font-size: 13px;
  font-weight: 500;
  padding: 0 6px;
  
  @media (min-width: 576px) {
    font-size: 14px;
    padding: 0 8px;
  }
}

// 数据卡片行 - 使用 flex 布局确保等高
.chart-cards-row {
  display: flex;
  flex-wrap: wrap;
  gap: 20px;
  margin-top: 32px;
  
  @media (min-width: 576px) {
    gap: 24px;
    margin-top: 40px;
  }
}

.chart-card-item {
  width: calc(50% - 10px);
  
  @media (min-width: 768px) {
    width: calc(25% - 18px);
  }
  
  :deep(.chart-card) {
    height: 100%;
    min-height: 110px;
    border: 1px solid var(--color-border);
    box-shadow: 0 1px 2px rgba(0, 0, 0, 0.05);
    
    @media (max-width: 576px) {
      min-height: 100px;
    }
  }
}

// 图表区域卡片样式
.chart-section {
  :deep(.arco-card) {
    border: 1px solid var(--color-border);
    box-shadow: 0 1px 2px rgba(0, 0, 0, 0.05);
  }
}

// 地图容器 - 紧凑型（用于并排显示）
.map-container-compact {
  display: flex;
  flex-direction: column;
  gap: 12px;
  
  @media (min-width: 768px) {
    flex-direction: row;
    height: 320px;
  }
}

.map-chart {
  flex: 1;
  min-height: 280px;
  
  @media (min-width: 768px) {
    min-height: auto;
  }
}

.map-legend-compact {
  width: 100%;
  
  @media (min-width: 768px) {
    width: 180px;
    flex-shrink: 0;
  }
}

.legend-title {
  font-size: 13px;
  font-weight: 500;
  color: var(--color-text-1);
  margin-bottom: 12px;
  padding-bottom: 8px;
  border-bottom: 1px solid var(--color-border);
}

.legend-list {
  display: flex;
  flex-direction: column;
  gap: 8px;
}

.legend-item {
  display: flex;
  align-items: center;
  gap: 8px;
  padding: 6px 8px;
  background: var(--color-fill-1);
  border-radius: 4px;
  
  .rank {
    width: 20px;
    height: 20px;
    display: flex;
    align-items: center;
    justify-content: center;
    border-radius: 4px;
    font-size: 11px;
    font-weight: 600;
    background: var(--color-fill-3);
    color: var(--color-text-3);
    
    &.rank-1 { background: #165DFF; color: #fff; }
    &.rank-2 { background: #14C9C9; color: #fff; }
    &.rank-3 { background: #F7BA1E; color: #fff; }
  }
  
  .name {
    flex: 1;
    font-size: 12px;
    color: var(--color-text-1);
  }
  
  .value {
    font-size: 12px;
    font-weight: 500;
    color: var(--color-text-1);
  }
  
  .percent {
    font-size: 11px;
    color: var(--color-text-3);
    width: 36px;
    text-align: right;
  }
}

:deep(.arco-card) {
  .arco-card-header {
    padding: 12px 16px;
    font-size: 14px;
    
    @media (max-width: 576px) {
      padding: 10px 12px;
      font-size: 13px;
    }
  }
  
  .arco-card-body {
    padding: 12px 16px;
    
    @media (max-width: 576px) {
      padding: 10px 12px;
    }
  }
}

:deep(.arco-descriptions-item-label) {
  font-size: 12px;
  
  @media (min-width: 576px) {
    font-size: 13px;
  }
}

:deep(.arco-descriptions-item-value) {
  font-size: 12px;
  
  @media (min-width: 576px) {
    font-size: 13px;
  }
}

:deep(.arco-radio-group) {
  .arco-radio-button {
    font-size: 11px;
    padding: 0 8px;
    
    @media (min-width: 576px) {
      font-size: 12px;
      padding: 0 10px;
    }
  }
}
</style>
