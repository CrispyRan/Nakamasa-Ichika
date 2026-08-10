<template>
  <div class="world-map-container">
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

// 加载世界地图 GeoJSON
const loadWorldMap = async () => {
  if (mapRegistered) return true
  
  try {
    // 从 CDN 加载世界地图数据
    const response = await fetch('https://cdn.jsdelivr.net/npm/echarts-map@3.0.1/json/world.json')
    if (!response.ok) {
      throw new Error('Failed to load map data')
    }
    const worldJson = await response.json()
    echarts.registerMap('world', worldJson)
    mapRegistered = true
    return true
  } catch (error) {
    console.error('Failed to load world map:', error)
    return false
  }
}

// 获取国家坐标（主要国家）
const countryCoords = {
  'China': [104.1954, 35.8617],
  'United States': [-95.7129, 37.0902],
  'Japan': [138.2529, 36.2048],
  'South Korea': [127.7669, 35.9078],
  'Germany': [10.4515, 51.1657],
  'United Kingdom': [-3.4360, 55.3781],
  'France': [2.2137, 46.2276],
  'Singapore': [103.8198, 1.3521],
  'Australia': [133.7751, -25.2744],
  'Canada': [-106.3468, 56.1304],
  'Russia': [105.3188, 61.5240],
  'Brazil': [-51.9253, -14.2350],
  'India': [78.9629, 20.5937],
  'Italy': [12.5674, 41.8719],
  'Spain': [-3.7492, 40.4637],
  'Netherlands': [5.2913, 52.1326],
  'Taiwan': [120.9605, 23.6978],
  'Hong Kong': [114.1694, 22.3193],
  'Malaysia': [101.9758, 4.2105],
  'Thailand': [100.9925, 15.8700],
  'Vietnam': [108.2772, 14.0583],
  'Indonesia': [113.9213, -0.7893],
  'Philippines': [121.7740, 12.8797],
  'Mexico': [-102.5528, 23.6345],
  'Argentina': [-63.6167, -38.4161],
  'South Africa': [22.9375, -30.5595],
  'Egypt': [30.8025, 26.8206],
  'Nigeria': [8.6753, 9.0820],
  'Kenya': [37.9062, -0.0236],
  'Saudi Arabia': [45.0792, 23.8859],
  'UAE': [53.8478, 23.4241],
  'Turkey': [35.2433, 38.9637],
  'Poland': [19.1451, 51.9194],
  'Sweden': [18.6435, 60.1282],
  'Norway': [8.4689, 60.4720],
  'Finland': [25.7482, 61.9241],
  'Denmark': [9.5018, 56.2639],
  'Belgium': [4.4699, 50.5039],
  'Switzerland': [8.2275, 46.8182],
  'Austria': [14.5501, 47.5162],
  'Portugal': [-8.2245, 39.3999],
  'Greece': [21.8243, 39.0742],
  'Czech Republic': [15.4730, 49.8175],
  'Hungary': [19.5033, 47.1625],
  'Romania': [24.9668, 45.9432],
  'Ukraine': [31.1656, 48.3794],
  'New Zealand': [174.8860, -40.9006],
  'Ireland': [-8.2439, 53.4129],
  'Israel': [34.8516, 31.0461],
  'Pakistan': [69.3451, 30.3753],
  'Bangladesh': [90.3563, 23.6850],
  'Iran': [53.6880, 32.4279],
  'Iraq': [43.6793, 33.2232],
  'Kuwait': [47.4817, 29.3117],
  'Qatar': [51.1839, 25.3548],
  'Bahrain': [50.6378, 26.0667],
  'Oman': [55.9233, 21.5126],
  'Morocco': [-7.0926, 31.7917],
  'Algeria': [1.6596, 28.0339],
  'Tunisia': [9.5375, 33.8869],
  'Chile': [-71.5430, -35.6751],
  'Colombia': [-74.2973, 4.5709],
  'Peru': [-75.0152, -9.1900],
  'Venezuela': [-66.5897, 6.4238],
  'Ecuador': [-78.1834, -1.8312],
  'Uruguay': [-55.7658, -32.5228],
  'Paraguay': [-58.4438, -23.4425],
  'Bolivia': [-63.5887, -16.2902],
  'Cuba': [-77.7812, 21.5218],
  'Jamaica': [-77.2975, 18.1096],
  'Dominican Republic': [-70.1627, 18.7357],
  'Puerto Rico': [-66.5901, 18.2208],
  'Guatemala': [-90.2308, 15.7835],
  'Honduras': [-86.2419, 15.2000],
  'El Salvador': [-88.8965, 13.7942],
  'Nicaragua': [-85.2072, 12.8654],
  'Costa Rica': [-83.7534, 9.7489],
  'Panama': [-80.7821, 8.5380],
  'Cambodia': [104.9910, 12.5657],
  'Laos': [102.4955, 19.8563],
  'Myanmar': [95.9562, 21.9162],
  'Nepal': [84.1240, 28.3949],
  'Sri Lanka': [80.7718, 7.8731],
  'Mongolia': [103.8467, 46.8625],
  'North Korea': [127.5101, 40.3399],
  'Kazakhstan': [66.9237, 48.0196],
  'Uzbekistan': [64.5853, 41.3775],
  'Azerbaijan': [47.5769, 40.1431],
  'Georgia': [43.3569, 42.3154],
  'Armenia': [45.0382, 40.0691],
  'Lebanon': [35.8623, 33.8547],
  'Jordan': [36.2384, 30.5852],
  'Syria': [38.9968, 34.8021],
  'Afghanistan': [67.7099, 33.9391],
  'Slovakia': [19.6990, 48.6690],
  'Slovenia': [14.9955, 46.1512],
  'Croatia': [15.2000, 45.1000],
  'Serbia': [21.0059, 44.0165],
  'Bulgaria': [25.4858, 42.7339],
  'Macedonia': [21.7453, 41.5124],
  'Albania': [20.1683, 41.1533],
  'Lithuania': [23.8813, 55.1694],
  'Latvia': [24.6032, 56.8796],
  'Estonia': [25.0136, 58.5953],
  'Belarus': [27.9534, 53.7098],
  'Moldova': [28.3699, 47.4116],
  'Bosnia and Herzegovina': [17.6791, 43.9159],
  'Montenegro': [19.3744, 42.7087],
  'Kosovo': [20.9029, 42.6026],
  'Cyprus': [33.4299, 35.1264],
  'Iceland': [-19.0208, 64.9631],
  'Luxembourg': [6.1296, 49.8153],
  'Malta': [14.3754, 35.9375],
  'Monaco': [7.4246, 43.7503],
  'San Marino': [12.4578, 43.9424],
  'Vatican City': [12.4534, 41.9029],
  'Liechtenstein': [9.5554, 47.1660],
  'Andorra': [1.5218, 42.5063],
  'Macedonia': [21.7453, 41.5124]
}

// 格式化数据
const formatData = (data) => {
  const mapData = []
  const scatterData = []
  
  data.forEach(item => {
    const coords = countryCoords[item.name] || countryCoords[item.name?.replace('The ', '').replace('Republic of ', '')]
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
  
  const loaded = await loadWorldMap()
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
          ? ['#1a3a5c', '#2a5a8c', '#3a7abc', '#4a9aec', '#6abaff']
          : ['#e0f3ff', '#a0d8ff', '#60bdff', '#20a2ff', '#0087ff']
      },
      calculable: true,
      itemWidth: 10,
      itemHeight: 100
    },
    geo: {
      map: 'world',
      roam: true,
      zoom: 1.2,
      center: [10, 20],
      scaleLimit: {
        min: 1,
        max: 8
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
          areaColor: isDark ? '#3a5a7a' : '#a0d8ff'
        }
      },
      itemStyle: {
        areaColor: isDark ? '#1a2a3a' : '#e8f4fc',
        borderColor: isDark ? '#2a3a4a' : '#b0d4e8',
        borderWidth: 0.5
      }
    },
    series: [
      {
        name: '用户分布',
        type: 'map',
        map: 'world',
        geoIndex: 0,
        data: mapData
      },
      {
        name: '热点城市',
        type: 'effectScatter',
        coordinateSystem: 'geo',
        data: scatterData.slice(0, 20),
        symbolSize: (val) => {
          return Math.max(Math.min(val[2] / maxValue * 20 + 5, 25), 6)
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
          color: '#ff6b6b',
          shadowBlur: 10,
          shadowColor: 'rgba(255, 107, 107, 0.5)'
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
.world-map-container {
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
