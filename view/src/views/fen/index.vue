<template>
  <div class="fen-management">
    <!-- 标签页 -->
    <a-tabs v-model:active-key="activeTab">
      <a-tab-pane key="event" title="积分事件">
        <a-card class="mb-4" :bordered="false">
          <a-space>
            <a-button type="primary" @click="handleAddEvent">
              <template #icon><icon-plus /></template>
              创建事件
            </a-button>
            <a-button status="danger" :disabled="selectedKeys.length === 0" @click="handleDeleteEventBatch">
              批量删除
            </a-button>
          </a-space>
        </a-card>
        <a-card :bordered="false">
          <a-table 
            :columns="eventColumns" 
            :data="eventData" 
            :loading="eventLoading" 
            :pagination="eventPagination"
            row-key="id"
            :row-selection="rowSelection"
            @page-change="onEventPageChange"
          >
            <template #vip="{ record }">
              <span>{{ formatVip(record.vip) }}</span>
            </template>
            <template #vip_free="{ record }">
              <div style="display: flex; justify-content: center; align-items: center;">
                <span v-if="record.vip_free === 'y'" style="color: #52c41a; font-size: 16px;">✓</span>
                <span v-else style="color: #f5222d; font-size: 16px;">×</span>
              </div>
            </template>
            <template #state="{ record }">
              <a-switch 
                :model-value="record.state === 'on'" 
                @change="(val) => handleEventStateChange(record, val)"
              />
            </template>
            <template #actions="{ record }">
              <a-space>
                <a-button type="text" size="small" @click="handleEditEvent(record)">编辑</a-button>
                <a-popconfirm content="确定删除吗？" @ok="handleDeleteEvent(record)">
                  <a-button type="text" size="small" status="danger">删除</a-button>
                </a-popconfirm>
              </a-space>
            </template>
          </a-table>
        </a-card>
      </a-tab-pane>

      <a-tab-pane key="order" title="积分订单">
        <a-card :bordered="false">
          <a-table 
            :columns="orderColumns" 
            :data="orderData" 
            :loading="orderLoading" 
            :pagination="orderPagination" 
            row-key="id" 
            @page-change="onOrderPageChange"
          >
            <template #type="{ record }">
              <a-tag :color="record.type === 1 ? 'green' : 'red'">
                {{ record.type === 1 ? '收入' : '支出' }}
              </a-tag>
            </template>
            <template #amount="{ record }">
              <span :class="record.type === 1 ? 'text-green-500' : 'text-red-500'">
                {{ record.type === 1 ? '+' : '-' }}{{ record.amount }}
              </span>
            </template>
          </a-table>
        </a-card>
      </a-tab-pane>
    </a-tabs>

    <!-- 事件编辑弹窗 -->
    <a-modal 
      v-model:visible="eventModalVisible" 
      :title="eventModalTitle" 
      :width="480"
      :footer="false"
      :mask-closable="false"
      title-align="start"
    >
      <a-form 
        ref="eventFormRef" 
        :model="eventForm" 
        :rules="eventRules" 
        layout="horizontal"
        :auto-label-width="true"
        @submit="handleEventSubmit"
      >
        <a-form-item field="name" label="事件名称">
          <a-input v-model="eventForm.name" placeholder="如：付费点播" />
        </a-form-item>
        
        <a-form-item field="type" label="事件类型">
          <a-radio-group v-model="eventForm.type" :disabled="eventForm.typeDisabled">
            <a-radio value="fen">消耗积分</a-radio>
            <a-radio value="vip">兑换会员</a-radio>
          </a-radio-group>
        </a-form-item>
        
        <a-form-item field="fen" label="消耗积分">
          <a-input-number 
            v-model="eventForm.fen" 
            :min="1" 
            placeholder="100"
            style="width: 100%" 
          >
            <template #append>
              <span>积分</span>
            </template>
          </a-input-number>
        </a-form-item>
        
        <!-- 兑换会员时显示会员值输入 -->
        <a-form-item v-if="eventForm.type === 'vip'" field="vip" label="会员值">
          <a-input-number 
            v-model="eventForm.vip" 
            :min="1" 
            placeholder="600"
            style="width: 100%" 
          >
            <template #append>
              <a-select v-model="eventForm.vipType" style="width: 65px">
                <a-option value="s">秒</a-option>
                <a-option value="i">分</a-option>
                <a-option value="h">时</a-option>
                <a-option value="d">天</a-option>
              </a-select>
            </template>
          </a-input-number>
        </a-form-item>
        
        <!-- 消耗积分时显示会员免费选项 -->
        <a-form-item v-if="eventForm.type === 'fen'" field="vip_free" label="会员免费">
          <a-radio-group v-model="eventForm.vip_free">
            <a-radio value="y">是</a-radio>
            <a-radio value="n">否</a-radio>
          </a-radio-group>
        </a-form-item>
        
        <a-form-item>
          <a-space direction="vertical" fill>
            <a-button 
              type="primary" 
              html-type="submit" 
              :loading="eventForm.btnLoading" 
              long
            >
              提交
            </a-button>
          </a-space>
        </a-form-item>
      </a-form>
    </a-modal>
  </div>
</template>

<script setup>
import { ref, reactive, computed, onMounted } from 'vue'
import { Message } from '@arco-design/web-vue'
import fenApi from '@/api/system/fen'

const activeTab = ref('event')

// 积分事件
const eventData = ref([])
const eventLoading = ref(false)
const eventModalVisible = ref(false)
const eventFormRef = ref(null)
const selectedKeys = ref([])

// 表单数据
const eventForm = reactive({ 
  id: null, 
  name: '', 
  type: 'fen',
  fen: 1, 
  vip: null,
  vipType: 'd',
  vip_free: 'n',
  typeDisabled: false,
  btnLoading: false
})

const eventRules = {
  name: [
    { required: true, message: '请输入事件名称' },
    { minLength: 2, message: '事件名称至少2个字符' },
    { maxLength: 125, message: '事件名称最多125个字符' }
  ],
  fen: [
    { required: true, message: '请输入消耗积分' }
  ]
}

const eventPagination = computed(() => ({
  current: eventPage.current,
  pageSize: eventPage.size,
  total: eventPage.total,
  showTotal: true
}))

const eventPage = reactive({ current: 1, size: 20, total: 0 })

const rowSelection = computed(() => ({
  type: 'checkbox',
  showCheckedAll: true,
  selectedRowKeys: selectedKeys.value
}))

const eventColumns = [
  { title: 'ID', dataIndex: 'id', width: 80 },
  { title: '事件名称', dataIndex: 'name' },
  { title: '消耗积分', dataIndex: 'fen', width: 100 },
  { title: '兑换会员', dataIndex: 'vip', slotName: 'vip', width: 120 },
  { title: '会员免费', dataIndex: 'vip_free', slotName: 'vip_free', width: 100 },
  { title: '状态', dataIndex: 'state', slotName: 'state', width: 80 },
  { title: '操作', slotName: 'actions', width: 120 }
]

const eventModalTitle = computed(() => eventForm.id ? '编辑事件' : '创建事件')

// 积分订单
const orderData = ref([])
const orderLoading = ref(false)
const orderPage = reactive({ current: 1, size: 20, total: 0 })

const orderPagination = computed(() => ({
  current: orderPage.current,
  pageSize: orderPage.size,
  total: orderPage.total,
  showTotal: true
}))

const orderColumns = [
  { title: 'ID', dataIndex: 'id', width: 80 },
  { title: '用户', dataIndex: 'username' },
  { title: '类型', dataIndex: 'type', slotName: 'type' },
  { title: '积分', dataIndex: 'amount', slotName: 'amount' },
  { title: '事件', dataIndex: 'event_name' },
  { title: '余额', dataIndex: 'balance' },
  { title: '时间', dataIndex: 'create_time', width: 160 }
]

// 格式化会员值显示
function formatVip(vip) {
  if (!vip) return '-'
  // 假设后端返回的是秒数，转换为可读格式
  if (vip >= 86400) {
    return Math.floor(vip / 86400) + '天'
  } else if (vip >= 3600) {
    return Math.floor(vip / 3600) + '小时'
  } else if (vip >= 60) {
    return Math.floor(vip / 60) + '分钟'
  }
  return vip + '秒'
}

// 将会员值转换为秒
function vipToSeconds(value, type) {
  if (!value) return 0
  switch (type) {
    case 's': return value
    case 'i': return value * 60
    case 'h': return value * 3600
    case 'd': return value * 86400
    default: return value
  }
}

// 加载事件列表
async function loadEventData() {
  eventLoading.value = true
  try {
    const res = await fenApi.getEventList({ 
      page: eventPage.current, 
      size: eventPage.size 
    })
    if (res.code === 200) {
      eventData.value = res.data.list || []
      eventPage.total = res.data.dataTotal || 0
    }
  } finally {
    eventLoading.value = false
  }
}

// 加载订单列表
async function loadOrderData() {
  orderLoading.value = true
  try {
    const res = await fenApi.getOrderList({ 
      page: orderPage.current, 
      size: orderPage.size 
    })
    if (res.code === 200) {
      orderData.value = res.data.list || []
      orderPage.total = res.data.dataTotal || 0
    }
  } finally {
    orderLoading.value = false
  }
}

function resetEventForm() {
  eventForm.id = null
  eventForm.name = ''
  eventForm.type = 'fen'
  eventForm.fen = 1
  eventForm.vip = null
  eventForm.vipType = 'd'
  eventForm.vip_free = 'n'
  eventForm.typeDisabled = false
  eventForm.btnLoading = false
}

function handleAddEvent() {
  resetEventForm()
  eventModalVisible.value = true
}

function handleEditEvent(record) {
  eventForm.id = record.id
  eventForm.name = record.name
  eventForm.fen = record.fen
  eventForm.vip_free = record.vip_free || 'n'
  eventForm.typeDisabled = false
  eventForm.btnLoading = false
  
  // 根据 vip 值判断类型
  if (record.vip && record.vip > 0) {
    eventForm.type = 'vip'
    eventForm.vip = record.vip
    eventForm.vipType = 'd'
  } else {
    eventForm.type = 'fen'
    eventForm.vip = null
  }
  
  eventModalVisible.value = true
}

async function handleEventSubmit() {
  try {
    const valid = await eventFormRef.value?.validate()
    if (valid) return
    
    eventForm.btnLoading = true
    
    const submitData = {
      name: eventForm.name,
      fen: eventForm.fen,
      vip_free: eventForm.vip_free
    }
    
    // 根据类型处理 vip 字段
    if (eventForm.type === 'vip' && eventForm.vip) {
      submitData.vip = String(vipToSeconds(eventForm.vip, eventForm.vipType))
    }
    
    if (eventForm.id) {
      submitData.id = eventForm.id
    }
    
    const api = eventForm.id ? fenApi.editEvent : fenApi.addEvent
    const res = await api(submitData)
    
    if (res.code === 200) {
      Message.success('操作成功')
      eventModalVisible.value = false
      resetEventForm()
      loadEventData()
    } else {
      Message.error(res.msg || '操作失败')
    }
  } catch (e) {
    Message.error('操作失败：' + (e.message || e))
  } finally {
    eventForm.btnLoading = false
  }
}

async function handleEventStateChange(record, val) {
  const oldState = record.state
  const newState = val ? 'on' : 'off'
  try {
    const res = await fenApi.editEventState({ id: record.id, state: newState })
    if (res.code === 200) {
      Message.success('状态更新成功')
      record.state = newState
    } else {
      // 恢复原状态
      record.state = oldState
      Message.error(res.msg || '操作失败')
    }
  } catch (e) {
    record.state = oldState
    Message.error('出错了：' + (e.message || e))
  }
}

async function handleDeleteEvent(record) {
  try {
    const res = await fenApi.delEvent(record.id)
    if (res.code === 200) {
      Message.success('删除成功')
      loadEventData()
    } else {
      Message.error(res.msg || '删除失败')
    }
  } catch (e) {
    Message.error('删除失败')
  }
}

async function handleDeleteEventBatch() {
  if (selectedKeys.value.length === 0) return
  try {
    const res = await fenApi.delEventAll(selectedKeys.value)
    if (res.code === 200) {
      Message.success('批量删除成功')
      selectedKeys.value = []
      loadEventData()
    } else {
      Message.error(res.msg || '删除失败')
    }
  } catch (e) {
    Message.error('删除失败')
  }
}

function onEventPageChange(page) {
  eventPage.current = page
  loadEventData()
}

function onOrderPageChange(page) {
  orderPage.current = page
  loadOrderData()
}

onMounted(() => {
  loadEventData()
  loadOrderData()
})
</script>

<script>
export default { name: 'FenManagement' }
</script>

<style scoped>
.fen-management { padding: 16px; }
.mb-4 { margin-bottom: 16px; }
</style>