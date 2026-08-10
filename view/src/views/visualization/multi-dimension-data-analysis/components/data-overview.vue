<template>
  <a-spin :loading="loading" style="width: 100%">
    <a-card class="general-card" title="数据总览">
      <a-row justify="space-between">
        <a-col v-for="(item, idx) in renderData" :key="idx" :span="6">
          <a-statistic
            :title="item.title"
            :value="item.value"
            show-group-separator
            :value-from="0"
            animation
          >
            <template #prefix>
              <span class="statistic-prefix" :style="{ background: item.prefix.background }">
                <component :is="item.prefix.icon" :style="{ color: item.prefix.iconColor }" />
              </span>
            </template>
          </a-statistic>
        </a-col>
      </a-row>
      <sa-chart style="height: 328px; margin-top: 20px" :options="chartOption" />
    </a-card>
  </a-spin>
</template>

<script setup>
import { computed, ref } from 'vue'
import { queryDataOverview } from '@/api/system/visualization'
import useLoading from '@/hooks/loading'
import useThemes from '@/hooks/themes'
import useChartOption from '@/hooks/chart-option'

const { loading, setLoading } = useLoading(true)
const { isDark } = useThemes()

const renderData = computed(() => [
  {
    title: '内容生产量',
    value: 1902,
    prefix: {
      icon: 'icon-edit',
      background: isDark.value ? '#593E2F' : '#FFE4BA',
      iconColor: isDark.value ? '#F29A43' : '#F77234'
    }
  },
  {
    title: '内容点击量',
    value: 2445,
    prefix: {
      icon: 'icon-thumb-up',
      background: isDark.value ? '#3D5A62' : '#E8FFFB',
      iconColor: isDark.value ? '#6ED1CE' : '#33D1C9'
    }
  },
  {
    title: '内容曝光量',
    value: 3034,
    prefix: {
      icon: 'icon-heart',
      background: isDark.value ? '#354276' : '#E8F3FF',
      iconColor: isDark.value ? '#4A7FF7' : '#165DFF'
    }
  },
  {
    title: '活跃用户数',
    value: 1275,
    prefix: {
      icon: 'icon-user',
      background: isDark.value ? '#3F385E' : '#F5E8FF',
      iconColor: isDark.value ? '#8558D3' : '#722ED1'
    }
  }
])

const xAxis = ref([])
const contentProductionData = ref([])
const contentClickData = ref([])
const contentExposureData = ref([])
const activeUsersData = ref([])

const generateSeries = (name, lineColor, itemBorderColor, data) => ({
  name,
  data,
  stack: 'Total',
  type: 'line',
  smooth: true,
  symbol: 'circle',
  symbolSize: 10,
  itemStyle: { color: lineColor },
  emphasis: {
    focus: 'series',
    itemStyle: { color: lineColor, borderWidth: 2, borderColor: itemBorderColor }
  },
  lineStyle: { width: 2, color: lineColor },
  showSymbol: false,
  areaStyle: { opacity: 0.1, color: lineColor }
})

const { chartOption } = useChartOption((dark) => ({
  grid: { left: '2.6%', right: '4', top: '40', bottom: '40' },
  xAxis: {
    type: 'category',
    offset: 2,
    data: xAxis.value,
    boundaryGap: false,
    axisLabel: {
      color: '#4E5969',
      formatter: (value, idx) => (idx === 0 || idx === xAxis.value.length - 1) ? '' : `${value}`
    },
    axisLine: { show: false },
    axisTick: { show: false },
    splitLine: { show: false },
    axisPointer: { show: true, lineStyle: { color: '#23ADFF', width: 2 } }
  },
  yAxis: {
    type: 'value',
    axisLine: { show: false },
    axisLabel: {
      formatter: (value, idx) => idx === 0 ? String(value) : `${value / 1000}k`
    },
    splitLine: { lineStyle: { color: dark ? '#2E2E30' : '#F2F3F5' } }
  },
  tooltip: { trigger: 'axis', className: 'echarts-tooltip-diy' },
  graphic: {
    elements: [
      { type: 'text', left: '2.6%', bottom: '18', style: { text: '12.10', textAlign: 'center', fill: '#4E5969', fontSize: 12 } },
      { type: 'text', right: '0', bottom: '18', style: { text: '12.17', textAlign: 'center', fill: '#4E5969', fontSize: 12 } }
    ]
  },
  series: [
    generateSeries('内容生产量', '#722ED1', '#F5E8FF', contentProductionData.value),
    generateSeries('内容点击量', '#F77234', '#FFE4BA', contentClickData.value),
    generateSeries('内容曝光量', '#33D1C9', '#E8FFFB', contentExposureData.value),
    generateSeries('活跃用户数', '#3469FF', '#E8F3FF', activeUsersData.value)
  ]
}))

const fetchData = async () => {
  setLoading(true)
  try {
    const res = await queryDataOverview()
    if (res.code === 200 && res.data) {
      xAxis.value = res.data.xAxis
      res.data.data.forEach((el) => {
        if (el.name === '内容生产量') contentProductionData.value = el.value
        else if (el.name === '内容点击量') contentClickData.value = el.value
        else if (el.name === '内容曝光量') contentExposureData.value = el.value
        activeUsersData.value = el.value
      })
    }
  } catch (err) {
    // 模拟数据
    xAxis.value = ['12.10', '12.11', '12.12', '12.13', '12.14', '12.15', '12.16', '12.17']
    contentProductionData.value = [1200, 1400, 1100, 1600, 1300, 1500, 1700, 1902]
    contentClickData.value = [1800, 2000, 1600, 2200, 1900, 2100, 2300, 2445]
    contentExposureData.value = [2500, 2800, 2200, 3100, 2700, 2900, 3200, 3034]
    activeUsersData.value = [1000, 1100, 900, 1300, 1100, 1200, 1400, 1275]
  } finally {
    setLoading(false)
  }
}

fetchData()
</script>

<style scoped lang="less">
:deep(.arco-statistic) {
  .arco-statistic-title {
    color: rgb(var(--gray-10));
    font-weight: bold;
  }
  .arco-statistic-value {
    display: flex;
    align-items: center;
  }
}
.statistic-prefix {
  display: inline-block;
  width: 32px;
  height: 32px;
  margin-right: 8px;
  color: var(--color-white);
  font-size: 16px;
  line-height: 32px;
  text-align: center;
  vertical-align: middle;
  border-radius: 6px;
}
</style>
