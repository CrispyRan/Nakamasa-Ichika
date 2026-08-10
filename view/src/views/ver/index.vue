<template>
  <div class="ver-management">
    <!-- 弹窗 -->
    <a-modal
      v-model:visible="modal.visible"
      :title="modal.title"
      title-align="start"
      :mask-closable="false"
      :fullscreen="isMobile"
      @cancel="modal.visible = false"
    >
      <div class="modal-content">
        <a-form :model="modal.data" auto-label-width :layout="isMobile ? 'vertical' : 'horizontal'">
          <a-form-item field="name" label="版本名" extra="创建输入新版本名时需选中它或按回车生效">
            <a-select
              v-model="modal.data.name"
              placeholder="如：安卓版"
              allow-create
              @change="handleNameChange"
            >
              <template #empty>
                <div class="empty-tip">暂无版本</div>
              </template>
              <a-option
                v-for="item in verGroup"
                :key="item.ver_key"
                :value="item.name"
                @click="handleSelectGroup(item.ver_key, item.name)"
              />
            </a-select>
          </a-form-item>
          <a-form-item field="ver_key" label="版本索引">
            <a-input v-model="modal.data.ver_key" placeholder="如：android" :readonly="modal.data.id > 0" />
          </a-form-item>
          <a-form-item field="ver_val" label="版本号">
            <a-space>
              <a-input-number
                v-model="modal.data.ver_major"
                :hide-button="true"
                placeholder="1"
                :min="1"
                :max="999"
                :readonly="modal.data.id > 0"
                @keydown="handleVersionKeydown(1, $event)"
              />
              <span>.</span>
              <a-input-number
                v-model="modal.data.ver_minor"
                :hide-button="true"
                placeholder="0"
                :min="0"
                :max="999"
                :readonly="modal.data.id > 0"
                @keydown="handleVersionKeydown(2, $event)"
              />
              <span>.</span>
              <a-input-number
                v-model="modal.data.ver_patch"
                :hide-button="true"
                placeholder="0"
                :min="0"
                :max="999"
                :readonly="modal.data.id > 0"
                @keydown="handleVersionKeydown(3, $event)"
              />
            </a-space>
          </a-form-item>
          <a-form-item field="ver_state" label="版本状态">
            <a-select v-model="modal.data.ver_state">
              <a-option value="on">正常</a-option>
              <a-option value="off">关闭</a-option>
            </a-select>
          </a-form-item>
          <a-form-item v-if="modal.data.ver_state === 'off'" field="ver_off_msg" label="关闭提示">
            <a-input v-model="modal.data.ver_off_msg" placeholder="如：当前版本维护中" />
          </a-form-item>
          <template v-else>
            <a-form-item field="ver_new_url" label="下载地址">
              <a-input v-model="modal.data.ver_new_url" placeholder="如：http://www.example.com/1.0.apk" />
            </a-form-item>
            <a-form-item field="ver_new_content" label="更新提示">
              <a-textarea
                v-model="modal.data.ver_new_content"
                placeholder="如：1、更新了界面&#10;2、优化了用户体验&#10;3、修复了已知BUG"
                :auto-size="{ minRows: 3, maxRows: 5 }"
              />
            </a-form-item>
            <a-form-item v-if="milist.length > 0" field="mid" label="加密方案">
              <a-select v-model="modal.data.mid" placeholder="可选，空则不加密" allow-clear>
                <a-option v-for="item in milist" :key="item.id" :value="item.id">
                  {{ item.name }}
                </a-option>
              </a-select>
            </a-form-item>
          </template>
        </a-form>
      </div>
      <template #footer>
        <a-space>
          <a-button @click="modal.visible = false">取消</a-button>
          <a-button type="primary" :loading="modal.btnLoading" @click="handleSubmit">提交</a-button>
        </a-space>
      </template>
    </a-modal>

    <!-- 主内容区 -->
    <div class="main-content">
      <div class="content-wrapper">
        <!-- 顶部操作和搜索 -->
        <div class="header-row">
          <div class="header-left">
            <a-button type="primary" class="mb-5" @click="handleAdd">
              <template #icon><IconPlus /></template>
              创建版本
            </a-button>
          </div>
          <div class="header-right">
            <a-form :model="searchForm" class="search-form" auto-label-width>
              <a-form-item field="ver_key" hide-label>
                <a-select
                  v-model="searchForm.ver_key"
                  placeholder="全部版本"
                  allow-clear
                  class="filter-select"
                  @change="handleSearch"
                >
                  <template #empty>
                    <div class="empty-tip">暂无版本</div>
                  </template>
                  <a-option v-for="item in verGroup" :key="item.ver_key" :value="item.ver_key">
                    {{ item.name }}
                  </a-option>
                </a-select>
              </a-form-item>
              <a-form-item field="keyword" hide-label>
                <a-input-search
                  v-model="searchForm.keyword"
                  placeholder="关键词搜索..."
                  :loading="searchLoading"
                  allow-clear
                  class="keyword-input"
                  @search="handleSearch"
                  @press-enter="handleSearch"
                  @clear="handleSearch"
                />
              </a-form-item>
            </a-form>
          </div>
        </div>

        <!-- 数据表格 -->
        <div class="table-card">
          <a-table
            :columns="columns"
            :data="tableData"
            :loading="loading"
            :pagination="false"
            :row-selection="{ type: 'checkbox', showCheckedAll: true, onlyCurrent: false, selectedKeys: selectedKeys }"
            v-model:selectedKeys="selectedKeys"
            row-key="id"
            :bordered="false"
            class="ver-table"
          >
            <template #ver_number="{ record }">
              {{ record.ver_major }}.{{ record.ver_minor }}.{{ record.ver_patch }}
            </template>
            <template #discard_past="{ record }">
              <a-checkbox
                v-model="record.discard"
                @change="handleDiscard(record.id, record.discard)"
              >
                <template #checkbox="{ checked }">
                  <div :class="['custom-checkbox', { checked }]">
                    <IconCheck v-if="checked" />
                  </div>
                </template>
              </a-checkbox>
            </template>
            <template #ver_mi="{ record }">
              <a-tag v-if="record.mi_type" size="small" :color="miColorMap[record.mi_type] || 'gray'">
                {{ record.mi_name }}
              </a-tag>
              <span v-else class="text-gray">不加密</span>
            </template>
            <template #ver_state="{ record }">
              <a-badge :status="record.ver_state === 'off' ? 'danger' : 'success'" :text="record.ver_state === 'off' ? '关闭' : '正常'" />
            </template>
            <template #operate="{ record }">
              <a-space>
                <a-button type="text" size="small" @click="handleEdit(record)">编辑</a-button>
                <a-popconfirm content="确认删除该版本？" @ok="handleDelete(record.id)">
                  <a-button type="text" size="small" status="danger">删除</a-button>
                </a-popconfirm>
              </a-space>
            </template>
          </a-table>

          <!-- 分页 -->
          <div class="pagination-bar">
            <div class="pagination-left">
              <a-button
                v-if="selectedKeys.length > 0"
                status="danger"
                :loading="batchDeleteLoading"
                @click="handleBatchDelete"
              >
                删除
              </a-button>
              <span v-if="selectedKeys.length > 0" class="selected-info">
                当前选中的 {{ selectedKeys.length }} 条数据
              </span>
              <span v-else class="pagination-info">
                当前第 {{ pagination.current }} 页 共 {{ pageTotal }} 页 {{ pagination.total }} 条结果
              </span>
            </div>
            <div class="pagination-right">
              <a-pagination
                v-model:current="pagination.current"
                v-model:page-size="pagination.pageSize"
                :total="pagination.total"
                show-total
                show-jumper
                show-page-size
                :page-size-options="[10, 20, 50, 100]"
                @change="handlePageChange"
                @page-size-change="handlePageSizeChange"
              />
            </div>
          </div>
        </div>
      </div>
    </div>
  </div>
</template>

<script setup>
import { ref, reactive, computed, onMounted } from 'vue'
import { useRouter } from 'vue-router'
import { Message } from '@arco-design/web-vue'
import verApi from '@/api/system/ver'

const router = useRouter()
const isMobile = computed(() => router.currentRoute.value.meta.isMobile)

// 搜索表单
const searchForm = reactive({ ver_key: '', keyword: '' })
const searchLoading = ref(false)

// 表格数据
const tableData = ref([])
const loading = ref(false)
const pagination = reactive({ current: 1, pageSize: 10, total: 0 })
const pageTotal = computed(() => Math.ceil(pagination.total / pagination.pageSize) || 1)

// 选中项
const selectedKeys = ref([])
const batchDeleteLoading = ref(false)

// 版本分组和加密方案列表
const verGroup = ref([])
const milist = ref([])

// 加密方案颜色映射
const miColorMap = { rc4: 'cyan', des: 'blue', aes: 'arcoblue', rsa: 'red' }

// 表格列配置
const columns = [
  { title: '版本名称', slotName: 'ver_number', width: 80, align: 'center' },
  { title: '版本名', dataIndex: 'name', width: 100, align: 'center' },
  { title: '版本索引', dataIndex: 'ver_key', width: 100, align: 'center' },
  { title: '下载地址', dataIndex: 'ver_url' },
  { title: '弃用', slotName: 'discard_past', width: 100, align: 'center' },
  { title: '加密方案', slotName: 'ver_mi', width: 120, align: 'center' },
  { title: '状态', slotName: 'ver_state', width: 80, align: 'center' },
  { title: '操作', slotName: 'operate', width: 130, align: 'center' }
]

// 弹窗状态
const modal = reactive({
  visible: false,
  btnLoading: false,
  title: '添加版本',
  verKeyDisabled: false,
  data: {
    id: null,
    name: '',
    ver_key: '',
    ver_major: null,
    ver_minor: null,
    ver_patch: null,
    ver_state: 'on',
    ver_off_msg: '',
    ver_new_url: '',
    ver_new_content: '',
    mid: null,
    discard: false
  }
})

// 创建空白表单数据
const createEmptyForm = () => ({
  id: null,
  name: '',
  ver_key: '',
  ver_major: null,
  ver_minor: null,
  ver_patch: null,
  ver_state: 'on',
  ver_off_msg: '',
  ver_new_url: '',
  ver_new_content: '',
  mid: null,
  discard: false
})

// 加载数据
const loadData = async () => {
  loading.value = true
  searchLoading.value = true
  try {
    const res = await verApi.get(pagination.current, pagination.pageSize, searchForm)
    loading.value = false
    searchLoading.value = false
    if (res.code === 200) {
      tableData.value = res.data.list || []
      pagination.current = Number(res.data.currentPage)
      pagination.total = Number(res.data.dataTotal)
    } else {
      Message.error(res.msg)
    }
  } catch (e) {
    loading.value = false
    searchLoading.value = false
    Message.error('出错了：' + e)
  }
}

// 搜索
const handleSearch = () => {
  pagination.current = 1
  loadData()
}

// 分页
const handlePageChange = (page) => {
  pagination.current = page
  loadData()
}

const handlePageSizeChange = (size) => {
  pagination.pageSize = size
  pagination.current = 1
  loadData()
}

// 添加版本
const handleAdd = () => {
  modal.title = '添加版本'
  modal.data = createEmptyForm()
  modal.verKeyDisabled = false
  modal.visible = true
}

// 编辑版本
const handleEdit = (record) => {
  modal.title = '编辑版本'
  modal.data.id = record.id
  modal.data.name = record.name
  modal.data.ver_key = record.ver_key
  modal.data.ver_major = record.ver_major
  modal.data.ver_minor = record.ver_minor
  modal.data.ver_patch = record.ver_patch
  modal.data.ver_state = record.ver_state
  modal.data.ver_off_msg = record.ver_off_msg
  modal.data.ver_new_url = record.ver_url
  modal.data.ver_new_content = record.ver_content
  modal.data.mid = record.mid
  modal.data.discard = record.discard
  modal.verKeyDisabled = true
  modal.visible = true
}

// 选择版本分组
const handleSelectGroup = (verKey, name) => {
  modal.data.ver_key = verKey
  modal.verKeyDisabled = true
}

// 版本名改变时重置版本索引
const handleNameChange = () => {
  modal.data.ver_key = ''
  modal.verKeyDisabled = false
}

// 版本号输入框按键处理
const handleVersionKeydown = (index, event) => {
  if (event.key === '.') {
    event.preventDefault()
    // 自动跳转到下一个输入框
  }
}

// 提交表单
const handleSubmit = async () => {
  modal.btnLoading = true
  try {
    const submitData = { ...modal.data }
    submitData.mid = submitData.mid || null
    const res = await verApi.submit(submitData)
    modal.btnLoading = false
    if (res.code !== 200) {
      Message.error(res.msg)
      return
    }
    modal.visible = false
    Message.success(res.msg)
    // 如果是新版本，更新版本分组
    if (!submitData.id && !verGroup.value.some(v => v.ver_key === submitData.ver_key)) {
      verGroup.value.push({ name: submitData.name, ver_key: submitData.ver_key })
    }
    loadData()
  } catch (e) {
    modal.btnLoading = false
    Message.error('出错了：' + e)
  }
}

// 删除版本
const handleDelete = async (id) => {
  try {
    const res = await verApi.del(id)
    if (res.code !== 200) {
      Message.error(res.msg)
      return false
    }
    Message.success(res.msg)
    loadData()
    return true
  } catch (e) {
    Message.error('出错了：' + e)
    return false
  }
}

// 批量删除
const handleBatchDelete = async () => {
  batchDeleteLoading.value = true
  try {
    const res = await verApi.delall(selectedKeys.value)
    batchDeleteLoading.value = false
    if (res.code !== 200) {
      Message.error(res.msg)
      return
    }
    selectedKeys.value = []
    Message.success(res.msg)
    loadData()
  } catch (e) {
    batchDeleteLoading.value = false
    Message.error('出错了：' + e)
  }
}

// 设置弃用状态
const handleDiscard = async (id, discard) => {
  try {
    const res = await verApi.discard(id, discard)
    if (res.code !== 200) {
      Message.error(res.msg)
      return
    }
    Message.success(res.msg)
  } catch (e) {
    Message.error('出错了：' + e)
  }
}

// 加载加密方案列表
const loadMilist = async () => {
  try {
    const res = await verApi.getMilist()
    if (res.code === 200) {
      milist.value = res.data || []
    }
  } catch (e) {
    // 忽略错误
  }
}

// 加载版本分组
const loadVerGroup = async () => {
  try {
    const res = await verApi.getGroup()
    if (res.code === 200) {
      verGroup.value = res.data || []
    }
  } catch (e) {
    // 忽略错误
  }
}

onMounted(() => {
  loadData()
  loadMilist()
  loadVerGroup()
})
</script>

<script>
export default { name: 'VersionList' }
</script>

<style scoped>
.ver-management {
  padding: 0;
}

.modal-content {
  width: 100%;
  max-width: 384px;
}

.empty-tip {
  padding: 20px;
  text-align: center;
  color: var(--color-text-3);
}

/* 主内容区 */
.main-content {
  background: rgba(255, 255, 255, 0.3);
  border-radius: 12px;
  padding: 20px;
  box-shadow: 0 2px 8px rgba(0, 0, 0, 0.06);
}

body[arco-theme='dark'] .main-content {
  background: rgba(30, 30, 30, 0.3);
}

.content-wrapper {
  width: 100%;
  max-width: 100%;
  margin: 0 auto;
}

.header-row {
  display: flex;
  align-items: center;
  justify-content: space-between;
  margin-bottom: 20px;
}

.header-left {
  width: 100%;
}

.header-right {
  width: 100%;
  overflow-x: auto;
  -webkit-overflow-scrolling: touch;
  scrollbar-width: none;
  -ms-overflow-style: none;
}

.header-right::-webkit-scrollbar {
  display: none;
}

.search-form {
  display: flex;
  flex-direction: row;
  gap: 12px;
  justify-content: flex-end;
  align-items: center;
  width: max-content;
  min-width: 100%;
}

.search-form :deep(.arco-form-item) {
  margin-bottom: 0;
}

.filter-select {
  width: 150px;
}

.filter-select :deep(.arco-select-view) {
  height: 32px;
}

.keyword-input {
  width: 280px;
}

.keyword-input :deep(.arco-input-wrapper) {
  height: 32px;
}

/* 表格卡片 */
.table-card {
  background: transparent;
}

/* 表格透明化 */
.table-card :deep(.arco-table),
.table-card :deep(.arco-table-container),
.table-card :deep(.arco-table-th),
.table-card :deep(.arco-table-td),
.table-card :deep(.arco-table-tr),
.table-card :deep(.arco-table-header),
.table-card :deep(.arco-table-header-wrap),
.table-card :deep(.arco-table-body) {
  background: transparent !important;
}

.table-card :deep(.arco-table-tr:hover .arco-table-td) {
  background: rgba(var(--primary-6), 0.05) !important;
}

.table-card :deep(.arco-table-th) {
  background: rgba(0, 0, 0, 0.02) !important;
}

body[arco-theme='dark'] .table-card :deep(.arco-table-th) {
  background: rgba(255, 255, 255, 0.02) !important;
}

/* 弃用复选框 */
.custom-checkbox {
  width: 16px;
  height: 16px;
  border: 2px solid var(--color-fill-3);
  border-radius: 2px;
  display: flex;
  align-items: center;
  justify-content: center;
}

.custom-checkbox.checked {
  background: rgb(var(--red-6));
  border-color: rgb(var(--red-6));
}

.custom-checkbox.checked svg {
  color: white;
  font-size: 12px;
}

/* 分页栏 */
.pagination-bar {
  display: flex;
  justify-content: space-between;
  align-items: center;
  padding: 16px 0;
  border-top: 1px solid var(--color-border-1);
}

.pagination-left {
  display: flex;
  align-items: center;
  gap: 12px;
}

.pagination-right {
  display: flex;
  justify-content: flex-end;
}

.selected-info {
  font-size: 13px;
  color: var(--color-text-3);
}

.pagination-info {
  font-size: 13px;
  color: var(--color-text-3);
}

.text-gray {
  color: var(--color-text-3);
}

/* 响应式 */
@media (max-width: 768px) {
  .header-row {
    flex-direction: column;
    align-items: stretch;
    gap: 16px;
  }

  .header-left {
    display: flex;
    flex-wrap: wrap;
    gap: 8px;
  }

  .header-left .arco-btn {
    margin-bottom: 0 !important;
  }

  .header-right {
    width: 100%;
  }

  .search-form {
    flex-wrap: nowrap;
  }

  .filter-select {
    width: 120px;
  }

  .keyword-input {
    width: 180px;
  }

  .pagination-bar {
    flex-direction: column;
    gap: 12px;
    text-align: center;
  }

  .pagination-left,
  .pagination-right {
    width: 100%;
    justify-content: center;
  }
}
</style>