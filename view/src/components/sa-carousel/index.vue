<template>
  <a-carousel
    :indicator-type="indicatorType"
    :show-arrow="showArrow"
    :auto-play="autoPlay"
    :animation-name="animationName"
    :style="carouselStyle"
  >
    <a-carousel-item v-for="(item, index) in items" :key="index">
      <div class="carousel-item" @click="handleClick(item, index)">
        <img
          v-if="typeof item === 'string' || item.src"
          :src="typeof item === 'string' ? item : item.src"
          :alt="item.title || ''"
          class="carousel-image"
        />
        <div v-if="item.title || item.description" class="carousel-overlay">
          <div v-if="item.title" class="carousel-title">{{ item.title }}</div>
          <div v-if="item.description" class="carousel-desc">{{ item.description }}</div>
        </div>
        <slot v-if="$slots.default" :item="item" :index="index" />
      </div>
    </a-carousel-item>
  </a-carousel>
</template>

<script setup>
import { computed } from 'vue'

const props = defineProps({
  // 图片列表，可以是字符串数组或对象数组
  // 字符串格式: ['url1', 'url2']
  // 对象格式: [{ src: 'url', title: '标题', description: '描述' }]
  images: {
    type: Array,
    default: () => []
  },
  // 指示器类型
  indicatorType: {
    type: String,
    default: 'slider',
    validator: (v) => ['dot', 'slider', 'never'].includes(v)
  },
  // 箭头显示方式
  showArrow: {
    type: String,
    default: 'hover',
    validator: (v) => ['always', 'hover', 'never'].includes(v)
  },
  // 自动播放
  autoPlay: {
    type: Boolean,
    default: true
  },
  // 动画效果
  animationName: {
    type: String,
    default: 'slide',
    validator: (v) => ['slide', 'fade', 'card'].includes(v)
  },
  // 高度
  height: {
    type: String,
    default: '170px'
  },
  // 圆角
  borderRadius: {
    type: String,
    default: '4px'
  }
})

const emit = defineEmits(['click', 'change'])

const items = computed(() => props.images || [])

const carouselStyle = computed(() => ({
  width: '100%',
  height: props.height,
  borderRadius: props.borderRadius,
  overflow: 'hidden'
}))

const handleClick = (item, index) => {
  emit('click', { item, index })
}
</script>

<script>
export default { name: 'SaCarousel' }
</script>

<style scoped lang="less">
.carousel-item {
  width: 100%;
  height: 100%;
  position: relative;
  cursor: pointer;
}
.carousel-image {
  width: 100%;
  height: 100%;
  object-fit: cover;
}
.carousel-overlay {
  position: absolute;
  bottom: 0;
  left: 0;
  right: 0;
  padding: 16px;
  background: linear-gradient(transparent, rgba(0, 0, 0, 0.6));
  color: #fff;
}
.carousel-title {
  font-size: 16px;
  font-weight: 500;
  margin-bottom: 4px;
}
.carousel-desc {
  font-size: 12px;
  opacity: 0.8;
}
</style>
