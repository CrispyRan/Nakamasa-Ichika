<template>
  <div class="w-full mx-auto">
    <a-grid :cols="{ xs: 1, sm: 2, md: 4, lg: 4 }" :row-gap="16" :col-gap="16" class="panel ma-content-block mt-3 p-4">
      <!-- 用户统计 -->
      <a-grid-item class="panel-col">
        <a-space>
          <a-avatar :size="54" class="col-avatar" style="padding: 10px; background: linear-gradient(135deg, #667eea 0%, #764ba2 100%)">
            <icon-user-group :size="24" style="color: white" />
          </a-avatar>
          <a-statistic title="用户总数" :value="data.userCount" :value-from="0" animation show-group-separator>
            <template #suffix><span class="unit">人</span></template>
          </a-statistic>
        </a-space>
        <div class="stat-footer">
          <span class="online">
            <icon-check-circle-fill style="color: #52c41a" /> 在线: {{ data.onlineCount }}
          </span>
        </div>
      </a-grid-item>
      
      <!-- 订单统计 -->
      <a-grid-item class="panel-col">
        <a-space>
          <a-avatar :size="54" class="col-avatar" style="padding: 10px; background: linear-gradient(135deg, #f093fb 0%, #f5576c 100%)">
            <icon-file :size="24" style="color: white" />
          </a-avatar>
          <a-statistic title="订单总数" :value="data.orderCount" :value-from="0" animation show-group-separator>
            <template #suffix><span class="unit">笔</span></template>
          </a-statistic>
        </a-space>
        <div class="stat-footer">
          <span class="today">今日: {{ data.todayOrderCount }} 笔</span>
          <span class="rate" :class="data.todaySuccessRate >= 90 ? 'up' : 'down'">
            成功率: {{ (data.todaySuccessRate * 100).toFixed(1) }}%
          </span>
        </div>
      </a-grid-item>
      
      <!-- 金额统计 -->
      <a-grid-item class="panel-col">
        <a-space>
          <a-avatar :size="54" class="col-avatar" style="padding: 10px; background: linear-gradient(135deg, #4facfe 0%, #00f2fe 100%)">
            <icon-money-collect :size="24" style="color: white" />
          </a-avatar>
          <a-statistic title="总收入" :value="data.totalAmount" :value-from="0" animation show-group-separator :precision="2">
            <template #prefix>¥</template>
            <template #suffix><span class="unit">元</span></template>
          </a-statistic>
        </a-space>
        <div class="stat-footer">
          <span class="today">今日: ¥{{ data.todayAmount.toFixed(2) }}</span>
          <span class="yesterday">昨日: ¥{{ data.yesterdayAmount.toFixed(2) }}</span>
        </div>
      </a-grid-item>
      
      <!-- 卡密统计 -->
      <a-grid-item class="panel-col">
        <a-space>
          <a-avatar :size="54" class="col-avatar" style="padding: 10px; background: linear-gradient(135deg, #fa709a 0%, #fee140 100%)">
            <icon-code-square :size="24" style="color: white" />
          </a-avatar>
          <a-statistic title="卡密总数" :value="data.kamiCount" :value-from="0" animation show-group-separator>
            <template #suffix><span class="unit">张</span></template>
          </a-statistic>
        </a-space>
        <div class="stat-footer">
          <span class="used">已使用: {{ data.kamiUsedCount }}</span>
          <span class="unused">剩余: {{ data.kamiCount - data.kamiUsedCount }}</span>
        </div>
      </a-grid-item>
    </a-grid>
  </div>
</template>

<script setup>
import { ref, onMounted } from 'vue'
import statisticsApi from '@/api/system/statistics'

const data = ref({
  userCount: 0,
  onlineCount: 0,
  signInToday: 0,
  signInYesterday: 0,
  userCensus: [],
  
  orderCount: 0,
  totalAmount: 0,
  todayAmount: 0,
  yesterdayAmount: 0,
  todayOrderCount: 0,
  yesterdayOrderCount: 0,
  todaySuccessRate: 0,
  orderCensus: [],
  
  kamiCount: 0,
  kamiUsedCount: 0,
  kamiCensus: []
})

const loading = ref(false)

const getData = async () => {
  loading.value = true
  try {
    const res = await statisticsApi.getFormatted()
    if (res.code === 200 && res.data) {
      data.value = { ...data.value, ...res.data }
    }
  } catch (e) {
    console.error('获取统计数据失败:', e)
  } finally {
    loading.value = false
  }
}

onMounted(() => {
  getData()
})
</script>

<style scoped lang="less">
.arco-grid.panel {
  margin-bottom: 0;
  border-radius: 8px;
}

.panel-col {
  padding: 8px 16px;
  border-radius: 8px;
  transition: all 0.3s ease;
  
  &:hover {
    background: rgba(var(--primary-6), 0.05);
  }
}

.col-avatar {
  margin-right: 12px;
}

.unit {
  margin-left: 8px;
  color: rgb(var(--gray-8));
  font-size: 12px;
}

.stat-footer {
  margin-top: 8px;
  padding-top: 8px;
  border-top: 1px solid rgb(var(--gray-2));
  display: flex;
  justify-content: space-between;
  font-size: 12px;
  color: rgb(var(--gray-6));
  
  .online {
    color: #52c41a;
  }
  
  .today {
    color: rgb(var(--primary-6));
  }
  
  .yesterday {
    color: rgb(var(--gray-5));
  }
  
  .used {
    color: #faad14;
  }
  
  .unused {
    color: #52c41a;
  }
  
  .rate {
    &.up {
      color: #52c41a;
    }
    &.down {
      color: #f5222d;
    }
  }
}

:deep(.panel-border) {
  margin: 4px 0 0 0;
}
</style>

<!-- 皮肤适配样式 -->
<style lang="less">
[mine-skin="video"] {
  .arco-grid.panel,
  .panel.arco-grid {
    background: rgba(80, 80, 85, 0.35) !important;
    backdrop-filter: blur(6px) saturate(120%);
    -webkit-backdrop-filter: blur(6px) saturate(120%);
    border: 1px solid rgba(255, 255, 255, 0.1) !important;
  }
  
  .panel-col {
    border-right-color: rgba(255, 255, 255, 0.1) !important;
  }
  
  .arco-statistic-title {
    color: rgba(255, 255, 255, 0.85) !important;
  }
  
  .arco-statistic-value {
    color: #fff !important;
  }
  
  .unit {
    color: rgba(255, 255, 255, 0.6) !important;
  }
  
  .stat-footer {
    border-top-color: rgba(255, 255, 255, 0.1) !important;
    color: rgba(255, 255, 255, 0.6) !important;
  }
}

[mine-skin="mine"] {
  .arco-grid.panel,
  .panel.arco-grid {
    background: rgba(255, 255, 255, 0.4) !important;
    backdrop-filter: blur(6px) saturate(120%);
    -webkit-backdrop-filter: blur(6px) saturate(120%);
    border: 1px solid rgba(255, 255, 255, 0.5) !important;
  }
  
  .panel-col {
    border-right-color: rgba(0, 0, 0, 0.08) !important;
  }
}

[mine-skin="city"] {
  .arco-grid.panel,
  .panel.arco-grid {
    background: rgba(255, 255, 255, 0.2) !important;
    backdrop-filter: blur(8px) saturate(120%);
    -webkit-backdrop-filter: blur(8px) saturate(120%);
    border: 1px solid rgba(255, 255, 255, 0.3) !important;
  }
  
  .panel-col {
    border-right-color: rgba(255, 255, 255, 0.2) !important;
  }
  
  .arco-statistic-title {
    color: rgba(255, 255, 255, 0.85) !important;
  }
  
  .arco-statistic-value {
    color: #fff !important;
  }
  
  .unit {
    color: rgba(255, 255, 255, 0.6) !important;
  }
  
  .stat-footer {
    border-top-color: rgba(255, 255, 255, 0.2) !important;
    color: rgba(255, 255, 255, 0.6) !important;
  }
}

[mine-skin="classics"],
[mine-skin="businessGray"] {
  .arco-grid.panel,
  .panel.arco-grid {
    background: rgba(50, 52, 56, 0.35) !important;
    backdrop-filter: blur(6px) saturate(120%);
    -webkit-backdrop-filter: blur(6px) saturate(120%);
    border: 1px solid rgba(255, 255, 255, 0.08) !important;
  }
  
  .panel-col {
    border-right-color: rgba(255, 255, 255, 0.08) !important;
  }
}
</style>