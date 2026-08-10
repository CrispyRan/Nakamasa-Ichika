<template>
  <div class="goods-management">
    <!-- 搜索栏 -->
    <a-card class="mb-4" :bordered="false">
      <a-form :model="searchForm" layout="inline" auto-label-width>
        <a-form-item label="商品名称">
          <a-input v-model="searchForm.name" placeholder="请输入商品名称" allow-clear style="width: 180px" />
        </a-form-item>
        <a-form-item label="类型">
          <a-select v-model="searchForm.type" placeholder="请选择" allow-clear style="width: 120px">
            <a-option value="vip">VIP</a-option>
            <a-option value="fen">积分</a-option>
            <a-option value="agent">代理</a-option>
          </a-select>
        </a-form-item>
        <a-form-item label="状态">
          <a-select v-model="searchForm.status" placeholder="请选择" allow-clear style="width: 100px">
            <a-option :value="1">上架</a-option>
            <a-option :value="0">下架</a-option>
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
        添加商品
      </a-button>
    </a-card>

    <!-- 数据表格 -->
    <a-card :bordered="false">
      <a-table :columns="columns" :data="tableData" :loading="loading" :pagination="pagination" row-key="id" @page-change="handlePageChange">
        <template #type="{ record }">
          <a-tag :color="record.type === 'vip' ? 'gold' : record.type === 'fen' ? 'blue' : 'purple'">
            {{ record.type === 'vip' ? 'VIP' : record.type === 'fen' ? '积分' : '代理' }}
          </a-tag>
        </template>
        <template #price="{ record }">
          <span class="text-red-500 font-medium">¥{{ record.price }}</span>
        </template>
        <template #status="{ record }">
          <a-switch v-model="record.status" :checked-value="1" :unchecked-value="0" @change="handleStatusChange(record)" />
        </template>
        <template #actions="{ record }">
          <a-space>
            <a-button type="text" size="small" @click="handleEdit(record)">编辑</a-button>
            <a-popconfirm content="确定删除该商品吗？" @ok="handleDelete(record)">
              <a-button type="text" size="small" status="danger">删除</a-button>
            </a-popconfirm>
          </a-space>
        </template>
      </a-table>
    </a-card>

    <!-- 编辑弹窗 -->
    <a-modal v-model:visible="modalVisible" :title="modalTitle" :width="560" @ok="handleSubmit" @cancel="handleCancel">
      <a-form ref="formRef" :model="form" :rules="rules" layout="vertical">
        <a-form-item field="name" label="商品名称" required>
          <a-input v-model="form.name" placeholder="请输入商品名称" />
        </a-form-item>
        <a-row :gutter="16">
          <a-col :span="12">
            <a-form-item field="type" label="商品类型" required>
              <a-select v-model="form.type">
                <a-option value="vip">VIP商品</a-option>
                <a-option value="fen">积分商品</a-option>
                <a-option value="agent">代理商品</a-option>
              </a-select>
            </a-form-item>
          </a-col>
          <a-col :span="12">
            <a-form-item field="price" label="价格" required>
              <a-input-number v-model="form.price" :min="0" :precision="2" style="width: 100%">
                <template #prefix>¥</template>
              </a-input-number>
            </a-form-item>
          </a-col>
        </a-row>
        <a-row :gutter="16">
          <a-col :span="12">
            <a-form-item field="days" label="有效天数">
              <a-input-number v-model="form.days" :min="0" style="width: 100%" />
            </a-form-item>
          </a-col>
          <a-col :span="12">
            <a-form-item field="sort" label="排序">
              <a-input-number v-model="form.sort" :min="0" style="width: 100%" />
            </a-form-item>
          </a-col>
        </a-row>
        <a-form-item field="description" label="商品描述">
          <a-textarea v-model="form.description" placeholder="请输入商品描述" />
        </a-form-item>
        <a-form-item field="status" label="状态">
          <a-radio-group v-model="form.status">
            <a-radio :value="1">上架</a-radio>
            <a-radio :value="0">下架</a-radio>
          </a-radio-group>
        </a-form-item>
      </a-form>
    </a-modal>
  </div>
</template>

<script setup>
import { ref, reactive, computed, onMounted } from 'vue'
import { Message } from '@arco-design/web-vue'
import goodsApi from '@/api/system/goods'

const searchForm = reactive({ name: '', type: undefined, status: undefined })
const tableData = ref([])
const loading = ref(false)
const pagination = reactive({ current: 1, pageSize: 20, total: 0, showTotal: true })

const modalVisible = ref(false)
const modalTitle = computed(() => form.id ? '编辑商品' : '添加商品')
const formRef = ref(null)
const form = reactive({ id: '', name: '', type: 'vip', price: 0, days: 0, sort: 0, description: '', status: 1 })

const rules = {
  name: [{ required: true, message: '请输入商品名称' }],
  type: [{ required: true, message: '请选择商品类型' }],
  price: [{ required: true, message: '请输入价格' }]
}

const columns = [
  { title: 'ID', dataIndex: 'id', width: 80 },
  { title: '商品名称', dataIndex: 'name' },
  { title: '类型', dataIndex: 'type', slotName: 'type' },
  { title: '价格', dataIndex: 'price', slotName: 'price' },
  { title: '有效天数', dataIndex: 'days' },
  { title: '描述', dataIndex: 'description', ellipsis: true },
  { title: '状态', dataIndex: 'status', slotName: 'status', width: 80 },
  { title: '操作', slotName: 'actions', width: 120 }
]

const loadData = async () => {
  loading.value = true
  try {
    const res = await goodsApi.getList({ ...searchForm, page: pagination.current, size: pagination.pageSize })
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
const handleReset = () => { Object.assign(searchForm, { name: '', type: undefined, status: undefined }); handleSearch() }
const handlePageChange = (page) => { pagination.current = page; loadData() }

const handleAdd = () => {
  Object.assign(form, { id: '', name: '', type: 'vip', price: 0, days: 0, sort: 0, description: '', status: 1 })
  modalVisible.value = true
}

const handleEdit = (record) => { Object.assign(form, record); modalVisible.value = true }

const handleSubmit = async () => {
  const valid = await formRef.value?.validate()
  if (valid) return
  try {
    const api = form.id ? goodsApi.edit : goodsApi.add
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
    const res = await goodsApi.del(record.id)
    if (res.code === 200) { Message.success('删除成功'); loadData() }
  } catch (e) { Message.error('删除失败') }
}

const handleStatusChange = async (record) => {
  try {
    await goodsApi.editState({ id: record.id, status: record.status })
    Message.success('状态更新成功')
  } catch (e) { record.status = record.status === 1 ? 0 : 1; Message.error('操作失败') }
}

onMounted(() => { loadData() })
</script>

<script>
export default { name: 'GoodsList' }
</script>

<style scoped>
.goods-management { padding: 16px; }
</style>
