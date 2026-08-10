<template>
  <a-spin :loading="loading" style="width: 100%">
    <a-card class="general-card" :header-style="{ paddingBottom: '14px' }">
      <template #title>内容发布比例</template>
      <template #extra>
        <a-link>查看更多</a-link>
      </template>
      <sa-chart style="width: 100%; height: 347px" :options="chartOption" />
    </a-card>
  </a-spin>
</template>

<script setup>
import { ref } from 'vue'
import useLoading from '@/hooks/loading'
import { queryContentPublish } from '@/api/system/visualization'
import useChartOption from '@/hooks/chart-option'

const { loading, setLoading } = useLoading(true)
const xAxis = ref([])
const textChartsData = ref([])
const imgChartsData = ref([])
const videoChartsData = ref([])

const { chartOption } = useChartOption((isDark) => ({
  grid: { left: '4%', right: 0, top: '20', bottom: '60' },
  legend: {
    bottom: 0,
    icon: 'circle',
    textStyle: { color: isDark ? '#A6ADB5' : '#4E5969' }
  },
  xAxis: {
    type: 'category',
    data: xAxis.value,
    axisLine: { lineStyle: { color: isDark ? '#3f3f3f' : '#A9AEB8' } },
    axisTick: { show: true, alignWithLabel: true, lineStyle: { color: '#86909C' } },
    axisLabel: { color: '#86909C' }
  },
  yAxis: {
    type: 'value',
    axisLabel: {
      color: '#86909C',
      formatter: (value, idx) => idx === 0 ? `${value}` : `${value / 1000}k`
    },
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
      stack: 'one',
      type: 'bar',
      barWidth: 16,
      color: isDark ? '#4A7FF7' : '#246EFF'
    },
    {
      name: '图文类',
      data: imgChartsData.value,
      stack: 'one',
      type: 'bar',
      color: isDark ? '#085FEF' : '#00B2FF'
    },
    {
      name: '视频类',
      data: videoChartsData.value,
      stack: 'one',
      type: 'bar',
      color: isDark ? '#01349F' : '#81E2FF',
      itemStyle: { borderRadius: 2 }
    }
  ]
}))

const fetchData = async () => {
  setLoading(true)
  try {
    const res = await queryContentPublish()
    if (res.code === 200 && res.data) {
      xAxis.value = res.data[0]?.x || []
      res.data.forEach((el) => {
        if (el.name === '纯文本') {
          textChartsData.value = el.y
        } else if (el.name === '图文类') {
          imgChartsData.value = el.y
        } else {
          videoChartsData.value = el.y
        }
      })
    }
  } catch (err) {
    // 模拟数据
    xAxis.value = ['周一', '周二', '周三', '周四', '周五', '周六', '周日']
    textChartsData.value = [320, 332, 301, 334, 390, 330, 320]
    imgChartsData.value = [120, 132, 101, 134, 90, 230, 210]
    videoChartsData.value = [150, 232, 201, 154, 190, 330, 410]
  } finally {
    setLoading(false)
  }
}

fetchData()
</script>

<style scoped lang="less"></style>
