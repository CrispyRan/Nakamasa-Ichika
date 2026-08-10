<template>
  <div class="carousel-container ma-content-block mt-3">
    <a-carousel
      auto-play
      :style="{ width: '100%', height: '240px' }"
      indicator-type="dot"
      show-arrow="hover"
      animation-name="fade"
    >
      <a-carousel-item v-for="(item, index) in carouselItems" :key="index">
        <div class="carousel-item" :style="{ background: item.bg }">
          <div class="carousel-content">
            <div class="carousel-text">
              <h3>{{ item.title }}</h3>
              <p>{{ item.desc }}</p>
              <a-button type="primary" size="small" v-if="item.link">
                了解更多
                <template #icon><icon-right /></template>
              </a-button>
            </div>
            <div class="carousel-illustration">
              <component :is="item.icon" :size="80" :style="{ color: item.iconColor }" />
            </div>
          </div>
        </div>
      </a-carousel-item>
    </a-carousel>
  </div>
</template>

<script setup>
import { ref } from 'vue'

const carouselItems = ref([
  {
    title: '欢迎使用管理系统',
    desc: '高效、安全、可靠的后台管理解决方案',
    bg: 'linear-gradient(135deg, #667eea 0%, #764ba2 100%)',
    icon: 'icon-rocket',
    iconColor: 'rgba(255,255,255,0.9)',
    link: '#'
  },
  {
    title: '数据可视化',
    desc: '实时监控系统运行状态，数据一目了然',
    bg: 'linear-gradient(135deg, #f093fb 0%, #f5576c 100%)',
    icon: 'icon-bar-chart',
    iconColor: 'rgba(255,255,255,0.9)',
    link: '#'
  },
  {
    title: '安全可靠',
    desc: '多重安全防护，保障您的数据安全',
    bg: 'linear-gradient(135deg, #4facfe 0%, #00f2fe 100%)',
    icon: 'icon-safe',
    iconColor: 'rgba(255,255,255,0.9)',
    link: '#'
  },
  {
    title: '云端同步',
    desc: '数据云端备份，随时随地访问',
    bg: 'linear-gradient(135deg, #43e97b 0%, #38f9d7 100%)',
    icon: 'icon-cloud',
    iconColor: 'rgba(255,255,255,0.9)',
    link: '#'
  }
])
</script>

<style scoped lang="less">
.carousel-container {
  border-radius: 12px;
  overflow: hidden;
  
  :deep(.arco-carousel) {
    border-radius: 12px;
  }
  
  :deep(.arco-carousel-indicator) {
    bottom: 12px;
  }
  
  :deep(.arco-carousel-indicator-item) {
    width: 24px;
    height: 4px;
    border-radius: 2px;
    background: rgba(255, 255, 255, 0.4);
    
    &.arco-carousel-indicator-item-active {
      width: 32px;
      background: #fff;
    }
  }
  
  :deep(.arco-carousel-arrow) {
    background: rgba(255, 255, 255, 0.2);
    backdrop-filter: blur(4px);
    width: 36px;
    height: 36px;
    
    &:hover {
      background: rgba(255, 255, 255, 0.4);
    }
    
    .arco-icon {
      color: #fff;
    }
  }
}

.carousel-item {
  width: 100%;
  height: 240px;
  display: flex;
  align-items: center;
  padding: 0 48px;
}

.carousel-content {
  display: flex;
  align-items: center;
  justify-content: space-between;
  width: 100%;
  max-width: 1200px;
  margin: 0 auto;
}

.carousel-text {
  color: #fff;
  
  h3 {
    font-size: 28px;
    font-weight: 600;
    margin-bottom: 12px;
    color: #fff;
  }
  
  p {
    font-size: 16px;
    opacity: 0.9;
    margin-bottom: 20px;
  }
  
  .arco-btn {
    background: rgba(255, 255, 255, 0.2);
    border: 1px solid rgba(255, 255, 255, 0.3);
    backdrop-filter: blur(4px);
    
    &:hover {
      background: rgba(255, 255, 255, 0.3);
    }
  }
}

.carousel-illustration {
  opacity: 0.8;
}

@media screen and (max-width: 768px) {
  .carousel-item {
    height: 200px;
    padding: 0 24px;
  }
  
  .carousel-text h3 {
    font-size: 20px;
  }
  
  .carousel-text p {
    font-size: 14px;
  }
  
  .carousel-illustration {
    display: none;
  }
}
</style>
