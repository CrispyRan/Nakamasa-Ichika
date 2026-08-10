<template>
  <div class="api-code">
    <!-- 操作栏 -->
    <a-card class="mb-4" :bordered="false">
      <a-space>
        <a-button type="primary" @click="handleAdd">
          <template #icon><icon-plus /></template>
          添加代码
        </a-button>
        <a-button @click="loadData">
          <template #icon><icon-refresh /></template>
          刷新
        </a-button>
      </a-space>
    </a-card>

    <!-- 数据表格 -->
    <a-card :bordered="false">
      <a-table :columns="columns" :data="tableData" :loading="loading" row-key="id">
        <template #status="{ record }">
          <a-switch v-model="record.status" :checked-value="1" :unchecked-value="0" @change="handleStatusChange(record)" />
        </template>
        <template #actions="{ record }">
          <a-space>
            <a-button type="text" size="small" @click="handleEdit(record)">编辑</a-button>
            <a-button type="text" size="small" @click="handleViewCode(record)">查看代码</a-button>
            <a-popconfirm content="确定删除吗？" @ok="handleDelete(record)">
              <a-button type="text" size="small" status="danger">删除</a-button>
            </a-popconfirm>
          </a-space>
        </template>
      </a-table>
    </a-card>

    <!-- 编辑弹窗 -->
    <a-modal v-model:visible="modalVisible" :title="modalTitle" :width="700" @ok="handleSubmit">
      <a-form ref="formRef" :model="form" :rules="rules" layout="vertical">
        <a-form-item field="name" label="代码名称" required>
          <a-input v-model="form.name" placeholder="请输入代码名称" />
        </a-form-item>
        <a-form-item field="code" label="代码内容">
          <a-textarea v-model="form.code" placeholder="请输入代码内容" :auto-size="{ minRows: 10, maxRows: 20 }" class="font-mono" />
        </a-form-item>
        <a-form-item field="description" label="描述">
          <a-textarea v-model="form.description" placeholder="请输入描述" />
        </a-form-item>
        <a-form-item field="status" label="状态">
          <a-radio-group v-model="form.status">
            <a-radio :value="1">启用</a-radio>
            <a-radio :value="0">禁用</a-radio>
          </a-radio-group>
        </a-form-item>
      </a-form>
    </a-modal>

    <!-- 查看代码弹窗 -->
    <a-modal v-model:visible="codeVisible" title="代码预览" :width="600" :footer="false">
      <pre class="code-preview">{{ currentCode }}</pre>
    </a-modal>
  </div>
</template>

<script setup>
import { ref, reactive, computed, onMounted } from 'vue'
import { Message } from '@arco-design/web-vue'
import systemApi from '@/api/system/system'

const tableData = ref([])
const loading = ref(false)

const modalVisible = ref(false)
const modalTitle = computed(() => form.id ? '编辑代码' : '添加代码')
const formRef = ref(null)
const form = reactive({ id: '', name: '', code: '', description: '', status: 1 })

const rules = { name: [{ required: true, message: '请输入代码名称' }] }

const codeVisible = ref(false)
const currentCode = ref('')

const columns = [
  { title: 'ID', dataIndex: 'id', width: 80 },
  { title: '代码名称', dataIndex: 'name' },
  { title: '描述', dataIndex: 'description' },
  { title: '状态', dataIndex: 'status', slotName: 'status', width: 80 },
  { title: '更新时间', dataIndex: 'update_time', width: 160 },
  { title: '操作', slotName: 'actions', width: 180 }
]

const loadData = async () => {
  loading.value = true
  try {
    const res = await systemApi.getUserApiCode()
    if (res.code === 200) tableData.value = res.data || []
  } finally {
    loading.value = false
  }
}

const handleAdd = () => {
  Object.assign(form, { id: '', name: '', code: '', description: '', status: 1 })
  modalVisible.value = true
}

const handleEdit = (record) => { Object.assign(form, record); modalVisible.value = true }

const handleViewCode = (record) => {
  currentCode.value = record.code || ''
  codeVisible.value = true
}

const handleSubmit = async () => {
  const valid = await formRef.value?.validate()
  if (valid) return
  try {
    const res = await systemApi.editUserApiCode(form)
    if (res.code === 200) { Message.success('操作成功'); modalVisible.value = false; loadData() }
  } catch (e) { Message.error('操作失败') }
}

const handleDelete = async (record) => {
  try {
    await systemApi.editUserApiCode({ id: record.id, _delete: true })
    Message.success('删除成功')
    loadData()
  } catch (e) { Message.error('删除失败') }
}

const handleStatusChange = async (record) => {
  try {
    await systemApi.switchUserApiCode({ id: record.id, status: record.status })
    Message.success('状态更新成功')
  } catch (e) { record.status = record.status === 1 ? 0 : 1; Message.error('操作失败') }
}

onMounted(() => { loadData() })
</script>

<script>
export default { name: 'ApiCode' }
</script>

<style scoped>
.api-code { padding: 16px; }
.code-preview {
  background: #1e1e1e;
  color: #d4d4d4;
  padding: 16px;
  border-radius: 4px;
  overflow-x: auto;
  font-family: 'Fira Code', monospace;
  font-size: 13px;
}
</style>
