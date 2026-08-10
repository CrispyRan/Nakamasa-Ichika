<template>
  <div class="admin-management">
    <!-- 搜索栏 -->
    <a-card class="mb-4" :bordered="false">
      <a-form :model="searchForm" layout="inline" auto-label-width>
        <a-form-item label="用户名">
          <a-input v-model="searchForm.username" placeholder="请输入用户名" allow-clear style="width: 180px" />
        </a-form-item>
        <a-form-item label="状态">
          <a-select v-model="searchForm.status" placeholder="请选择" allow-clear style="width: 100px">
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
      <a-button type="primary" @click="handleAdd">
        <template #icon><icon-plus /></template>
        添加管理员
      </a-button>
    </a-card>

    <!-- 数据表格 -->
    <a-card :bordered="false">
      <a-table :columns="columns" :data="tableData" :loading="loading" :pagination="pagination" row-key="id" @page-change="handlePageChange">
        <template #avatar="{ record }">
          <a-avatar :size="32">
            <img v-if="record.avatar" :src="record.avatar" />
            <icon-user v-else />
          </a-avatar>
        </template>
        <template #role="{ record }">
          <a-tag color="blue">{{ record.role_name || '管理员' }}</a-tag>
        </template>
        <template #status="{ record }">
          <a-switch v-model="record.status" :checked-value="1" :unchecked-value="0" @change="handleStatusChange(record)" />
        </template>
        <template #actions="{ record }">
          <a-space>
            <a-button type="text" size="small" @click="handleEdit(record)">编辑</a-button>
            <a-button type="text" size="small" @click="handleResetPwd(record)">重置密码</a-button>
            <a-popconfirm content="确定删除该管理员吗？" @ok="handleDelete(record)">
              <a-button type="text" size="small" status="danger">删除</a-button>
            </a-popconfirm>
          </a-space>
        </template>
      </a-table>
    </a-card>

    <!-- 编辑弹窗 -->
    <a-modal v-model:visible="modalVisible" :title="modalTitle" :width="520" @ok="handleSubmit" @cancel="handleCancel">
      <a-form ref="formRef" :model="form" :rules="rules" layout="vertical">
        <a-form-item field="username" label="用户名" required>
          <a-input v-model="form.username" placeholder="请输入用户名" :disabled="!!form.id" />
        </a-form-item>
        <a-form-item field="nickname" label="昵称">
          <a-input v-model="form.nickname" placeholder="请输入昵称" />
        </a-form-item>
        <a-form-item field="password" label="密码" :required="!form.id">
          <a-input-password v-model="form.password" :placeholder="form.id ? '留空则不修改' : '请输入密码'" />
        </a-form-item>
        <a-row :gutter="16">
          <a-col :span="12">
            <a-form-item field="phone" label="手机号">
              <a-input v-model="form.phone" placeholder="请输入手机号" />
            </a-form-item>
          </a-col>
          <a-col :span="12">
            <a-form-item field="email" label="邮箱">
              <a-input v-model="form.email" placeholder="请输入邮箱" />
            </a-form-item>
          </a-col>
        </a-row>
        <a-form-item field="status" label="状态">
          <a-radio-group v-model="form.status">
            <a-radio :value="1">正常</a-radio>
            <a-radio :value="0">禁用</a-radio>
          </a-radio-group>
        </a-form-item>
      </a-form>
    </a-modal>
  </div>
</template>

<script setup>
import { ref, reactive, computed, onMounted } from 'vue'
import { Message } from '@arco-design/web-vue'
import adminApi from '@/api/system/adminList'

const searchForm = reactive({ username: '', status: undefined })
const tableData = ref([])
const loading = ref(false)
const pagination = reactive({ current: 1, pageSize: 20, total: 0, showTotal: true })

const modalVisible = ref(false)
const modalTitle = computed(() => form.id ? '编辑管理员' : '添加管理员')
const formRef = ref(null)
const form = reactive({ id: '', username: '', nickname: '', password: '', phone: '', email: '', status: 1 })

const rules = {
  username: [{ required: true, message: '请输入用户名' }],
  password: [{ required: !form.id, message: '请输入密码' }]
}

const columns = [
  { title: 'ID', dataIndex: 'id', width: 80 },
  { title: '头像', dataIndex: 'avatar', slotName: 'avatar', width: 80 },
  { title: '用户名', dataIndex: 'username' },
  { title: '昵称', dataIndex: 'nickname' },
  { title: '角色', dataIndex: 'role', slotName: 'role' },
  { title: '手机号', dataIndex: 'phone' },
  { title: '状态', dataIndex: 'status', slotName: 'status', width: 80 },
  { title: '最后登录', dataIndex: 'last_login', width: 160 },
  { title: '操作', slotName: 'actions', width: 180 }
]

const loadData = async () => {
  loading.value = true
  try {
    const res = await adminApi.getList({ ...searchForm, page: pagination.current, size: pagination.pageSize })
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
  Object.assign(form, { id: '', username: '', nickname: '', password: '', phone: '', email: '', status: 1 })
  modalVisible.value = true
}

const handleEdit = (record) => { Object.assign(form, { ...record, password: '' }); modalVisible.value = true }

const handleSubmit = async () => {
  const valid = await formRef.value?.validate()
  if (valid) return
  try {
    const api = form.id ? adminApi.edit : adminApi.add
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
    const res = await adminApi.del(record.id)
    if (res.code === 200) { Message.success('删除成功'); loadData() }
  } catch (e) { Message.error('删除失败') }
}

const handleStatusChange = async (record) => {
  try {
    await adminApi.edit({ id: record.id, status: record.status })
    Message.success('状态更新成功')
  } catch (e) { record.status = record.status === 1 ? 0 : 1; Message.error('操作失败') }
}

const handleResetPwd = async (record) => {
  try {
    await adminApi.edit({ id: record.id, password: '123456' })
    Message.success('密码已重置为: 123456')
  } catch (e) { Message.error('重置失败') }
}

onMounted(() => { loadData() })
</script>

<script>
export default { name: 'AdminList' }
</script>

<style scoped>
.admin-management { padding: 16px; }
</style>
