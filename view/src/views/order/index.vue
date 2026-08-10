<template>
  <div class="order-management">
    <!-- 统计卡片 -->
    <a-row :gutter="16" class="mb-4">
      <a-col :span="6">
        <a-card :bordered="false">
          <a-statistic title="订单总数" :value="stats.total" show-group-separator>
            <template #prefix><icon-file /></template>
          </a-statistic>
        </a-card>
      </a-col>
      <a-col :span="6">
        <a-card :bordered="false">
          <a-statistic title="今日订单" :value="stats.today" show-group-separator>
            <template #prefix><icon-calendar /></template>
          </a-statistic>
        </a-card>
      </a-col>
      <a-col :span="6">
        <a-card :bordered="false">
          <a-statistic title="累计金额" :value="stats.amount" :precision="2">
            <template #prefix>¥</template>
          </a-statistic>
        </a-card>
      </a-col>
      <a-col :span="6">
        <a-card :bordered="false">
          <a-statistic title="今日金额" :value="stats.todayAmount" :precision="2">
            <template #prefix>¥</template>
          </a-statistic>
        </a-card>
      </a-col>
    </a-row>

    <!-- 搜索栏 -->
    <a-card class="mb-4" :bordered="false">
      <a-form :model="searchForm" layout="inline" auto-label-width>
        <a-form-item label="订单号">
          <a-input v-model="searchForm.order_no" placeholder="请输入订单号" allow-clear style="width: 200px" />
        </a-form-item>
        <a-form-item label="支付方式">
          <a-select v-model="searchForm.pay_type" placeholder="请选择" allow-clear style="width: 120px">
            <a-option value="alipay">支付宝</a-option>
            <a-option value="wechat">微信</a-option>
          </a-select>
        </a-form-item>
        <a-form-item label="状态">
          <a-select v-model="searchForm.status" placeholder="请选择" allow-clear style="width: 120px">
            <a-option :value="0">待支付</a-option>
            <a-option :value="1">已支付</a-option>
            <a-option :value="-1">已关闭</a-option>
          </a-select>
        </a-form-item>
        <a-form-item>
          <a-space>
            <a-button type="primary" @click="handleSearch">搜索</a-button>
            <a-button @click="handleReset">重置</a-button>
          </a-space>
        </a-form-item>
      </a-form>
    </a-card>

    <!-- 数据表格 -->
    <a-card :bordered="false">
      <a-table :columns="columns" :data="tableData" :loading="loading" :pagination="pagination" row-key="id" @page-change="handlePageChange">
        <template #order_no="{ record }">
          <span class="font-mono">{{ record.order_no }}</span>
        </template>
        <template #amount="{ record }">
          <span class="text-red-500 font-medium">¥{{ record.amount.toFixed(2) }}</span>
        </template>
        <template #pay_type="{ record }">
          <a-tag :color="record.pay_type === 'alipay' ? 'blue' : 'green'">
            {{ record.pay_type === 'alipay' ? '支付宝' : '微信' }}
          </a-tag>
        </template>
        <template #status="{ record }">
          <a-tag v-if="record.status === 0" color="orange">待支付</a-tag>
          <a-tag v-else-if="record.status === 1" color="green">已支付</a-tag>
          <a-tag v-else color="gray">已关闭</a-tag>
        </template>
        <template #actions="{ record }">
          <a-space>
            <a-button type="text" size="small" @click="handleDetail(record)">详情</a-button>
            <a-button v-if="record.status === 0" type="text" size="small" status="warning" @click="handleClose(record)">关闭</a-button>
            <a-button v-if="record.status === 1" type="text" size="small" status="danger" @click="handleRefund(record)">退款</a-button>
          </a-space>
        </template>
      </a-table>
    </a-card>
  </div>
</template>

<script setup>
import { ref, reactive, onMounted } from 'vue'
import { Message } from '@arco-design/web-vue'
import orderApi from '@/api/system/order'

const stats = reactive({ total: 0, today: 0, amount: 0, todayAmount: 0 })
const searchForm = reactive({ order_no: '', pay_type: undefined, status: undefined })
const tableData = ref([])
const loading = ref(false)
const pagination = reactive({ current: 1, pageSize: 20, total: 0, showTotal: true })

const columns = [
  { title: '订单号', dataIndex: 'order_no', slotName: 'order_no' },
  { title: '用户', dataIndex: 'user_id' },
  { title: '金额', dataIndex: 'amount', slotName: 'amount' },
  { title: '支付方式', dataIndex: 'pay_type', slotName: 'pay_type' },
  { title: '状态', dataIndex: 'status', slotName: 'status' },
  { title: '创建时间', dataIndex: 'create_time', width: 160 },
  { title: '操作', slotName: 'actions', width: 150 }
]

const loadStats = async () => {
  try {
    const res = await orderApi.statistics('today')
    if (res.code === 200 && res.data) {
      // 后端返回格式: { count: { total, success_total }, money: { total, ali_total, wx_total } }
      const { count, money } = res.data
      stats.total = count?.total || 0
      stats.today = count?.success_total || 0
      stats.amount = money?.total || 0
      stats.todayAmount = (money?.ali_total || 0) + (money?.wx_total || 0)
    }
  } catch (e) {}
}

const loadData = async () => {
  loading.value = true
  try {
    const res = await orderApi.getList({ ...searchForm, page: pagination.current, size: pagination.pageSize })
    if (res.code === 200) {
      // 后端返回格式: { list, currentPage, pageTotal, dataTotal }
      tableData.value = res.data.list || []
      pagination.total = res.data.dataTotal || 0
    }
  } finally {
    loading.value = false
  }
}

const handleSearch = () => { pagination.current = 1; loadData() }
const handleReset = () => { Object.assign(searchForm, { order_no: '', pay_type: undefined, status: undefined }); handleSearch() }
const handlePageChange = (page) => { pagination.current = page; loadData() }

const handleDetail = (record) => { Message.info('订单详情功能开发中') }
const handleClose = async (record) => {
  try {
    const res = await orderApi.close(record.id)
    if (res.code === 200) { Message.success('订单已关闭'); loadData() }
  } catch (e) { Message.error('操作失败') }
}

const handleRefund = async (record) => {
  try {
    const res = await orderApi.refund({ id: record.id })
    if (res.code === 200) { Message.success('退款成功'); loadData() }
  } catch (e) { Message.error('退款失败') }
}

onMounted(() => { loadStats(); loadData() })
</script>

<script>
export default { name: 'OrderList' }
</script>

<style scoped>
.order-management { padding: 16px; }
</style>
