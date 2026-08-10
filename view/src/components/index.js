import { use } from 'echarts/core'
import { CanvasRenderer } from 'echarts/renderers'
import {
  BarChart,
  LineChart,
  PieChart,
  RadarChart,
  GaugeChart,
  ScatterChart,
  MapChart
} from 'echarts/charts'
import {
  GridComponent,
  TooltipComponent,
  LegendComponent,
  DataZoomComponent,
  GraphicComponent,
  GeoComponent,
  VisualMapComponent
} from 'echarts/components'
import VueApexCharts from 'vue3-apexcharts'

import MaWangEditor from './ma-wangEditor/index.vue'
import MaColorPicker from './ma-colorPicker/index.vue'
import MaCityLinkage from './ma-cityLinkage/index.vue'

import SaChart from './sa-chart/index.vue'
import SaApexChart from './sa-apexchart/index.vue'
import SaChartCard from './sa-chart-card/index.vue'
import SaCarousel from './sa-carousel/index.vue'
import SaWorldMap from './sa-world-map/index.vue'
import SaChinaMap from './sa-china-map/index.vue'
import SaCheckbox from './sa-checkbox/index.vue'
import SaRadio from './sa-radio/index.vue'
import SaSelect from './sa-select/index.vue'
import SaSwitch from './sa-switch/index.vue'
import SaTable from './sa-table/index.vue'
import SaTreeSlider from './sa-treeSlider/index.vue'
import SaResource from './sa-resource/index.vue'
import SaResourceButton from './sa-resource/button.vue'
import SaDict from './sa-dict/index.vue'
import SaUser from './sa-user/index.vue'
import SaUploadImage from './sa-upload-image/index.vue'
import SaUploadFile from './sa-upload-file/index.vue'
import SaUploadChunk from './sa-upload-chunk/index.vue'
import SaIcon from './sa-icon/index.vue'
import SaIconPicker from './sa-icon-picker/index.vue'
import SaPickImage from './sa-pick-image/index.vue'

use([
  CanvasRenderer,
  BarChart,
  LineChart,
  PieChart,
  RadarChart,
  GaugeChart,
  ScatterChart,
  MapChart,
  GridComponent,
  TooltipComponent,
  LegendComponent,
  DataZoomComponent,
  GraphicComponent,
  GeoComponent,
  VisualMapComponent
])

export default {
  install(Vue) {
    Vue.use(VueApexCharts)
    Vue.component('MaWangEditor', MaWangEditor)
    Vue.component('MaColorPicker', MaColorPicker)
    Vue.component('MaCityLinkage', MaCityLinkage)

    Vue.component('SaChart', SaChart)
    Vue.component('SaApexChart', SaApexChart)
    Vue.component('SaChartCard', SaChartCard)
    Vue.component('SaCarousel', SaCarousel)
    Vue.component('SaWorldMap', SaWorldMap)
    Vue.component('SaChinaMap', SaChinaMap)
    Vue.component('SaCheckbox', SaCheckbox)
    Vue.component('SaRadio', SaRadio)
    Vue.component('SaSelect', SaSelect)
    Vue.component('SaSwitch', SaSwitch)
    Vue.component('SaTable', SaTable)
    Vue.component('SaTreeSlider', SaTreeSlider)
    Vue.component('SaResource', SaResource)
    Vue.component('SaResourceButton', SaResourceButton)
    Vue.component('SaDict', SaDict)
    Vue.component('SaUser', SaUser)
    Vue.component('SaUploadImage', SaUploadImage)
    Vue.component('SaUploadFile', SaUploadFile)
    Vue.component('SaUploadChunk', SaUploadChunk)
    Vue.component('SaIcon', SaIcon)
    Vue.component('SaIconPicker', SaIconPicker)
    Vue.component('SaPickImage', SaPickImage)
  }
}
