<template>
  <div class="china-map-container">
    <a-spin :loading="loading" style="width: 100%">
      <div ref="chartRef" class="chart-wrapper"></div>
    </a-spin>
  </div>
</template>

<script setup>
import { ref, onMounted, onUnmounted, watch, nextTick } from 'vue'
import * as echarts from 'echarts/core'
import { MapChart, ScatterChart, EffectScatterChart } from 'echarts/charts'
import { 
  GeoComponent, 
  TooltipComponent, 
  VisualMapComponent,
  LegendComponent 
} from 'echarts/components'
import { CanvasRenderer } from 'echarts/renderers'

// 注册必要的组件
echarts.use([
  MapChart,
  ScatterChart,
  EffectScatterChart,
  GeoComponent,
  TooltipComponent,
  VisualMapComponent,
  LegendComponent,
  CanvasRenderer
])

const props = defineProps({
  data: {
    type: Array,
    default: () => []
  },
  height: {
    type: String,
    default: '400px'
  },
  theme: {
    type: String,
    default: 'dark'
  }
})

const chartRef = ref(null)
const loading = ref(true)
let chartInstance = null
let mapRegistered = false

// 省份名称映射（中文 -> 英文/拼音）
const provinceNameMap = {
  '北京': 'beijing',
  '天津': 'tianjin',
  '河北': 'hebei',
  '山西': 'shanxi',
  '内蒙古': 'neimenggu',
  '辽宁': 'liaoning',
  '吉林': 'jilin',
  '黑龙江': 'heilongjiang',
  '上海': 'shanghai',
  '江苏': 'jiangsu',
  '浙江': 'zhejiang',
  '安徽': 'anhui',
  '福建': 'fujian',
  '江西': 'jiangxi',
  '山东': 'shandong',
  '河南': 'henan',
  '湖北': 'hubei',
  '湖南': 'hunan',
  '广东': 'guangdong',
  '广西': 'guangxi',
  '海南': 'hainan',
  '重庆': 'chongqing',
  '四川': 'sichuan',
  '贵州': 'guizhou',
  '云南': 'yunnan',
  '西藏': 'xizang',
  '陕西': 'shaanxi',
  '甘肃': 'gansu',
  '青海': 'qinghai',
  '宁夏': 'ningxia',
  '新疆': 'xinjiang',
  '台湾': 'taiwan',
  '香港': 'xianggang',
  '澳门': 'aomen'
}

// 省份坐标中心点
const provinceCoords = {
  '北京': [116.405285, 39.904989],
  '天津': [117.190182, 39.125596],
  '河北': [114.502461, 38.045474],
  '山西': [112.549248, 37.857014],
  '内蒙古': [111.670801, 40.818311],
  '辽宁': [123.429096, 41.796767],
  '吉林': [125.3245, 43.886841],
  '黑龙江': [126.642464, 45.756967],
  '上海': [121.472644, 31.231706],
  '江苏': [118.767413, 32.041544],
  '浙江': [120.153576, 30.287459],
  '安徽': [117.283042, 31.86119],
  '福建': [119.306239, 26.075302],
  '江西': [115.892151, 28.676493],
  '山东': [117.000923, 36.675807],
  '河南': [113.665412, 34.757975],
  '湖北': [114.298572, 30.584355],
  '湖南': [112.982279, 28.19409],
  '广东': [113.280637, 23.125178],
  '广西': [108.320004, 22.82402],
  '海南': [110.33119, 20.031971],
  '重庆': [106.504962, 29.533155],
  '四川': [104.065735, 30.659462],
  '贵州': [106.713478, 26.578343],
  '云南': [102.712251, 25.040609],
  '西藏': [91.132212, 29.660361],
  '陕西': [108.948024, 34.263161],
  '甘肃': [103.823557, 36.058039],
  '青海': [101.778916, 36.623178],
  '宁夏': [106.278179, 38.46637],
  '新疆': [87.617733, 43.792818],
  '台湾': [121.509062, 25.044332],
  '香港': [114.173355, 22.320048],
  '澳门': [113.54909, 22.198951]
}

// 加载中国地图 GeoJSON
const loadChinaMap = async () => {
  if (mapRegistered) return true
  
  try {
    // 从阿里云 DataV 加载中国地图数据
    const response = await fetch('https://geo.datav.aliyun.com/areas_v3/bound/100000_full.json')
    if (!response.ok) {
      throw new Error('Failed to load map data')
    }
    const chinaJson = await response.json()
    echarts.registerMap('china', chinaJson)
    mapRegistered = true
    return true
  } catch (error) {
    console.error('Failed to load china map:', error)
    return false
  }
}

// 格式化数据
const formatData = (data) => {
  const mapData = []
  const scatterData = []
  
  data.forEach(item => {
    const coords = provinceCoords[item.name]
    if (coords) {
      mapData.push({
        name: item.name,
        value: item.value
      })
      scatterData.push({
        name: item.name,
        value: [...coords, item.value]
      })
    }
  })
  
  return { mapData, scatterData }
}

// 初始化图表
const initChart = async () => {
  if (!chartRef.value) return
  
  const loaded = await loadChinaMap()
  if (!loaded) {
    loading.value = false
    return
  }
  
  const isDark = props.theme === 'dark'
  const { mapData, scatterData } = formatData(props.data)
  
  // 计算最大值用于视觉映射
  const maxValue = Math.max(...props.data.map(d => d.value), 1)
  
  const option = {
    backgroundColor: 'transparent',
    tooltip: {
      trigger: 'item',
      backgroundColor: isDark ? 'rgba(20, 20, 20, 0.9)' : 'rgba(255, 255, 255, 0.95)',
      borderColor: isDark ? '#333' : '#ddd',
      textStyle: {
        color: isDark ? '#fff' : '#333',
        fontSize: 12
      },
      formatter: (params) => {
        if (params.seriesType === 'effectScatter' || params.seriesType === 'scatter') {
          return `${params.name}<br/>用户数: <b>${params.value[2]?.toLocaleString() || 0}</b>`
        }
        if (params.seriesType === 'map') {
          return `${params.name}<br/>用户数: <b>${params.value?.toLocaleString() || 0}</b>`
        }
        return params.name
      }
    },
    visualMap: {
      min: 0,
      max: maxValue,
      left: 'left',
      top: 'bottom',
      text: ['高', '低'],
      textStyle: {
        color: isDark ? '#aaa' : '#666',
        fontSize: 11
      },
      inRange: {
        color: isDark 
          ? ['#1a4a3c', '#2a6a5c', '#3a8a7c', '#4aaa9c', '#6acabc']
          : ['#e0fff3', '#a0ffe3', '#60ffd3', '#20ffc3', '#00ffa3']
      },
      calculable: true,
      itemWidth: 10,
      itemHeight: 100
    },
    geo: {
      map: 'china',
      roam: true,
      zoom: 1.2,
      center: [104, 36],
      scaleLimit: {
        min: 0.8,
        max: 5
      },
      label: {
        show: false
      },
      emphasis: {
        label: {
          show: true,
          fontSize: 10,
          color: isDark ? '#fff' : '#333'
        },
        itemStyle: {
          areaColor: isDark ? '#3a7a6a' : '#a0ffe3'
        }
      },
      itemStyle: {
        areaColor: isDark ? '#1a2a3a' : '#e8fcf4',
        borderColor: isDark ? '#2a4a5a' : '#b0e4d8',
        borderWidth: 0.5
      }
    },
    series: [
      {
        name: '用户分布',
        type: 'map',
        map: 'china',
        geoIndex: 0,
        data: mapData
      },
      {
        name: '热点省份',
        type: 'effectScatter',
        coordinateSystem: 'geo',
        data: scatterData.slice(0, 15),
        symbolSize: (val) => {
          return Math.max(Math.min(val[2] / maxValue * 18 + 4, 22), 5)
        },
        showEffectOn: 'render',
        rippleEffect: {
          brushType: 'stroke',
          scale: 3,
          period: 4
        },
        label: {
          show: false
        },
        emphasis: {
          scale: true
        },
        itemStyle: {
          color: '#00d4aa',
          shadowBlur: 10,
          shadowColor: 'rgba(0, 212, 170, 0.5)'
        }
      }
    ]
  }
  
  if (chartInstance) {
    chartInstance.dispose()
  }
  
  chartInstance = echarts.init(chartRef.value)
  chartInstance.setOption(option)
  loading.value = false
}

// 响应式调整
const handleResize = () => {
  chartInstance?.resize()
}

// 监听数据变化
watch(() => props.data, () => {
  if (mapRegistered) {
    initChart()
  }
}, { deep: true })

watch(() => props.theme, () => {
  if (mapRegistered) {
    initChart()
  }
})

onMounted(async () => {
  await nextTick()
  await initChart()
  window.addEventListener('resize', handleResize)
})

onUnmounted(() => {
  window.removeEventListener('resize', handleResize)
  chartInstance?.dispose()
})
</script>

<style scoped lang="less">
.china-map-container {
  width: 100%;
  height: v-bind(height);
  min-height: 300px;
}

.chart-wrapper {
  width: 100%;
  height: 100%;
  min-height: 300px;
}
</style>
