<template>
  <a-spin :loading="loading" style="width: 100%">
    <a-card
      class="general-card"
      :title="title"
      :header-style="{ paddingBottom: '12px' }"
    >
      <div class="content">
        <a-statistic
          :value="count"
          :show-group-separator="true"
          :value-from="0"
          animation
        />
        <a-typography-text
          class="percent-text"
          :type="isUp ? 'danger' : 'success'"
        >
          {{ growth }}%
          <icon-arrow-rise v-if="isUp" />
          <icon-arrow-fall v-else />
        </a-typography-text>
      </div>
      <div class="chart">
        <sa-chart :options="chartOption" height="80px" />
      </div>
    </a-card>
  </a-spin>
</template>

<script setup>
import { computed, ref } from 'vue'
import useLoading from '@/hooks/loading'
import { queryDataChainGrowth } from '@/api/system/visualization'
import useChartOption from '@/hooks/chart-option'

const props = defineProps({
  title: { type: String, default: '' },
  quota: { type: String, default: '' },
  chartType: { type: String, default: '' }
})

const { loading, setLoading } = useLoading(true)
const count = ref(0)
const growth = ref(100)
const isUp = computed(() => growth.value > 50)
const chartData = ref([])

const { chartOption } = useChartOption(() => ({
  grid: { left: 0, right: 0, top: 0, bottom: 0 },
  xAxis: { type: 'category', show: false },
  yAxis: { show: false },
  tooltip: { show: true, trigger: 'axis', formatter: '{c}' },
  series: [{
    data: chartData.value,
    ...(props.chartType === 'bar'
      ? { type: 'bar', barWidth: 7, barGap: '0' }
      : { type: 'line', showSymbol: false, smooth: true, lineStyle: { color: '#4080FF' } })
  }]
}))

const fetchData = async (params) => {
  try {
    const res = await queryDataChainGrowth(params)
    if (res.code === 200 && res.data) {
      count.value = res.data.count
      growth.value = res.data.growth
      res.data.chartData?.data?.value?.forEach((el, idx) => {
        if (props.chartType === 'bar') {
          chartData.value.push({
            value: el,
            itemStyle: { color: idx % 2 ? '#468DFF' : '#86DF6C' }
          })
        } else {
          chartData.value.push(el)
        }
      })
    }
  } catch (err) {
    // 模拟数据
    count.value = Math.floor(Math.random() * 10000) + 1000
    growth.value = Math.floor(Math.random() * 100)
    const mockData = Array.from({ length: 7 }, () => Math.floor(Math.random() * 500) + 100)
    if (props.chartType === 'bar') {
      mockData.forEach((el, idx) => {
        chartData.value.push({
          value: el,
          itemStyle: { color: idx % 2 ? '#468DFF' : '#86DF6C' }
        })
      })
    } else {
      chartData.value = mockData
    }
  } finally {
    setLoading(false)
  }
}

fetchData({ quota: props.quota })
</script>

<style scoped lang="less">
.general-card {
  min-height: 204px;
}
.content {
  display: flex;
  align-items: center;
  width: 100%;
  margin-bottom: 12px;
}
.percent-text {
  margin-left: 16px;
}
.chart {
  width: 100%;
  height: 80px;
  vertical-align: bottom;
}
.unit {
  padding-left: 8px;
  font-size: 12px;
}
.label {
  padding-right: 8px;
  font-size: 12px;
}
</style>
