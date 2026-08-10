<template>
  <div class="extend-management">
    <!-- 操作栏 -->
    <a-card class="mb-4" :bordered="false">
      <a-button type="primary" @click="handleAdd">
        <template #icon><icon-plus /></template>
        添加扩展字段
      </a-button>
    </a-card>

    <!-- 数据表格 -->
    <a-card :bordered="false">
      <a-table :columns="columns" :data="tableData" :loading="loading" row-key="id">
        <template #type="{ record }">
          <a-tag :color="getFieldTypeColor(record.type)">{{ getFieldType(record.type) }}</a-tag>
        </template>
        <template #required="{ record }">
          <a-tag :color="record.required ? 'red' : 'gray'">{{ record.required ? '必填' : '选填' }}</a-tag>
        </template>
        <template #actions="{ record }">
          <a-space>
            <a-button type="text" size="small" @click="handleEdit(record)">编辑</a-button>
            <a-popconfirm content="确定删除该字段吗？" @ok="handleDelete(record)">
              <a-button type="text" size="small" status="danger">删除</a-button>
            </a-popconfirm>
          </a-space>
        </template>
      </a-table>
    </a-card>

    <!-- 编辑弹窗 -->
    <a-modal v-model:visible="modalVisible" :title="modalTitle" :width="520" @ok="handleSubmit">
      <a-form ref="formRef" :model="form" :rules="rules" layout="vertical">
        <a-form-item field="name" label="字段名称" required>
          <a-input v-model="form.name" placeholder="请输入字段名称" />
        </a-form-item>
        <a-form-item field="field" label="字段标识" required>
          <a-input v-model="form.field" placeholder="请输入字段标识（英文）" />
        </a-form-item>
        <a-row :gutter="16">
          <a-col :span="12">
            <a-form-item field="type" label="字段类型">
              <a-select v-model="form.type">
                <a-option value="text">文本</a-option>
                <a-option value="number">数字</a-option>
                <a-option value="select">下拉选择</a-option>
                <a-option value="textarea">多行文本</a-option>
                <a-option value="date">日期</a-option>
              </a-select>
            </a-form-item>
          </a-col>
          <a-col :span="12">
            <a-form-item field="required" label="是否必填">
              <a-switch v-model="form.required" />
            </a-form-item>
          </a-col>
        </a-row>
        <a-form-item field="options" label="选项值" v-if="form.type === 'select'">
          <a-textarea v-model="form.options" placeholder="每行一个选项" :auto-size="{ minRows: 3 }" />
        </a-form-item>
        <a-form-item field="default" label="默认值">
          <a-input v-model="form.default" placeholder="请输入默认值" />
        </a-form-item>
      </a-form>
    </a-modal>
  </div>
</template>

<script setup>
import { ref, reactive, computed, onMounted } from 'vue'
import { Message } from '@arco-design/web-vue'
import extendApi from '@/api/system/extend'

const tableData = ref([])
const loading = ref(false)

const modalVisible = ref(false)
const modalTitle = computed(() => form.id ? '编辑字段' : '添加字段')
const formRef = ref(null)
const form = reactive({ id: '', name: '', field: '', type: 'text', required: false, options: '', default: '' })

const rules = {
  name: [{ required: true, message: '请输入字段名称' }],
  field: [{ required: true, message: '请输入字段标识' }]
}

const columns = [
  { title: 'ID', dataIndex: 'id', width: 80 },
  { title: '字段名称', dataIndex: 'name' },
  { title: '字段标识', dataIndex: 'field' },
  { title: '字段类型', dataIndex: 'type', slotName: 'type' },
  { title: '是否必填', dataIndex: 'required', slotName: 'required' },
  { title: '默认值', dataIndex: 'default' },
  { title: '操作', slotName: 'actions', width: 120 }
]

const fieldTypes = { text: '文本', number: '数字', select: '下拉选择', textarea: '多行文本', date: '日期' }
const fieldTypeColors = { text: 'blue', number: 'green', select: 'orange', textarea: 'purple', date: 'cyan' }

const getFieldType = (type) => fieldTypes[type] || type
const getFieldTypeColor = (type) => fieldTypeColors[type] || 'gray'

const loadData = async () => {
  loading.value = true
  try {
    const res = await extendApi.getList({ page: 1, size: 100 })
    if (res.code === 200) {
      // 后端返回格式: { list, currentPage, pageTotal, dataTotal }
      tableData.value = res.data.list || []
    }
  } finally {
    loading.value = false
  }
}

const handleAdd = () => {
  Object.assign(form, { id: '', name: '', field: '', type: 'text', required: false, options: '', default: '' })
  modalVisible.value = true
}

const handleEdit = (record) => { Object.assign(form, record); modalVisible.value = true }

const handleSubmit = async () => {
  const valid = await formRef.value?.validate()
  if (valid) return
  try {
    const api = form.id ? extendApi.edit : extendApi.add
    const res = await api(form)
    if (res.code === 200) { Message.success('操作成功'); modalVisible.value = false; loadData() }
  } catch (e) { Message.error('操作失败') }
}

const handleDelete = async (record) => {
  try {
    const res = await extendApi.del(record.id)
    if (res.code === 200) { Message.success('删除成功'); loadData() }
  } catch (e) { Message.error('删除失败') }
}

onMounted(() => { loadData() })
</script>

<script>
export default { name: 'ExtendList' }
</script>

<style scoped>
.extend-management { padding: 16px; }
</style>
