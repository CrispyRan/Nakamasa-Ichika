<template>
  <a-spin :loading="loading" style="width: 100%">
    <a-card class="general-card" :header-style="{ paddingBottom: '16px' }">
      <template #title>内容时段分析</template>
      <sa-chart style="width: 100%; height: 370px" :options="chartOption" />
    </a-card>
  </a-spin>
</template>

<script setup>
import { ref } from 'vue'
import useLoading from '@/hooks/loading'
import { queryContentPeriodAnalysis } from '@/api/system/visualization'
import useChartOption from '@/hooks/chart-option'

const { loading, setLoading } = useLoading(true)
const xAxis = ref([])
const textChartsData = ref([])
const imgChartsData = ref([])
const videoChartsData = ref([])

const { chartOption } = useChartOption((isDark) => ({
  grid: { left: '40', right: 0, top: '20', bottom: '100' },
  legend: {
    bottom: 0,
    icon: 'circle',
    textStyle: { color: isDark ? '#A6ADB5' : '#4E5969' }
  },
  xAxis: {
    type: 'category',
    data: xAxis.value,
    boundaryGap: false,
    axisLine: { lineStyle: { color: isDark ? '#3f3f3f' : '#A9AEB8' } },
    axisTick: {
      show: true,
      alignWithLabel: true,
      lineStyle: { color: '#86909C' },
      interval: (idx) => idx !== 0 && idx !== xAxis.value.length - 1
    },
    axisLabel: {
      color: '#86909C',
      formatter: (value, idx) => (idx === 0 || idx === xAxis.value.length - 1) ? '' : `${value}`
    }
  },
  yAxis: {
    type: 'value',
    axisLabel: { color: '#86909C', formatter: '{value}%' },
    splitLine: { lineStyle: { color: isDark ? '#3F3F3F' : '#E5E6EB' } }
  },
  tooltip: {
    show: true,
    trigger: 'axis',
    className: 'echarts-tooltip-diy'
  },
  series: [
    {
      name: '纯文本',
      data: textChartsData.value,
      type: 'line',
      smooth: true,
      showSymbol: false,
      color: isDark ? '#3D72F6' : '#246EFF',
      symbol: 'circle',
      symbolSize: 10,
      emphasis: { focus: 'series', itemStyle: { borderWidth: 2, borderColor: '#E0E3FF' } }
    },
    {
      name: '图文类',
      data: imgChartsData.value,
      type: 'line',
      smooth: true,
      showSymbol: false,
      color: isDark ? '#A079DC' : '#00B2FF',
      symbol: 'circle',
      symbolSize: 10,
      emphasis: { focus: 'series', itemStyle: { borderWidth: 2, borderColor: '#E2F2FF' } }
    },
    {
      name: '视频类',
      data: videoChartsData.value,
      type: 'line',
      smooth: true,
      showSymbol: false,
      color: isDark ? '#6CAAF5' : '#81E2FF',
      symbol: 'circle',
      symbolSize: 10,
      emphasis: { focus: 'series', itemStyle: { borderWidth: 2, borderColor: '#D9F6FF' } }
    }
  ],
  dataZoom: [
    {
      bottom: 40,
      type: 'slider',
      left: 40,
      right: 14,
      height: 14,
      borderColor: 'transparent',
      handleIcon: 'M10,10 L15,5 L15,15 Z',
      handleSize: '20',
      handleStyle: { shadowColor: 'rgba(0, 0, 0, 0.2)', shadowBlur: 4 },
      brushSelect: false,
      backgroundColor: isDark ? '#313132' : '#F2F3F5'
    },
    { type: 'inside', start: 0, end: 100, zoomOnMouseWheel: false }
  ]
}))

const fetchData = async () => {
  setLoading(true)
  try {
    const res = await queryContentPeriodAnalysis()
    if (res.code === 200 && res.data) {
      xAxis.value = res.data.xAxis
      res.data.data.forEach((el) => {
        if (el.name === '纯文本') {
          textChartsData.value = el.value
        } else if (el.name === '图文类') {
          imgChartsData.value = el.value
        } else {
          videoChartsData.value = el.value
        }
      })
    }
  } catch (err) {
    // 模拟数据
    xAxis.value = Array.from({ length: 24 }, (_, i) => `${i}:00`)
    textChartsData.value = Array.from({ length: 24 }, () => Math.floor(Math.random() * 50) + 10)
    imgChartsData.value = Array.from({ length: 24 }, () => Math.floor(Math.random() * 40) + 20)
    videoChartsData.value = Array.from({ length: 24 }, () => Math.floor(Math.random() * 30) + 5)
  } finally {
    setLoading(false)
  }
}

fetchData()
</script>

<style scoped lang="less">
.chart-box {
  width: 100%;
  height: 230px;
}
</style>
