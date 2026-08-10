<template>
  <div class="blocklist-management">
    <!-- 搜索栏 -->
    <a-card class="mb-4" :bordered="false">
      <a-form :model="searchForm" layout="inline" auto-label-width>
        <a-form-item label="类型">
          <a-select v-model="searchForm.type" placeholder="请选择" allow-clear style="width: 120px">
            <a-option value="ip">IP地址</a-option>
            <a-option value="device">设备码</a-option>
            <a-option value="user">用户</a-option>
          </a-select>
        </a-form-item>
        <a-form-item label="值">
          <a-input v-model="searchForm.value" placeholder="请输入" allow-clear style="width: 200px" />
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
      <a-button type="primary" @click="handleAdd">
        <template #icon><icon-plus /></template>
        添加黑名单
      </a-button>
    </a-card>

    <!-- 数据表格 -->
    <a-card :bordered="false">
      <a-table :columns="columns" :data="tableData" :loading="loading" :pagination="pagination" row-key="id" @page-change="handlePageChange">
        <template #type="{ record }">
          <a-tag :color="record.type === 'ip' ? 'red' : record.type === 'device' ? 'orange' : 'purple'">
            {{ record.type === 'ip' ? 'IP地址' : record.type === 'device' ? '设备码' : '用户' }}
          </a-tag>
        </template>
        <template #actions="{ record }">
          <a-popconfirm content="确定从黑名单移除吗？" @ok="handleDelete(record)">
            <a-button type="text" size="small" status="danger">移除</a-button>
          </a-popconfirm>
        </template>
      </a-table>
    </a-card>

    <!-- 添加弹窗 -->
    <a-modal v-model:visible="modalVisible" title="添加黑名单" :width="480" @ok="handleSubmit" @cancel="handleCancel">
      <a-form ref="formRef" :model="form" :rules="rules" layout="vertical">
        <a-form-item field="type" label="类型" required>
          <a-select v-model="form.type" placeholder="请选择类型">
            <a-option value="ip">IP地址</a-option>
            <a-option value="device">设备码</a-option>
            <a-option value="user">用户</a-option>
          </a-select>
        </a-form-item>
        <a-form-item field="value" label="值" required>
          <a-input v-model="form.value" placeholder="请输入IP地址/设备码/用户名" />
        </a-form-item>
        <a-form-item field="reason" label="原因">
          <a-textarea v-model="form.reason" placeholder="请输入拉黑原因" />
        </a-form-item>
      </a-form>
    </a-modal>
  </div>
</template>

<script setup>
import { ref, reactive, onMounted } from 'vue'
import { Message } from '@arco-design/web-vue'
import blocklistApi from '@/api/system/blocklist'

const searchForm = reactive({ type: undefined, value: '' })
const tableData = ref([])
const loading = ref(false)
const pagination = reactive({ current: 1, pageSize: 20, total: 0, showTotal: true })

const modalVisible = ref(false)
const formRef = ref(null)
const form = reactive({ type: '', value: '', reason: '' })

const rules = {
  type: [{ required: true, message: '请选择类型' }],
  value: [{ required: true, message: '请输入值' }]
}

const columns = [
  { title: 'ID', dataIndex: 'id', width: 80 },
  { title: '类型', dataIndex: 'type', slotName: 'type' },
  { title: '值', dataIndex: 'value' },
  { title: '原因', dataIndex: 'reason' },
  { title: '添加时间', dataIndex: 'create_time', width: 160 },
  { title: '操作', slotName: 'actions', width: 80 }
]

const loadData = async () => {
  loading.value = true
  try {
    const res = await blocklistApi.getList({ ...searchForm, page: pagination.current, size: pagination.pageSize })
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
const handleReset = () => { Object.assign(searchForm, { type: undefined, value: '' }); handleSearch() }
const handlePageChange = (page) => { pagination.current = page; loadData() }

const handleAdd = () => {
  Object.assign(form, { type: '', value: '', reason: '' })
  modalVisible.value = true
}

const handleSubmit = async () => {
  const valid = await formRef.value?.validate()
  if (valid) return
  try {
    const res = await blocklistApi.add(form)
    if (res.code === 200) {
      Message.success('添加成功')
      modalVisible.value = false
      loadData()
    }
  } catch (e) { Message.error('操作失败') }
}

const handleCancel = () => { modalVisible.value = false }

const handleDelete = async (record) => {
  try {
    const res = await blocklistApi.del(record.id)
    if (res.code === 200) { Message.success('移除成功'); loadData() }
  } catch (e) { Message.error('操作失败') }
}

onMounted(() => { loadData() })
</script>

<script>
export default { name: 'Blocklist' }
</script>

<style scoped>
.blocklist-management { padding: 16px; }
</style>
