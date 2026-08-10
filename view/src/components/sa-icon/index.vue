<template>
  <template v-if="value.indexOf(':') === -1">
    <component :is="value" :size="props.size"></component>
  </template>
  <template v-else>
    <Icon :icon="value" class="iconify-icon" :style="{ fontSize: props.size + 'px' }" />
  </template>
</template>

<script setup>
import { ref, watch } from 'vue'
import { Icon } from '@iconify/vue'
const value = ref('')

const props = defineProps({
  icon: { type: String },
  size: { type: Number, default: 24 },
})

// 将 icon-xxx 格式转换为 IconXxx 格式
const normalizeIconName = (icon) => {
  if (!icon) return ''
  // 如果已经是 IconXxx 格式，直接返回
  if (icon.startsWith('Icon')) return icon
  // 如果是 icon-xxx 格式，转换为 IconXxx
  if (icon.startsWith('icon-')) {
    return 'Icon' + icon.slice(5).split('-').map(word => 
      word.charAt(0).toUpperCase() + word.slice(1)
    ).join('')
  }
  // 其他情况直接返回
  return icon
}

watch(
  () => props.icon,
  (vl) => {
    if (vl) {
      value.value = normalizeIconName(vl)
    }
  },
  { immediate: true }
)
</script>
