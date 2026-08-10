import { computed, ref } from 'vue'
import useAppStore from '@/store/modules/app'

/**
 * 图表配置 Hook
 * 支持根据主题动态生成图表配置
 * @param {Function} sourceOption - 接收 isDark 参数，返回 ECharts 配置对象
 * @returns {Object} { chartOption }
 */
export default function useChartOption(sourceOption) {
  const appStore = useAppStore()
  
  const isDark = computed(() => appStore.mode === 'dark')
  
  const chartOption = computed(() => {
    return sourceOption(isDark.value)
  })
  
  return {
    chartOption
  }
}

/**
 * 柱状图配置工厂
 */
export const barChartOptionsFactory = () => {
  const data = ref([])
  const { chartOption } = useChartOption((isDark) => ({
    grid: {
      left: 0,
      right: 0,
      top: 10,
      bottom: 0
    },
    xAxis: {
      type: 'category',
      show: false
    },
    yAxis: {
      show: false
    },
    tooltip: {
      show: true,
      trigger: 'axis'
    },
    series: {
      name: 'total',
      data,
      type: 'bar',
      barWidth: 7,
      itemStyle: {
        borderRadius: 2
      }
    }
  }))
  return { data, chartOption }
}

/**
 * 折线图配置工厂
 */
export const lineChartOptionsFactory = () => {
  const data = ref([[], []])
  const { chartOption } = useChartOption((isDark) => ({
    grid: {
      left: 0,
      right: 0,
      top: 10,
      bottom: 0
    },
    xAxis: {
      type: 'category',
      show: false
    },
    yAxis: {
      show: false
    },
    tooltip: {
      show: true,
      trigger: 'axis'
    },
    series: [
      {
        name: 'series1',
        data: data.value[0],
        type: 'line',
        showSymbol: false,
        smooth: true,
        lineStyle: {
          color: '#165DFF',
          width: 3
        }
      },
      {
        name: 'series2',
        data: data.value[1],
        type: 'line',
        showSymbol: false,
        smooth: true,
        lineStyle: {
          color: '#6AA1FF',
          width: 3,
          type: 'dashed'
        }
      }
    ]
  }))
  return { data, chartOption }
}

/**
 * 饼图配置工厂
 */
export const pieChartOptionsFactory = () => {
  const data = ref([])
  const { chartOption } = useChartOption((isDark) => ({
    grid: {
      left: 0,
      right: 0,
      top: 0,
      bottom: 0
    },
    legend: {
      show: true,
      top: 'center',
      right: '0',
      orient: 'vertical',
      icon: 'circle',
      itemWidth: 6,
      itemHeight: 6,
      textStyle: {
        color: isDark ? '#A6ADB5' : '#4E5969'
      }
    },
    tooltip: {
      show: true
    },
    series: [
      {
        name: '总计',
        type: 'pie',
        radius: ['50%', '70%'],
        label: {
          show: false
        },
        data
      }
    ]
  }))
  return { data, chartOption }
}
