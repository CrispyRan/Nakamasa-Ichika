<template>
  <apexchart
    v-if="renderChart"
    :type="type"
    :options="chartOptions"
    :series="series"
    :width="width"
    :height="height"
  />
</template>

<script setup>
import { ref, nextTick, computed } from 'vue'

const props = defineProps({
  type: {
    type: String,
    default: 'line'
  },
  options: {
    type: Object,
    default() {
      return {}
    }
  },
  series: {
    type: Array,
    default() {
      return []
    }
  },
  width: {
    type: String,
    default: '100%'
  },
  height: {
    type: String,
    default: '350'
  }
})

const renderChart = ref(false)

const chartOptions = computed(() => ({
  chart: {
    toolbar: {
      show: false
    },
    zoom: {
      enabled: false
    },
    ...props.options?.chart
  },
  ...props.options
}))

nextTick(() => {
  renderChart.value = true
})
</script>

<style scoped lang="less"></style>
