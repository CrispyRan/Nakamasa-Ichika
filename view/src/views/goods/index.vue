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
          <span class="text-red-500 font-medium">¥{{ record.money }}</span>
        </template>
        <template #value="{ record }">
          <span v-if="record.type === 'agent'">{{ getGroupName(record.val) || record.val }}</span>
          <span v-else>{{ record.val }}</span>
        </template>
        <template #status="{ record }">
          <a-switch v-model="record.state" :checked-value="'y'" :unchecked-value="'n'" @change="handleStatusChange(record)" />
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
      <a-form ref="formRef" :model="form" :rules="rules.value" layout="vertical">
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
            <a-form-item field="money" label="价格" required>
              <a-input-number v-model="form.money" :min="0" :precision="2" style="width: 100%">
                <template #prefix>¥</template>
              </a-input-number>
            </a-form-item>
          </a-col>
        </a-row>
        <a-row v-if="form.type === 'agent'" :gutter="16">
          <a-col :span="12">
            <a-form-item field="val" label="代理分组" required>
              <a-select
                v-model="form.val"
                :loading="groupLoading"
                placeholder="请选择代理分组"
                allow-clear
                style="width: 100%"
              >
                <a-option v-for="item in groupOptions" :key="item.id" :value="item.id" :label="item.name">
                  {{ item.name }}
                </a-option>
              </a-select>
            </a-form-item>
          </a-col>
        </a-row>
        <a-row v-if="form.type === 'vip' || form.type === 'fen'" :gutter="16">
          <a-col :span="12">
            <a-form-item field="val" :label="form.type === 'vip' ? '会员天数' : '积分数量'" required>
              <a-input-number v-model="form.val" :min="1" :precision="0" style="width: 100%" />
            </a-form-item>
          </a-col>
        </a-row>
        <a-row :gutter="16">
          <a-col :span="12">
            <a-form-item field="sort" label="排序">
              <a-input-number v-model="form.sort" :min="0" style="width: 100%" />
            </a-form-item>
          </a-col>
        </a-row>
        <a-form-item field="blurb" label="商品描述">
          <a-textarea v-model="form.blurb" placeholder="请输入商品描述" />
        </a-form-item>
        <a-form-item field="state" label="状态">
          <a-radio-group v-model="form.state">
            <a-radio value="y">上架</a-radio>
            <a-radio value="n">下架</a-radio>
          </a-radio-group>
        </a-form-item>
      </a-form>
    </a-modal>
  </div>
</template>

<script setup>
import { ref, reactive, computed, onMounted, watch } from 'vue'
import { Message } from '@arco-design/web-vue'
import goodsApi from '@/api/system/goods'
import agentApi from '@/api/system/agent'

const searchForm = reactive({ name: '', type: undefined, state: undefined })
const tableData = ref([])
const loading = ref(false)
const pagination = reactive({ current: 1, pageSize: 20, total: 0, showTotal: true })

const modalVisible = ref(false)
const modalTitle = computed(() => form.id ? '编辑商品' : '添加商品')
const formRef = ref(null)
const groupOptions = ref([])
const groupLoading = ref(false)
const form = reactive({ id: '', name: '', type: 'vip', val: 0, money: 0, blurb: '', state: 'y' })

const rules = computed(() => {
  const nextRules = {
    name: [{ required: true, message: '请输入商品名称' }],
    type: [{ required: true, message: '请选择商品类型' }],
    money: [{ required: true, message: '请输入价格' }]
  }
  if (form.type === 'agent') {
    nextRules.val = [{ required: true, message: '请选择代理分组' }]
  } else {
    nextRules.val = [
      { required: true, message: form.type === 'vip' ? '请输入会员天数' : '请输入积分数量' },
      { type: 'number', message: '必须是数字' },
      { min: 1, message: '必须大于等于 1' }
    ]
  }
  return nextRules
})

const getGroupName = (val) => groupOptions.value.find(item => item.id === val)?.name || String(val ?? '')
const loadGroupOptions = async () => {
  if (groupOptions.value.length > 0) return
  groupLoading.value = true
  try {
    const res = await agentApi.getGroupList({ pg: 1, size: 100 })
    if (res.code === 200) {
      groupOptions.value = res.data?.list || res.data || []
    }
  } finally {
    groupLoading.value = false
  }
}

watch(
  () => form.type,
  async (type) => {
    if (type === 'agent') {
      await loadGroupOptions()
    }
  }
)

const columns = [
  { title: 'ID', dataIndex: 'id', width: 80 },
  { title: '商品名称', dataIndex: 'name' },
  { title: '类型', dataIndex: 'type', slotName: 'type' },
  { title: '价格', dataIndex: 'money', slotName: 'price' },
  { title: '类型值', dataIndex: 'val', slotName: 'value' },
  { title: '描述', dataIndex: 'blurb', ellipsis: true },
  { title: '状态', dataIndex: 'state', slotName: 'status', width: 80 },
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
const handleReset = () => { Object.assign(searchForm, { name: '', type: undefined, state: undefined }); handleSearch() }
const handlePageChange = (page) => { pagination.current = page; loadData() }

const handleAdd = () => {
  Object.assign(form, { id: '', name: '', type: 'vip', val: 0, money: 0, blurb: '', state: 'y' })
  modalVisible.value = true
}

const handleEdit = async (record) => {
  Object.assign(form, record)
  if (record.type === 'agent') await loadGroupOptions()
  modalVisible.value = true
}

const handleSubmit = async () => {
  if (form.type === 'agent' && (!form.val || form.val === 0)) {
    Message.warning('请选择代理分组')
    return
  }
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
    await goodsApi.editState({ id: record.id, state: record.state })
    Message.success('状态更新成功')
  } catch (e) { record.state = record.state === 'y' ? 'n' : 'y'; Message.error('操作失败') }
}

onMounted(() => { loadData() })
</script>

<script>
export default { name: 'GoodsList' }
</script>

<style scoped>
.goods-management { padding: 16px; }
</style>
