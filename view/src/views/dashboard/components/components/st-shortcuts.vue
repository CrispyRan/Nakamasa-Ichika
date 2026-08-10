<template>
  <div class="w-full ma-content-block rounded-sm p-3 mt-3">
    <div class="flex justify-between">
      快捷功能
      <a-link>更多</a-link>
    </div>
    <div class="grid grid-cols-2 sm:grid-cols-3 md:grid-cols-4 lg:grid-cols-6 gap-3 mt-3">
      <div
        v-for="item in shortcuts"
        :key="item.name"
        class="shortcut-item"
        @click="handleClick(item)"
      >
        <div class="shortcut-icon" :style="{ background: item.bgColor }">
          <component :is="item.icon" :size="24" :style="{ color: item.iconColor }" />
        </div>
        <div class="shortcut-title">{{ item.title }}</div>
      </div>
    </div>
  </div>
</template>

<script setup>
import { useRouter } from 'vue-router'

const router = useRouter()

const shortcuts = [
  {
    name: 'dataAnalysis',
    title: '分析页',
    icon: 'icon-line-chart',
    bgColor: 'linear-gradient(135deg, #667eea 0%, #764ba2 100%)',
    iconColor: '#fff',
    path: '/statistics/data-analysis'
  },
  {
    name: 'multiDimension',
    title: '多维数据分析',
    icon: 'icon-apps',
    bgColor: 'linear-gradient(135deg, #f093fb 0%, #f5576c 100%)',
    iconColor: '#fff',
    path: '/statistics/multi-dimension'
  },
  {
    name: 'statistics',
    title: '统计概览',
    icon: 'icon-bar-chart',
    bgColor: 'linear-gradient(135deg, #4facfe 0%, #00f2fe 100%)',
    iconColor: '#fff',
    path: '/statistics/index'
  },
  {
    name: 'user',
    title: '用户管理',
    icon: 'icon-user-group',
    bgColor: 'linear-gradient(135deg, #43e97b 0%, #38f9d7 100%)',
    iconColor: '#fff',
    path: '/user/list'
  },
  {
    name: 'kami',
    title: '卡密管理',
    icon: 'icon-code-square',
    bgColor: 'linear-gradient(135deg, #fa709a 0%, #fee140 100%)',
    iconColor: '#fff',
    path: '/kami/list'
  },
  {
    name: 'order',
    title: '订单管理',
    icon: 'icon-file',
    bgColor: 'linear-gradient(135deg, #a8edea 0%, #fed6e3 100%)',
    iconColor: '#666',
    path: '/order/list'
  }
]

const handleClick = (item) => {
  router.push(item.path)
}
</script>

<style scoped lang="less">
.shortcut-item {
  display: flex;
  flex-direction: column;
  align-items: center;
  padding: 16px 8px;
  border-radius: 8px;
  cursor: pointer;
  transition: all 0.3s ease;

  &:hover {
    background: var(--color-fill-2);
    transform: translateY(-2px);
  }
}

.shortcut-icon {
  width: 48px;
  height: 48px;
  display: flex;
  align-items: center;
  justify-content: center;
  border-radius: 12px;
  margin-bottom: 8px;
}

.shortcut-title {
  font-size: 13px;
  color: var(--color-text-1);
  text-align: center;
}
</style>
