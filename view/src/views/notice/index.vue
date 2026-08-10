<template>
  <div class="notice-management">
    <!-- 搜索栏 -->
    <a-card class="mb-4" :bordered="false">
      <a-form :model="searchForm" layout="inline" auto-label-width>
        <a-form-item label="标题">
          <a-input v-model="searchForm.title" placeholder="请输入标题" allow-clear style="width: 200px" />
        </a-form-item>
        <a-form-item label="状态">
          <a-select v-model="searchForm.status" placeholder="请选择" allow-clear style="width: 120px">
            <a-option :value="1">显示</a-option>
            <a-option :value="0">隐藏</a-option>
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
      <a-button type="primary" @click="handleAdd">
        <template #icon><icon-plus /></template>
        添加公告
      </a-button>
    </a-card>

    <!-- 数据表格 -->
    <a-card :bordered="false">
      <a-table :columns="columns" :data="tableData" :loading="loading" :pagination="pagination" row-key="id" @page-change="handlePageChange">
        <template #type="{ record }">
          <a-tag :color="record.type === 1 ? 'blue' : 'green'">
            {{ record.type === 1 ? '公告' : '通知' }}
          </a-tag>
        </template>
        <template #status="{ record }">
          <a-switch v-model="record.status" :checked-value="1" :unchecked-value="0" @change="handleStatusChange(record)" />
        </template>
        <template #actions="{ record }">
          <a-space>
            <a-button type="text" size="small" @click="handleEdit(record)">编辑</a-button>
            <a-popconfirm content="确定删除该公告吗？" @ok="handleDelete(record)">
              <a-button type="text" size="small" status="danger">删除</a-button>
            </a-popconfirm>
          </a-space>
        </template>
      </a-table>
    </a-card>

    <!-- 编辑弹窗 -->
    <a-modal v-model:visible="modalVisible" :title="modalTitle" :width="640" @ok="handleSubmit" @cancel="handleCancel">
      <a-form ref="formRef" :model="form" :rules="rules" layout="vertical">
        <a-form-item field="title" label="标题" required>
          <a-input v-model="form.title" placeholder="请输入标题" />
        </a-form-item>
        <a-row :gutter="16">
          <a-col :span="12">
            <a-form-item field="type" label="类型">
              <a-select v-model="form.type">
                <a-option :value="1">公告</a-option>
                <a-option :value="2">通知</a-option>
              </a-select>
            </a-form-item>
          </a-col>
          <a-col :span="12">
            <a-form-item field="status" label="状态">
              <a-radio-group v-model="form.status">
                <a-radio :value="1">显示</a-radio>
                <a-radio :value="0">隐藏</a-radio>
              </a-radio-group>
            </a-form-item>
          </a-col>
        </a-row>
        <a-form-item field="content" label="内容">
          <a-textarea v-model="form.content" placeholder="请输入公告内容" :auto-size="{ minRows: 4 }" />
        </a-form-item>
      </a-form>
    </a-modal>
  </div>
</template>

<script setup>
import { ref, reactive, computed, onMounted } from 'vue'
import { Message } from '@arco-design/web-vue'
import noticeApi from '@/api/system/notice'

const searchForm = reactive({ title: '', status: undefined })
const tableData = ref([])
const loading = ref(false)
const pagination = reactive({ current: 1, pageSize: 20, total: 0, showTotal: true })

const modalVisible = ref(false)
const modalTitle = computed(() => form.id ? '编辑公告' : '添加公告')
const formRef = ref(null)
const form = reactive({ id: '', title: '', type: 1, content: '', status: 1 })

const rules = { title: [{ required: true, message: '请输入标题' }] }

const columns = [
  { title: 'ID', dataIndex: 'id', width: 80 },
  { title: '标题', dataIndex: 'title' },
  { title: '类型', dataIndex: 'type', slotName: 'type', width: 100 },
  { title: '状态', dataIndex: 'status', slotName: 'status', width: 80 },
  { title: '创建时间', dataIndex: 'create_time', width: 160 },
  { title: '操作', slotName: 'actions', width: 120 }
]

const loadData = async () => {
  loading.value = true
  try {
    const res = await noticeApi.getList({ ...searchForm, page: pagination.current, size: pagination.pageSize })
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
const handleReset = () => { Object.assign(searchForm, { title: '', status: undefined }); handleSearch() }
const handlePageChange = (page) => { pagination.current = page; loadData() }

const handleAdd = () => {
  Object.assign(form, { id: '', title: '', type: 1, content: '', status: 1 })
  modalVisible.value = true
}

const handleEdit = (record) => { Object.assign(form, record); modalVisible.value = true }

const handleSubmit = async () => {
  const valid = await formRef.value?.validate()
  if (valid) return
  try {
    const api = form.id ? noticeApi.edit : noticeApi.add
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
    const res = await noticeApi.del(record.id)
    if (res.code === 200) { Message.success('删除成功'); loadData() }
  } catch (e) { Message.error('删除失败') }
}

const handleStatusChange = async (record) => {
  try {
    await noticeApi.edit({ id: record.id, status: record.status })
    Message.success('状态更新成功')
  } catch (e) { record.status = record.status === 1 ? 0 : 1; Message.error('操作失败') }
}

onMounted(() => { loadData() })
</script>

<script>
export default { name: 'NoticeList' }
</script>

<style scoped>
.notice-management { padding: 16px; }
</style>
