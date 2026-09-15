<template>
  <div class="admin-management">
    <!-- 搜索栏 -->
    <a-card class="mb-4" :bordered="false">
      <a-form :model="searchForm" layout="inline" auto-label-width>
        <a-form-item label="账号">
          <a-input v-model="searchForm.keyword" placeholder="账号或昵称" allow-clear style="width: 200px" />
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
            <img v-if="record.avatars" :src="record.avatars" />
            <icon-user v-else />
          </a-avatar>
        </template>
        <template #auth="{ record }">
          <a-tag v-if="isAllAuth(record.auth)" color="red">全部权限</a-tag>
          <a-tag v-else-if="record.auth.length">部分权限</a-tag>
          <span v-else class="text-gray-400">未配置</span>
        </template>
        <template #state="{ record }">
          <a-switch
            v-model="record.state"
            checked-value="y"
            unchecked-value="n"
            :disabled="record.id === 1"
            @change="handleStateChange(record, $event)"
          />
        </template>
        <template #actions="{ record }">
          <a-space>
            <a-button type="text" size="small" @click="handleEdit(record)">编辑</a-button>
            <a-popconfirm
              v-if="record.id !== 1"
              content="重置为 123456？"
              @ok="handleResetPwd(record)"
            >
              <a-button type="text" size="small">重置密码</a-button>
            </a-popconfirm>
            <a-popconfirm
              v-if="record.id !== 1"
              content="确定删除该管理员吗？"
              @ok="handleDelete(record)"
            >
              <a-button type="text" size="small" status="danger">删除</a-button>
            </a-popconfirm>
          </a-space>
        </template>
      </a-table>
    </a-card>

    <!-- 编辑弹窗。宽度调大以容纳权限表格（模块名 + 三个操作列） -->
    <a-modal v-model:visible="modalVisible" :title="modalTitle" :width="640" @ok="handleSubmit" @cancel="handleCancel">
      <a-form ref="formRef" :model="form" :rules="rules" layout="vertical">
        <a-form-item field="user" label="管理员账号" required>
          <a-input v-model="form.user" placeholder="5-12 位，仅字母数字" />
        </a-form-item>
        <a-form-item field="notes" label="昵称">
          <a-input v-model="form.notes" placeholder="请输入昵称" />
        </a-form-item>
        <a-form-item field="password" label="密码" :required="!form.id">
          <a-input-password v-model="form.password" :placeholder="form.id ? '留空则不修改' : '6-18 位'" />
        </a-form-item>
        <a-form-item label="权限分配">
          <a-tree-select
            v-model="selectedGroups"
            :data="authTreeData"
            multiple
            tree-checkable
            check-strictly
            tree-checked-strategy="child"
            tree-expand-all
            allow-search
            allow-clear
            placeholder="勾选需要的权限分组；全部勾选即最高权限"
            style="width: 100%"
          />
          <div class="auth-hint">
            已选 <b>{{ selectedGroups.length }}</b> / {{ AUTH_LEAVES.length }} 个分组
          </div>
        </a-form-item>
      </a-form>
    </a-modal>
  </div>
</template>

<script setup>
import { ref, reactive, computed, onMounted } from 'vue'
import { Message } from '@arco-design/web-vue'
import adminApi from '@/api/system/adminList'

// ─────────────────────────────────────────────
// 权限分配：两级 TreeSelect（分类 → 权限分组）
//
// 后端 auth 列是 JSON 字符串数组，按组名做路径前缀匹配、不分增删改操作。
// 白名单见 adm_list.rs 的 AUTH_GROUP_WHITELIST，共 17 个业务组
// （不含 all / * 两个通配符）。因此：
//   - 叶子 key 直接用后端组名，勾上即可原样提交；
//   - 父节点只是分组展示，key 带 'cat_' 前缀，绝不作为权限名提交；
//   - check-strictly 开启：勾父节点不级联联动子节点（否则父 key 会进
//     v-model，提交后被后端判成「未知的权限分组」）；
//   - tree-checked-strategy="child"：v-model 只返回被勾选的叶子，
//     即便父节点被勾也不会混入父 key。
//   - 父节点 disableCheckbox：分类本身不是权限，不该被勾选。
//   - 无顶部「全部权限」节点：最高权限就是手动把 17 个叶子全勾上，
//     提交后是显式的完整组名数组，不依赖 NULL 表示全量。
// ─────────────────────────────────────────────
const AUTH_CATEGORIES = [
  { key: 'cat_access', label: '访问权限', groups: ['user', 'app'] },
  { key: 'cat_system', label: '系统管理', groups: ['adm', 'system', 'send', 'ver', 'logs'] },
  { key: 'cat_business', label: '业务模块', groups: ['cdk', 'agent', 'finance', 'goods', 'order', 'statistics'] },
  { key: 'cat_content', label: '内容与功能', groups: ['content', 'function', 'upload', 'blocklist'] }
]

// 叶子 key 用后端组名（直接可提交），显示用中文名，避免树里全是英文缩写
const AUTH_GROUP_LABELS = {
  user: '用户管理',
  app: '应用管理',
  adm: '管理员管理',
  system: '系统配置',
  send: '发信控制',
  ver: '版本管理',
  logs: '日志管理',
  cdk: '卡密管理',
  agent: '代理管理',
  finance: '支付财务',
  goods: '商品管理',
  order: '订单管理',
  statistics: '数据统计',
  content: '公告留言',
  function: '云函数',
  upload: '附件管理',
  blocklist: '封禁管理'
}

// 模板要读 AUTH_LEAVES.length 显示「已选 n / 17」
const AUTH_LEAVES = AUTH_CATEGORIES.flatMap(c => c.groups)

// 提交给后端的权限组名数组。新建默认空 = 零权限，由管理员按需勾选
const selectedGroups = ref([])

// 两级树数据：分类为父节点（不参与提交），权限组为叶子
const authTreeData = computed(() => {
  return AUTH_CATEGORIES.map(c => ({
    key: c.key,
    title: c.label,
    disableCheckbox: true,
    children: c.groups.map(g => ({ key: g, title: AUTH_GROUP_LABELS[g] || g, checkable: true }))
  }))
})
const searchForm = reactive({ keyword: '' })
const tableData = ref([])
const loading = ref(false)
const pagination = reactive({ current: 1, pageSize: 20, total: 0, showTotal: true })

const modalVisible = ref(false)
const modalTitle = computed(() => form.id ? '编辑管理员' : '添加管理员')
const formRef = ref(null)
// 字段名与后端 adm_list.rs 对齐：user / notes / password
// 注意 u_admin 表没有 phone/email/last_login 列，不要加这些字段
// auth 单独走 payload.auth（权限树勾选结果），不放在 form 里
const form = reactive({ id: '', user: '', notes: '', password: '' })

// password 校验随增/编切换：编辑时允许留空（留空 = 不改密码）
const rules = computed(() => ({
  user: [
    { required: true, message: '请输入管理员账号' },
    { minLength: 5, maxLength: 12, message: '账号长度需 5-12 位' }
  ],
  password: [
    { required: !form.id, message: '请输入密码' },
    { minLength: 6, maxLength: 18, message: '密码长度需 6-18 位' }
  ]
}))

const columns = [
  { title: 'ID', dataIndex: 'id', width: 80 },
  { title: '头像', dataIndex: 'avatars', slotName: 'avatar', width: 80 },
  { title: '账号', dataIndex: 'user' },
  { title: '昵称', dataIndex: 'notes' },
  { title: '权限', dataIndex: 'auth', slotName: 'auth', width: 120 },
  { title: '状态', dataIndex: 'state', slotName: 'state', width: 90 },
  { title: '操作', slotName: 'actions', width: 200 }
]

// 后端 auth 为 JSON 组名数组；NULL 或缺失表示全部权限，[] 表示零权限
const isAllAuth = (auth) => auth == null

const loadData = async () => {
  loading.value = true
  try {
    // 后端 GetListRequest: { pg, size, so: { keyword } }
    const res = await adminApi.getList({
      pg: pagination.current,
      size: pagination.pageSize,
      so: { keyword: searchForm.keyword || undefined }
    })
    if (res.code === 200) {
      // 后端实际返回：data 是裸数组（ApiResponse::success("成功", Some(list))），
      // 没有 list/currentPage/pageTotal/dataTotal 分页字段。
      // 之前按 res.data.list 取值导致列表永远为空。
      const rows = Array.isArray(res.data) ? res.data : (res.data?.list || [])
      tableData.value = rows
      // 后端不返回总数，按当前页是否满页粗略估算，避免分页器报错
      if (!pagination.total || pagination.total < rows.length || rows.length < pagination.pageSize) {
        pagination.total = (pagination.current - 1) * pagination.pageSize + rows.length
      }
    }
  } finally {
    loading.value = false
  }
}

const handleSearch = () => { pagination.current = 1; loadData() }
const handleReset = () => { Object.assign(searchForm, { keyword: '' }); handleSearch() }
const handlePageChange = (page) => { pagination.current = page; loadData() }

const resetForm = () => {
  Object.assign(form, { id: '', user: '', notes: '', password: '' })
  // 新建默认不勾任何权限（零权限），由管理员按需勾选
  selectedGroups.value = []
}

const handleAdd = () => {
  resetForm()
  modalVisible.value = true
}

const handleEdit = (record) => {
  Object.assign(form, {
    id: record.id,
    user: record.user,
    notes: record.notes || '',
    password: ''
  })
  // 回填权限树：后端 list 把 auth 存成组名 JSON 数组。
  // null = 全部权限（全勾）；[] = 零权限（全不勾）；非空数组按组回填。
  // 只保留后端白名单内的组名，避免历史脏数据（旧版模块名如 'cdkKami'）
  // 混进 v-model 导致下拉里出现树中没有的幽灵标签。
  const raw = Array.isArray(record.auth) ? record.auth : (isAllAuth(record.auth) ? AUTH_LEAVES : [])
  selectedGroups.value = AUTH_LEAVES.filter(g => raw.includes(g))
  modalVisible.value = true
}

const handleSubmit = async () => {
  const valid = await formRef.value?.validate()
  if (valid) return
  // 超级管理员自己的权限不允许修改（后端同样拦截，这里给出前置提示）
  if (form.id === 1) {
    Message.error('不能修改超级管理员的权限')
    return
  }
  const payload = {
    ...(form.id ? { id: form.id } : {}),
    user: form.user,
    notes: form.notes
  }
  if (form.password) payload.password = form.password
  // 权限：树形选择 → 组名数组。tree-checked-strategy="child" 已保证 v-model
  // 只含叶子（父节点带 cat_ 前缀、不参与提交），这里再按白名单兜底一次，
  // 确保任何父节点 key 都不会发给后端被判成「未知的权限分组」。
  // 全勾 = 17 个组名 = 最高权限；全不勾 = [] = 零权限（后端原样落库）。
  payload.auth = AUTH_LEAVES.filter(g => selectedGroups.value.includes(g))
  try {
    const api = form.id ? adminApi.edit : adminApi.add
    const res = await api(payload)
    if (res.code === 200) {
      Message.success(form.id ? '编辑成功' : '添加成功')
      modalVisible.value = false
      loadData()
    } else {
      Message.error(res.msg || '操作失败')
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

// a-switch 的 v-model 直接绑定 'y'/'n'，$event 收到的是新值
const handleStateChange = async (record, checked) => {
  try {
    const res = await adminApi.editState({ id: record.id, state: checked })
    if (res.code === 200) {
      Message.success(checked === 'y' ? '已启用' : '已禁用')
      loadData()
    } else {
      Message.error(res.msg || '操作失败')
    }
  } catch (e) { Message.error('操作失败') }
}

const handleResetPwd = async (record) => {
  try {
    // 后端 edit 的 notes/user/auth 已改为可选，单独传 password 即可
    const res = await adminApi.edit({ id: record.id, password: '123456' })
    if (res.code === 200) Message.success('密码已重置为: 123456')
    else Message.error(res.msg || '重置失败')
  } catch (e) { Message.error('重置失败') }
}

onMounted(() => { loadData() })
</script>

<script>
export default { name: 'AdminList' }
</script>

<style scoped>
.admin-management { padding: 16px; }

/* 权限分配：TreeSelect 选中计数提示 */
.auth-hint {
  margin-top: 6px;
  font-size: 12px;
  color: var(--color-text-3);
}

.auth-hint b {
  color: var(--color-primary);
  font-weight: 600;
}
</style>
