<template>
  <a-spin :loading="loading" style="width: 100%">
    <a-card :bordered="false" class="chart-card">
      <div class="chart-card-wrap">
        <div class="content-area">
          <div class="statistic-title">{{ title }}</div>
          <div class="statistic-value">
            <slot name="prefix" />
            <span class="value">{{ formattedValue }}</span>
            <slot name="suffix" />
          </div>
          <div v-if="showGrowth" class="growth-area">
            <span class="growth-label">{{ growthLabel }}</span>
            <span :class="['growth-value', growth >= 0 ? 'up' : 'down']">
              {{ growth >= 0 ? '+' : '' }}{{ growth }}%
              <icon-arrow-rise v-if="growth >= 0" />
              <icon-arrow-fall v-else />
            </span>
          </div>
        </div>
        <div class="chart-area">
          <sa-chart v-if="!loading && chartOption" :options="chartOption" height="100%" />
        </div>
      </div>
    </a-card>
  </a-spin>
</template>

<script setup>
import { ref, computed, watch, onMounted, onUnmounted } from 'vue'
import useChartOption from '@/hooks/chart-option'

const props = defineProps({
  title: {
    type: String,
    default: ''
  },
  value: {
    type: [Number, String],
    default: 0
  },
  precision: {
    type: Number,
    default: 0
  },
  growth: {
    type: Number,
    default: 0
  },
  growthLabel: {
    type: String,
    default: '较昨日'
  },
  showGrowth: {
    type: Boolean,
    default: true
  },
  chartType: {
    type: String,
    default: 'bar',
    validator: (v) => ['bar', 'line', 'pie'].includes(v)
  },
  chartData: {
    type: Array,
    default: () => []
  },
  chartHeight: {
    type: String,
    default: '90px'
  },
  loading: {
    type: Boolean,
    default: false
  },
  cardStyle: {
    type: Object,
    default: () => ({})
  },
  colors: {
    type: Array,
    default: () => ['#165DFF', '#14C9C9', '#F7BA1E', '#722ED1', '#F53F3F']
  }
})

// 格式化数值
const formattedValue = computed(() => {
  const val = Number(props.value)
  if (isNaN(val)) return props.value
  return val.toLocaleString('zh-CN', {
    minimumFractionDigits: props.precision,
    maximumFractionDigits: props.precision
  })
})

// 柱状图配置
const useBarOption = (data, colors) => {
  return useChartOption(() => ({
    grid: { left: 0, right: 0, top: 5, bottom: 0 },
    xAxis: { type: 'category', show: false },
    yAxis: { show: false },
    tooltip: { show: true, trigger: 'axis' },
    series: {
      type: 'bar',
      data: data.value.map((item, idx) => ({
        value: typeof item === 'object' ? item.value : item,
        itemStyle: {
          color: colors[idx % colors.length],
          borderRadius: 2
        }
      })),
      barWidth: 6
    }
  }))
}

// 折线图配置
const useLineOption = (data, colors) => {
  return useChartOption(() => ({
    grid: { left: 0, right: 0, top: 5, bottom: 0 },
    xAxis: { type: 'category', show: false },
    yAxis: { show: false },
    tooltip: { show: true, trigger: 'axis' },
    series: {
      type: 'line',
      data: data.value.map(item => typeof item === 'object' ? item.value : item),
      showSymbol: false,
      smooth: true,
      lineStyle: {
        color: colors[0],
        width: 2
      },
      areaStyle: {
        color: {
          type: 'linear',
          x: 0, y: 0, x2: 0, y2: 1,
          colorStops: [
            { offset: 0, color: colors[0] + '40' },
            { offset: 1, color: colors[0] + '00' }
          ]
        }
      }
    }
  }))
}

// 饼图配置
const usePieOption = (data, colors) => {
  return useChartOption((isDark) => ({
    grid: { left: 0, right: 0, top: 0, bottom: 0 },
    legend: {
      show: true,
      top: 'center',
      right: '0',
      orient: 'vertical',
      icon: 'circle',
      itemWidth: 5,
      itemHeight: 5,
      itemGap: 4,
      textStyle: { 
        color: isDark ? '#A6ADB5' : '#4E5969', 
        fontSize: 10 
      }
    },
    tooltip: { show: true },
    series: {
      type: 'pie',
      radius: ['40%', '60%'],
      center: ['35%', '50%'],
      label: { show: false },
      data: data.value.map((item, idx) => ({
        name: item.name || `项目${idx + 1}`,
        value: typeof item === 'object' ? item.value : item,
        itemStyle: { color: colors[idx % colors.length] }
      }))
    }
  }))
}

const chartDataRef = ref([])
const chartOption = ref(null)

const updateChart = () => {
  chartDataRef.value = props.chartData
  
  let option
  switch (props.chartType) {
    case 'line':
      option = useLineOption(chartDataRef, props.colors)
      break
    case 'pie':
      option = usePieOption(chartDataRef, props.colors)
      break
    default:
      option = useBarOption(chartDataRef, props.colors)
  }
  
  chartOption.value = option.chartOption.value
}

watch(() => props.chartData, updateChart, { deep: true })

onMounted(() => {
  updateChart()
})
</script>

<script>
export default { name: 'SaChartCard' }
</script>

<style scoped lang="less">
.chart-card {
  border-radius: 4px;
  height: 100%;
  
  :deep(.arco-card-body) {
    padding: 0;
    height: 100%;
  }
}

.chart-card-wrap {
  display: flex;
  align-items: stretch;
  justify-content: space-between;
  padding: 12px;
  height: 100%;
  min-height: 90px;
  box-sizing: border-box;
  
  @media (min-width: 576px) {
    padding: 14px 16px;
    min-height: 100px;
  }
}

.content-area {
  display: flex;
  flex-direction: column;
  justify-content: center;
  flex-shrink: 0;
  width: 85px;
  
  @media (min-width: 576px) {
    width: 100px;
  }
}

.statistic-title {
  font-size: 12px;
  font-weight: 500;
  color: var(--color-text-2);
  margin-bottom: 6px;
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
  
  @media (min-width: 576px) {
    font-size: 13px;
    margin-bottom: 8px;
  }
}

.statistic-value {
  display: flex;
  align-items: center;
  
  .value {
    font-size: 18px;
    font-weight: 600;
    color: var(--color-text-1);
    
    @media (min-width: 576px) {
      font-size: 22px;
    }
  }
}

.growth-area {
  display: flex;
  align-items: center;
  margin-top: 6px;
  font-size: 11px;
  
  @media (min-width: 576px) {
    margin-top: 8px;
    font-size: 12px;
  }
}

.growth-label {
  color: var(--color-text-3);
  margin-right: 4px;
}

.growth-value {
  display: flex;
  align-items: center;
  gap: 2px;
  
  &.up {
    color: rgb(var(--success-6));
  }
  
  &.down {
    color: rgb(var(--danger-6));
  }
}

.chart-area {
  flex: 1;
  min-width: 0;
  margin-left: 8px;
  display: flex;
  align-items: center;
}
</style>
