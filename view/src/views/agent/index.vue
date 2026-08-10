<template>
  <div class="agent-management">
    <!-- 搜索栏 -->
    <a-card class="mb-4" :bordered="false">
      <a-form :model="searchForm" layout="inline" auto-label-width>
        <a-form-item label="代理名称">
          <a-input v-model="searchForm.username" placeholder="请输入代理名称" allow-clear style="width: 180px" />
        </a-form-item>
        <a-form-item label="状态">
          <a-select v-model="searchForm.status" placeholder="请选择" allow-clear style="width: 120px">
            <a-option :value="1">正常</a-option>
            <a-option :value="0">禁用</a-option>
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

    <!-- 操作栏 -->
    <a-card class="mb-4" :bordered="false">
      <a-space>
        <a-button type="primary" @click="handleAdd">
          <template #icon><icon-plus /></template>
          添加代理
        </a-button>
      </a-space>
    </a-card>

    <!-- 数据表格 -->
    <a-card :bordered="false">
      <a-table :columns="columns" :data="tableData" :loading="loading" :pagination="pagination" row-key="id" @page-change="handlePageChange">
        <template #level="{ record }">
          <a-tag :color="record.level === 1 ? 'gold' : 'silver'">
            {{ record.level === 1 ? '一级代理' : '二级代理' }}
          </a-tag>
        </template>
        <template #balance="{ record }">
          <span class="text-red-500 font-medium">¥{{ record.balance.toFixed(2) }}</span>
        </template>
        <template #commission_rate="{ record }">
          <span>{{ (record.commission_rate * 100).toFixed(0) }}%</span>
        </template>
        <template #status="{ record }">
          <a-switch v-model="record.status" :checked-value="1" :unchecked-value="0" />
        </template>
        <template #actions="{ record }">
          <a-space>
            <a-button type="text" size="small" @click="handleEdit(record)">编辑</a-button>
            <a-button type="text" size="small" @click="handleRecharge(record)">充值</a-button>
            <a-popconfirm content="确定删除该代理吗？" @ok="handleDelete(record)">
              <a-button type="text" size="small" status="danger">删除</a-button>
            </a-popconfirm>
          </a-space>
        </template>
      </a-table>
    </a-card>

    <!-- 编辑弹窗 -->
    <a-modal v-model:visible="modalVisible" :title="modalTitle" :width="520" @ok="handleSubmit" @cancel="handleCancel">
      <a-form ref="formRef" :model="form" :rules="rules" layout="vertical">
        <a-row :gutter="16">
          <a-col :span="12">
            <a-form-item field="username" label="用户名" required>
              <a-input v-model="form.username" placeholder="请输入用户名" />
            </a-form-item>
          </a-col>
          <a-col :span="12">
            <a-form-item field="nickname" label="昵称">
              <a-input v-model="form.nickname" placeholder="请输入昵称" />
            </a-form-item>
          </a-col>
        </a-row>
        <a-row :gutter="16">
          <a-col :span="12">
            <a-form-item field="phone" label="手机号">
              <a-input v-model="form.phone" placeholder="请输入手机号" />
            </a-form-item>
          </a-col>
          <a-col :span="12">
            <a-form-item field="level" label="代理等级">
              <a-select v-model="form.level">
                <a-option :value="1">一级代理</a-option>
                <a-option :value="2">二级代理</a-option>
              </a-select>
            </a-form-item>
          </a-col>
        </a-row>
        <a-row :gutter="16">
          <a-col :span="12">
            <a-form-item field="commission_rate" label="佣金比例">
              <a-input-number v-model="form.commission_rate" :min="0" :max="1" :step="0.01" style="width: 100%">
                <template #suffix>%</template>
              </a-input-number>
            </a-form-item>
          </a-col>
          <a-col :span="12">
            <a-form-item field="status" label="状态">
              <a-radio-group v-model="form.status">
                <a-radio :value="1">启用</a-radio>
                <a-radio :value="0">禁用</a-radio>
              </a-radio-group>
            </a-form-item>
          </a-col>
        </a-row>
      </a-form>
    </a-modal>
  </div>
</template>

<script setup>
import { ref, reactive, computed, onMounted } from 'vue'
import { Message } from '@arco-design/web-vue'
import agentApi from '@/api/system/agent'

const searchForm = reactive({ username: '', status: undefined })
const tableData = ref([])
const loading = ref(false)
const pagination = reactive({ current: 1, pageSize: 20, total: 0, showTotal: true })

const modalVisible = ref(false)
const modalTitle = computed(() => form.id ? '编辑代理' : '添加代理')
const formRef = ref(null)
const form = reactive({
  id: '', username: '', nickname: '', phone: '', level: 1, commission_rate: 0.1, status: 1
})

const rules = { username: [{ required: true, message: '请输入用户名' }] }

const columns = [
  { title: 'ID', dataIndex: 'id', width: 80 },
  { title: '用户名', dataIndex: 'username' },
  { title: '昵称', dataIndex: 'nickname' },
  { title: '手机号', dataIndex: 'phone' },
  { title: '等级', dataIndex: 'level', slotName: 'level' },
  { title: '余额', dataIndex: 'balance', slotName: 'balance' },
  { title: '佣金比例', dataIndex: 'commission_rate', slotName: 'commission_rate' },
  { title: '状态', dataIndex: 'status', slotName: 'status', width: 80 },
  { title: '操作', slotName: 'actions', width: 180 }
]

const loadData = async () => {
  loading.value = true
  try {
    const res = await agentApi.getList({ ...searchForm, page: pagination.current, size: pagination.pageSize })
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
const handleReset = () => { Object.assign(searchForm, { username: '', status: undefined }); handleSearch() }
const handlePageChange = (page) => { pagination.current = page; loadData() }

const handleAdd = () => {
  Object.assign(form, { id: '', username: '', nickname: '', phone: '', level: 1, commission_rate: 0.1, status: 1 })
  modalVisible.value = true
}

const handleEdit = (record) => { Object.assign(form, record); modalVisible.value = true }

const handleSubmit = async () => {
  const valid = await formRef.value?.validate()
  if (valid) return
  try {
    const api = form.id ? agentApi.edit : agentApi.add
    const res = await api(form)
    if (res.code === 200) {
      Message.success(form.id ? '编辑成功' : '添加成功')
      modalVisible.value = false
      loadData()
    }
  } catch (e) { Message.error('操作失败') }
}

const handleCancel = () => { modalVisible.value = false }
const handleDelete = async (record) => {
  try {
    const res = await agentApi.del(record.id)
    if (res.code === 200) { Message.success('删除成功'); loadData() }
  } catch (e) { Message.error('删除失败') }
}

const handleRecharge = (record) => { Message.info('充值功能开发中') }

onMounted(() => { loadData() })
</script>

<script>
export default { name: 'AgentList' }
</script>

<style scoped>
.agent-management { padding: 16px; }
</style>
