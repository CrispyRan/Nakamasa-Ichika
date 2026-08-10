<template>
  <div class="kami-management">
    <!-- 搜索栏 -->
    <a-card class="mb-4" :bordered="false">
      <a-form :model="searchForm" layout="inline" auto-label-width>
        <a-row :gutter="16" class="w-full">
          <a-col :xs="24" :sm="12" :md="8" :lg="6">
            <a-form-item field="state" label="卡密状态">
              <a-select v-model="searchForm.state" placeholder="全部" allow-clear @change="handleSearch">
                <a-option value="y">正常</a-option>
                <a-option value="n">禁用</a-option>
              </a-select>
            </a-form-item>
          </a-col>
          <a-col :xs="24" :sm="12" :md="8" :lg="6">
            <a-form-item field="use_state" label="使用状态">
              <a-select v-model="searchForm.use_state" placeholder="全部" allow-clear @change="handleSearch">
                <a-option value="n">未使用</a-option>
                <a-option value="y">已使用</a-option>
              </a-select>
            </a-form-item>
          </a-col>
          <a-col :xs="24" :sm="12" :md="8" :lg="6">
            <a-form-item field="out_state" label="导出状态">
              <a-select v-model="searchForm.out_state" placeholder="全部" allow-clear @change="handleSearch">
                <a-option value="n">未导出</a-option>
                <a-option value="y">已导出</a-option>
              </a-select>
            </a-form-item>
          </a-col>
          <a-col :xs="24" :sm="12" :md="8" :lg="6">
            <a-form-item field="add_role" label="创建人">
              <a-select v-model="searchForm.add_role" placeholder="全部" allow-clear @change="handleSearch">
                <a-option value="admin">管理员</a-option>
                <a-option value="agent">代理</a-option>
              </a-select>
            </a-form-item>
          </a-col>
          <a-col :xs="24" :sm="12" :md="8" :lg="6">
            <a-form-item field="type" label="卡密类型">
              <a-select v-model="searchForm.type" placeholder="全部" allow-clear @change="handleSearch">
                <a-option value="vip">会员</a-option>
                <a-option value="fen">积分</a-option>
                <a-option value="addsn">设备增绑卡</a-option>
              </a-select>
            </a-form-item>
          </a-col>
          <a-col :xs="24" :sm="12" :md="8" :lg="6">
            <a-form-item field="gid" label="分组">
              <a-select v-model="searchForm.gid" placeholder="全部" allow-clear @change="handleSearch">
                <a-option v-for="group in groups" :key="group.id" :value="group.id">{{ group.name }}</a-option>
              </a-select>
            </a-form-item>
          </a-col>
          <a-col :xs="24" :sm="12" :md="8" :lg="6">
            <a-form-item field="add_time" label="创建时间">
              <a-range-picker v-model="searchForm.add_time" style="width: 100%" @change="handleSearch" />
            </a-form-item>
          </a-col>
          <a-col :xs="24" :sm="12" :md="8" :lg="6">
            <a-form-item field="use_time" label="使用时间">
              <a-range-picker v-model="searchForm.use_time" style="width: 100%" @change="handleSearch" />
            </a-form-item>
          </a-col>
          <a-col :xs="24" :sm="24" :md="12" :lg="8">
            <a-form-item field="keyword" label="关键词">
              <a-input-group>
                <a-select v-model="searchForm.keywordType" style="width: 100px">
                  <a-option value="cdk">卡号</a-option>
                  <a-option value="user">使用者</a-option>
                  <a-option value="note">备注</a-option>
                  <a-option value="use_ip">使用IP</a-option>
                </a-select>
                <a-input v-model="searchForm.keyword" :placeholder="keywordPlaceholder" allow-clear @press-enter="handleSearch" />
              </a-input-group>
            </a-form-item>
          </a-col>
          <a-col :xs="24" :sm="12" :md="4" :lg="4">
            <a-form-item>
              <a-space>
                <a-button type="primary" @click="handleSearch">
                  <template #icon><icon-search /></template>
                  搜索
                </a-button>
                <a-button @click="handleReset">
                  <template #icon><icon-refresh /></template>
                  重置
                </a-button>
              </a-space>
            </a-form-item>
          </a-col>
        </a-row>
      </a-form>
    </a-card>

    <!-- 操作栏 -->
    <a-card class="mb-4" :bordered="false">
      <a-space>
        <a-button type="primary" @click="handleAdd">
          <template #icon><icon-plus /></template>
          创建卡密
        </a-button>
        <a-button status="danger" @click="handleClear">
          <template #icon><icon-delete /></template>
          清理卡密
        </a-button>
        <a-button @click="goToGroups">
          <template #icon><icon-folder /></template>
          分组管理
        </a-button>
      </a-space>
    </a-card>

    <!-- 数据表格 -->
    <a-card :bordered="false">
      <a-table
        :columns="columns"
        :data="tableData"
        :loading="loading"
        :pagination="false"
        :row-selection="rowSelection"
        :selected-keys="selectedKeys"
        row-key="id"
        @selection-change="handleSelectionChange"
      >
        <template #cdk="{ record }">
          <div class="flex items-center">
            <span class="font-mono font-medium">{{ record.cdk }}</span>
            <a-button type="text" size="mini" class="ml-1" @click="copyCode(record.cdk)">
              <template #icon><icon-copy /></template>
            </a-button>
          </div>
          <a-tag v-if="record.type" :color="record.type === 'vip' ? 'red' : (record.type === 'fen' ? 'orange' : 'arcoblue')" size="small" class="mt-1">
            {{ record.type === 'vip' ? '会员卡' : (record.type === 'fen' ? '积分卡' : '增绑卡') }}
          </a-tag>
          <p v-if="record.note" class="text-gray-400 text-xs mt-1">{{ record.note }}</p>
        </template>
        <template #group="{ record }">
          <a-tooltip :content="record.Gname">
            <span>{{ record.type === 'vip' ? formatVipVal(record.val) : (record.val + (record.type === 'fen' ? '积分' : '台')) }}</span>
          </a-tooltip>
          <p class="text-gray-400 text-xs">{{ record.add_user || '系统' }}</p>
        </template>
        <template #use="{ record }">
          <a-tooltip v-if="record.use_user" :content="'使用IP：' + record.use_ip">
            <span>{{ record.use_user }}</span>
          </a-tooltip>
          <span v-else class="text-gray-400">未使用</span>
          <p class="text-gray-400 text-xs">{{ record.use_time ? formatTime(record.use_time) : '未使用' }}</p>
        </template>
        <template #add_time="{ record }">
          <span>{{ formatTime(record.add_time) }}</span>
          <p class="text-gray-400 text-xs">{{ record.out_time ? formatTime(record.out_time) : '未导出' }}</p>
        </template>
        <template #state="{ record }">
          <a-switch
            v-model="record.state"
            checked-value="y"
            unchecked-value="n"
            checked-color="#23C343"
            unchecked-color="#F53F3F"
            size="small"
            @change="(val) => handleStateChange(record, val)"
          />
        </template>
        <template #actions="{ record }">
          <a-space>
            <a-button type="text" size="small" @click="handleEdit(record)" :disabled="!!record.use_time">编辑</a-button>
            <a-popconfirm content="确定删除该卡密吗？" @ok="handleDelete(record)">
              <a-button type="text" size="small" status="danger">删除</a-button>
            </a-popconfirm>
          </a-space>
        </template>
      </a-table>

      <!-- 底部操作栏 -->
      <div class="flex items-center justify-between mt-4">
        <div class="text-gray-500 text-sm">
          <template v-if="selectedKeys.length > 0">
            <a-space>
              <a-button size="small" @click="showExportModal">导出</a-button>
              <a-popconfirm content="确定删除选中数据？" @ok="handleBatchDelete">
                <a-button size="small" status="danger">删除</a-button>
              </a-popconfirm>
              <span>当前选中 {{ selectedKeys.length }} 条数据</span>
            </a-space>
          </template>
          <template v-else>
            当前第 {{ pagination.current }} 页 共 {{ pagination.totalPages }} 页 {{ pagination.total }} 条结果
          </template>
        </div>
        <a-pagination
          v-model:current="pagination.current"
          v-model:page-size="pagination.pageSize"
          :total="pagination.total"
          show-page-size
          show-total
          @change="handlePageChange"
          @page-size-change="handlePageSizeChange"
        />
      </div>
    </a-card>

    <!-- 创建卡密弹窗 -->
    <a-modal v-model:visible="modalVisible" title="创建卡密" :width="480" @ok="handleSubmit" @cancel="handleCancel">
      <a-form ref="formRef" :model="form" :rules="rules" layout="vertical">
        <a-form-item field="gid" label="卡密组" required>
          <a-select v-model="form.gid" placeholder="请选择卡密组">
            <a-option v-for="group in groups" :key="group.id" :value="group.id">
              <span>{{ group.name }}</span>
              <span class="text-gray-400 ml-2 text-sm">
                ({{ group.type === 'vip' ? formatVipVal(group.val) : (group.val + (group.type === 'fen' ? '积分' : '台')) }}
                · ¥{{ group.price }})
              </span>
            </a-option>
          </a-select>
        </a-form-item>
        <a-form-item field="note" label="备注">
          <a-input v-model="form.note" placeholder="如：活动卡密（可空）" />
        </a-form-item>
        <a-form-item field="length" label="长度" tooltip="自定义卡密长度，为保证卡密唯一性，仅可在13~32位字符区间">
          <a-input-number v-model="form.length" :min="13" :max="32" style="width: 100%">
            <template #suffix>位</template>
          </a-input-number>
        </a-form-item>
        <a-form-item field="pre" label="前缀" tooltip="卡密前缀有助于区分卡密，支持字母、数字、下划线(_)、横杠(-)">
          <a-input v-model="form.pre" placeholder="如：TK-（可空）" allow-clear />
        </a-form-item>
        <a-form-item field="num" label="数量" tooltip="为了避免生成超时，一次性最多生成4000张">
          <a-input-number v-model="form.num" :min="1" :max="4000" style="width: 100%">
            <template #suffix>张</template>
          </a-input-number>
        </a-form-item>
        <a-form-item field="out" label="导出" tooltip="生成后自动导出">
          <a-select v-model="form.out" placeholder="请选择导出格式（不选择不导出）" allow-clear>
            <a-option value="txt">文本（txt）</a-option>
            <a-option value="csv">表格（csv）</a-option>
          </a-select>
        </a-form-item>
      </a-form>
    </a-modal>

    <!-- 编辑弹窗 -->
    <a-modal v-model:visible="editModalVisible" title="编辑卡密" :width="480" @ok="handleEditSubmit" @cancel="editModalVisible = false">
      <div class="bg-blue-600 rounded p-4 text-white mb-4">
        <div class="mb-3">
          <p class="text-lg font-bold">{{ editForm.cdk }}</p>
          <p class="text-blue-200 text-sm">卡号</p>
        </div>
        <div class="flex justify-between">
          <p>卡密组</p>
          <p>{{ editForm.Gname }}</p>
        </div>
        <hr class="my-2 border-blue-400" />
        <div class="flex justify-between">
          <p>创建者</p>
          <p>{{ editForm.add_user || '系统' }}</p>
        </div>
        <hr class="my-2 border-blue-400" />
        <div class="flex justify-between">
          <p>创建时间</p>
          <p>{{ formatTime(editForm.add_time) }}</p>
        </div>
      </div>
      <a-form ref="editFormRef" :model="editForm" layout="vertical">
        <a-form-item field="note" label="卡密备注">
          <a-textarea v-model="editForm.note" placeholder="如：活动卡密（可空）" :max-length="200" />
        </a-form-item>
      </a-form>
    </a-modal>

    <!-- 导出弹窗 -->
    <a-modal v-model:visible="exportModalVisible" title="导出卡密" :width="400" @ok="handleExport" @cancel="exportModalVisible = false">
      <a-form :model="exportForm" layout="vertical">
        <a-form-item label="当前选中">
          <span>{{ selectedKeys.length }} 条卡密数据</span>
        </a-form-item>
        <a-form-item field="out" label="导出格式">
          <a-radio-group v-model="exportForm.out">
            <a-radio value="txt">文本（txt）</a-radio>
            <a-radio value="csv">表格（csv）</a-radio>
          </a-radio-group>
        </a-form-item>
      </a-form>
    </a-modal>
  </div>
</template>

<script setup>
import { ref, reactive, computed, onMounted } from 'vue'
import { useRouter } from 'vue-router'
import { Message, Modal } from '@arco-design/web-vue'
import cdkUserApi from '@/api/system/cdkUser'

const router = useRouter()

// 搜索表单
const searchForm = reactive({
  state: '',
  use_state: '',
  out_state: '',
  add_role: '',
  type: '',
  gid: '',
  add_time: [],
  use_time: [],
  keywordType: 'cdk',
  keyword: ''
})

// 关键词占位符
const keywordPlaceholder = computed(() => {
  const map = {
    cdk: '请输入卡密卡号',
    user: '请输入使用者账号',
    note: '请输入备注',
    use_ip: '请输入使用IP'
  }
  return map[searchForm.keywordType] || '请输入关键词'
})

// 分组列表
const groups = ref([])

// 表格数据
const tableData = ref([])
const loading = ref(false)
const selectedKeys = ref([])
const rowSelection = reactive({
  type: 'checkbox',
  showCheckedAll: true,
  onlyCurrent: false
})

const pagination = reactive({
  current: 1,
  pageSize: 20,
  total: 0,
  totalPages: 0
})

// 创建卡密弹窗
const modalVisible = ref(false)
const formRef = ref(null)
const form = reactive({
  gid: '',
  note: '',
  length: 13,
  pre: '',
  num: 1,
  out: ''
})

const rules = {
  gid: [{ required: true, message: '请选择分组' }],
  num: [{ required: true, message: '请输入数量' }]
}

// 编辑弹窗
const editModalVisible = ref(false)
const editFormRef = ref(null)
const editForm = reactive({
  id: '',
  cdk: '',
  note: '',
  Gname: '',
  add_user: '',
  add_time: ''
})

// 导出弹窗
const exportModalVisible = ref(false)
const exportForm = reactive({
  out: 'txt'
})

// 格式化时间戳
const formatTime = (timestamp) => {
  if (!timestamp) return '-'
  const date = new Date(timestamp * 1000)
  return date.toLocaleString('zh-CN', {
    year: 'numeric', month: '2-digit', day: '2-digit',
    hour: '2-digit', minute: '2-digit', second: '2-digit'
  })
}

// 格式化 VIP 时长
const formatVipVal = (seconds) => {
  if (!seconds || seconds === 0) return '0秒'
  if (seconds >= 999999999) return '永久'
  const days = Math.floor(seconds / 86400)
  const hours = Math.floor((seconds % 86400) / 3600)
  const minutes = Math.floor((seconds % 3600) / 60)
  if (days > 0) return `${days}天`
  if (hours > 0) return `${hours}小时`
  if (minutes > 0) return `${minutes}分钟`
  return `${seconds % 60}秒`
}

// 表格列
const columns = [
  { title: 'ID', dataIndex: 'id', width: 70 },
  { title: '卡密/备注', slotName: 'cdk' },
  { title: '面值/创建人', slotName: 'group', width: 120 },
  { title: '使用者/时间', slotName: 'use', width: 120 },
  { title: '创建/导出时间', slotName: 'add_time', width: 140 },
  { title: '状态', slotName: 'state', width: 70 },
  { title: '操作', slotName: 'actions', width: 100, fixed: 'right' }
]

// 加载分组
const loadGroups = async () => {
  try {
    const res = await cdkUserApi.getGroupList()
    if (res.code === 200) {
      groups.value = res.data || []
    }
  } catch (e) {
    console.error('加载分组失败')
  }
}

// 加载数据
const loadData = async () => {
  loading.value = true
  try {
    const res = await cdkUserApi.getList({
      ...searchForm,
      page: pagination.current,
      size: pagination.pageSize
    })
    if (res.code === 200) {
      tableData.value = res.data?.list || []
      pagination.total = res.data?.dataTotal || 0
      pagination.totalPages = res.data?.pageTotal || 0
    }
  } catch (e) {
    Message.error('加载数据失败')
  } finally {
    loading.value = false
  }
}

// 复制卡密码
const copyCode = (code) => {
  navigator.clipboard?.writeText(code)
  Message.success('已复制到剪贴板')
}

// 搜索
const handleSearch = () => {
  pagination.current = 1
  loadData()
}

// 重置
const handleReset = () => {
  Object.assign(searchForm, {
    state: '',
    use_state: '',
    out_state: '',
    add_role: '',
    type: '',
    gid: '',
    add_time: [],
    use_time: [],
    keywordType: 'cdk',
    keyword: ''
  })
  handleSearch()
}

// 分页
const handlePageChange = (page) => {
  pagination.current = page
  loadData()
}

const handlePageSizeChange = (pageSize) => {
  pagination.pageSize = pageSize
  pagination.current = 1
  loadData()
}

// 选择变化
const handleSelectionChange = (keys) => {
  selectedKeys.value = keys
}

// 创建卡密
const handleAdd = () => {
  Object.assign(form, { gid: '', note: '', length: 13, pre: '', num: 1, out: '' })
  modalVisible.value = true
}

// 编辑
const handleEdit = (record) => {
  Object.assign(editForm, {
    id: record.id,
    cdk: record.cdk,
    note: record.note || '',
    Gname: record.Gname || '',
    add_user: record.add_user || '',
    add_time: record.add_time
  })
  editModalVisible.value = true
}

// 提交创建
const handleSubmit = async () => {
  const valid = await formRef.value?.validate()
  if (valid) return

  try {
    const res = await cdkUserApi.add(form)
    if (res.code === 200) {
      Message.success(res.msg || '创建成功')
      modalVisible.value = false
      // 如果有下载链接，自动下载
      if (res.data?.downUrl) {
        cdkUserApi.downloadFile(res.data.downUrl)
      }
      loadData()
    }
  } catch (e) {
    Message.error('操作失败')
  }
}

// 提交编辑
const handleEditSubmit = async () => {
  try {
    const res = await cdkUserApi.edit(editForm.id, editForm.note)
    if (res.code === 200) {
      Message.success('编辑成功')
      editModalVisible.value = false
      loadData()
    }
  } catch (e) {
    Message.error('操作失败')
  }
}

// 取消
const handleCancel = () => {
  modalVisible.value = false
}

// 删除
const handleDelete = async (record) => {
  try {
    const res = await cdkUserApi.del(record.id)
    if (res.code === 200) {
      Message.success('删除成功')
      loadData()
    }
  } catch (e) {
    Message.error('删除失败')
  }
}

// 批量删除
const handleBatchDelete = async () => {
  try {
    const res = await cdkUserApi.delAll(selectedKeys.value)
    if (res.code === 200) {
      Message.success('删除成功')
      selectedKeys.value = []
      loadData()
    }
  } catch (e) {
    Message.error('删除失败')
  }
}

// 状态切换
const handleStateChange = async (record, val) => {
  try {
    const res = await cdkUserApi.editState(record.id, val)
    if (res.code === 200) {
      Message.success('状态已更新')
    } else {
      record.state = val === 'y' ? 'n' : 'y'
    }
  } catch (e) {
    record.state = val === 'y' ? 'n' : 'y'
    Message.error('操作失败')
  }
}

// 显示导出弹窗
const showExportModal = () => {
  exportForm.out = 'txt'
  exportModalVisible.value = true
}

// 导出
const handleExport = async () => {
  try {
    const res = await cdkUserApi.outAll(selectedKeys.value, exportForm.out)
    if (res.code === 200) {
      Message.success('导出成功')
      exportModalVisible.value = false
      selectedKeys.value = []
      // 下载文件
      if (res.data?.content) {
        cdkUserApi.downloadContent(res.data.content, res.data.format || exportForm.out)
      }
      loadData()
    }
  } catch (e) {
    Message.error('导出失败')
  }
}

// 清理卡密
const handleClear = () => {
  Modal.info({
    titleAlign: 'start',
    title: '确认清理卡密',
    content: '提示：此操作仅清理已被使用的卡密，清理后不可恢复，请谨慎操作！',
    okText: '确认清理',
    hideCancel: false,
    width: 350,
    onBeforeOk: async () => {
      try {
        const res = await cdkUserApi.clear()
        if (res.code === 200) {
          Message.success(res.msg || '清理成功')
          loadData()
          return true
        } else {
          Message.error(res.msg || '清理失败')
          return false
        }
      } catch (e) {
        Message.error('操作失败')
        return false
      }
    }
  })
}

// 跳转分组管理
const goToGroups = () => {
  router.push('/kami/group')
}

onMounted(() => {
  loadGroups()
  loadData()
})
</script>

<script>
export default { name: 'KamiList' }
</script>

<style scoped>
.kami-management {
  padding: 16px;
}
</style>
