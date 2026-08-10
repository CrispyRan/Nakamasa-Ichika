<template>
  <div class="encryption-management">
    <!-- 添加/编辑弹窗 -->
    <a-modal
      v-model:visible="modal.visible"
      :title="modal.title"
      :width="400"
      :fullscreen="isMobile"
      title-align="start"
      :mask-closable="false"
    >
      <div class="modal-content">
        <a-form :model="modal.data" auto-label-width :layout="isMobile ? 'vertical' : 'horizontal'">
          <a-form-item field="name" label="方案名称">
            <a-input v-model="modal.data.name" placeholder="如：1.0安卓版RSA加密" />
          </a-form-item>
          <a-form-item field="type" label="加密类型">
            <a-select v-model="modal.data.type" placeholder="请选择加密类型" @change="handleTypeChange">
              <a-option v-for="item in plugList" :key="item.id" :value="item.id">
                {{ item.name }}
              </a-option>
            </a-select>
            <template #extra v-if="modal.data.type">
              <span v-if="modal.data.type === 'rsa'">
                加解密在线测试/生成密钥：
                <a href="http://www.51154393.cn/tool/rsa" target="_blank" class="text-blue-500">点击前往</a>
              </span>
              <span v-else>
                加解密在线测试：
                <a :href="`http://www.51154393.cn/tool/${modal.data.type}`" target="_blank" class="text-blue-500">点击前往</a>
              </span>
            </template>
          </a-form-item>
          <!-- 动态配置字段 -->
          <a-form-item
            v-for="(field, key) in currentPlugForm"
            :key="key"
            :field="`config.${key}`"
            :label="field.name"
          >
            <a-textarea
              v-if="field.type === 'textarea'"
              v-model="modal.data.config[key]"
              :placeholder="field.placeholder"
              :auto-size="{ minRows: 3, maxRows: 6 }"
            />
            <a-select
              v-else-if="field.type === 'select'"
              v-model="modal.data.config[key]"
              :placeholder="field.placeholder"
              :default-value="field.default"
            >
              <a-option v-for="opt in field.options" :key="opt" :value="opt">{{ opt }}</a-option>
            </a-select>
            <a-input
              v-else
              v-model="modal.data.config[key]"
              :placeholder="field.placeholder"
              :max-length="field.maxLength"
              show-word-limit
            >
              <template #suffix v-if="field.maxLength">
                <icon-refresh class="cursor-pointer" @click="generateRandom(key, field.maxLength)" />
              </template>
            </a-input>
          </a-form-item>
          <a-form-item field="sign" label="数据签名">
            <a-radio-group v-model="modal.data.sign">
              <a-radio value="y">验证</a-radio>
              <a-radio value="n">不验证</a-radio>
            </a-radio-group>
          </a-form-item>
          <a-form-item field="time" label="时差校验">
            <a-input-number v-model="modal.data.time" placeholder="0则不校验" :min="0">
              <template #append>秒</template>
            </a-input-number>
          </a-form-item>
          <a-form-item field="all" label="应用">
            <a-radio-group v-model="modal.data.all">
              <a-radio value="n">当前应用</a-radio>
              <a-radio value="y">全局应用</a-radio>
            </a-radio-group>
          </a-form-item>
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
              新建加密方案
            </a-button>
          </div>
          <div class="header-right">
            <a-form :model="searchForm" class="search-form" auto-label-width>
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

        <!-- 数据卡片网格 -->
        <div class="table-card">
          <a-spin :loading="loading" tip="加载中" class="w-full">
            <template v-if="tableData.length > 0">
              <div class="card-grid">
                <a-card v-for="item in tableData" :key="item.id" hoverable class="encryption-card">
                  <div class="card-header">
                    <span class="card-title">{{ item.name }}</span>
                    <a-dropdown trigger="hover" v-if="hasPermission">
                      <a-button type="text" size="small" class="more-btn">
                        <icon-more-vertical />
                      </a-button>
                      <template #content>
                        <a-doption v-if="canEdit" @click="handleEdit(item)">编辑</a-doption>
                        <a-popconfirm
                          type="warning"
                          position="tr"
                          :content="`确认删除：${item.name} ？`"
                          @ok="handleDelete(item.id)"
                        >
                          <a-doption v-if="canDelete" class="text-red-500">删除</a-doption>
                        </a-popconfirm>
                      </template>
                    </a-dropdown>
                  </div>
                  <div class="card-row">
                    <span class="label">加密方式</span>
                    <a-tag :color="typeColors[item.type] || 'gray'" size="small">{{ item.type?.toUpperCase() }}</a-tag>
                  </div>
                  <div class="card-row">
                    <span class="label">时差校验</span>
                    <span>
                      {{ item.time }}秒
                      <a-tooltip v-if="Math.abs(serverTimeDiff) > 3" :content="`服务器时间与本地时间时差：${Math.abs(serverTimeDiff)}秒`">
                        <icon-exclamation-circle class="text-red-500 ml-1" />
                      </a-tooltip>
                    </span>
                  </div>
                  <div class="card-row">
                    <span class="label">数据签名</span>
                    <a-switch
                      v-model="item.sign"
                      checked-value="y"
                      unchecked-value="n"
                      size="small"
                      :disabled="!canEdit"
                      @change="handleSignChange(item)"
                    />
                  </div>
                </a-card>
              </div>

              <!-- 分页 -->
              <div class="pagination-bar">
                <div class="pagination-left">
                  <span class="pagination-info">当前第 {{ pagination.current }} 页 共 {{ pagination.pageTotal }} 页 {{ pagination.total }} 条结果</span>
                </div>
                <div class="pagination-right">
                  <a-pagination
                    v-model:current="pagination.current"
                    :total="pagination.total"
                    :page-size="12"
                    show-total
                    @change="handlePageChange"
                  />
                </div>
              </div>
            </template>
            <template v-else>
              <a-empty description="暂未创建加密方案" class="empty-state" />
            </template>
          </a-spin>
        </div>
      </div>
    </div>
  </div>
</template>

<script setup>
import { ref, reactive, computed, onMounted } from 'vue'
import { Message } from '@arco-design/web-vue'
import encryptionApi from '@/api/system/encryption'
import { useRouter } from 'vue-router'

const router = useRouter()
const isMobile = computed(() => router.currentRoute.value.meta?.isMobile)

// 加密类型颜色映射
const typeColors = {
  rc4: 'cyan',
  des: 'blue',
  aes: 'arcoblue',
  rsa: 'red'
}

// 权限判断（简化版，实际应从store获取）
const hasPermission = ref(true)
const canEdit = ref(true)
const canDelete = ref(true)

// 搜索
const searchForm = reactive({ keyword: '' })
const searchLoading = ref(false)

// 表格数据
const tableData = ref([])
const loading = ref(false)
const serverTimeDiff = ref(0)
const pagination = reactive({ current: 1, total: 0, pageTotal: 0 })

// 加密插件列表
const plugList = ref([])

// 弹窗
const modal = reactive({
  visible: false,
  title: '添加方案',
  btnLoading: false,
  data: {
    id: null,
    name: '',
    type: '',
    config: {},
    sign: 'y',
    time: 60,
    all: 'n'
  }
})

// 当前加密类型的配置表单
const currentPlugForm = ref({})

// 获取默认表单数据
const getFormData = () => ({
  id: null,
  name: '',
  type: '',
  config: {},
  sign: 'y',
  time: 60,
  all: 'n'
})

// 加载数据
const loadData = async () => {
  loading.value = true
  try {
    const res = await encryptionApi.getList({
      page: pagination.current,
      size: 12,
      keyword: searchForm.keyword
    })
    if (res.code === 200) {
      tableData.value = res.data.list || []
      pagination.total = res.data.dataTotal || 0
      pagination.pageTotal = res.data.pageTotal || 0
    }
  } catch (e) {
    Message.error('加载数据失败')
  } finally {
    loading.value = false
  }
}

// 加载插件列表
const loadPlugList = async () => {
  try {
    const res = await encryptionApi.getPlug()
    if (res.code === 200) {
      plugList.value = res.data || []
    }
  } catch (e) {
    console.error('加载插件列表失败', e)
  }
}

// 搜索
const handleSearch = () => {
  pagination.current = 1
  searchLoading.value = true
  loadData().finally(() => searchLoading.value = false)
}

// 分页
const handlePageChange = (page) => {
  pagination.current = page
  loadData()
}

// 添加
const handleAdd = () => {
  modal.title = '添加方案'
  modal.data = getFormData()
  currentPlugForm.value = {}
  modal.visible = true
}

// 编辑
const handleEdit = (record) => {
  modal.title = '编辑方案'
  modal.data = {
    id: record.id,
    name: record.name,
    type: record.type,
    config: record.config || {},
    sign: record.sign || 'y',
    time: record.time || 60,
    all: record.appid ? 'n' : 'y'
  }
  // 设置当前类型的表单
  handleTypeChange(record.type)
  modal.visible = true
}

// 加密类型改变
const handleTypeChange = (type) => {
  const plug = plugList.value.find(p => p.id === type)
  if (plug && plug.form) {
    currentPlugForm.value = plug.form
    // 初始化config中不存在的字段
    for (const key in plug.form) {
      if (modal.data.config[key] === undefined) {
        modal.data.config[key] = plug.form[key].default || ''
      }
    }
  } else {
    currentPlugForm.value = {}
  }
}

// 生成随机字符串
const generateRandom = (key, length = 16) => {
  const chars = 'ABCDEFGHJKLMNPQRSTUVWXYZabcdefghjkmnpqrstuvwxyz23456789'
  let result = ''
  for (let i = 0; i < length; i++) {
    result += chars.charAt(Math.floor(Math.random() * chars.length))
  }
  modal.data.config[key] = result
}

// 提交
const handleSubmit = async () => {
  if (!modal.data.name) {
    Message.warning('请输入方案名称')
    return
  }
  if (!modal.data.type) {
    Message.warning('请选择加密类型')
    return
  }
  
  modal.btnLoading = true
  try {
    const res = await encryptionApi.submit(modal.data)
    if (res.code === 200) {
      Message.success(res.msg || '操作成功')
      modal.visible = false
      loadData()
    } else {
      Message.error(res.msg || '操作失败')
    }
  } catch (e) {
    Message.error('操作失败')
  } finally {
    modal.btnLoading = false
  }
}

// 删除
const handleDelete = async (id) => {
  try {
    const res = await encryptionApi.del(id)
    if (res.code === 200) {
      Message.success('删除成功')
      loadData()
    } else {
      Message.error(res.msg || '删除失败')
    }
  } catch (e) {
    Message.error('删除失败')
  }
}

// 签名状态切换
const handleSignChange = async (item) => {
  try {
    const res = await encryptionApi.editSignStatus(item.id, item.sign)
    if (res.code === 200) {
      Message.success('操作成功')
    } else {
      Message.error(res.msg || '操作失败')
      item.sign = item.sign === 'y' ? 'n' : 'y'
    }
  } catch (e) {
    Message.error('操作失败')
    item.sign = item.sign === 'y' ? 'n' : 'y'
  }
}

onMounted(() => {
  loadPlugList()
  loadData()
})
</script>

<script>
export default { name: 'EncryptionList' }
</script>

<style scoped>
.encryption-management {
  padding: 0;
}

.modal-content {
  max-width: 380px;
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
}

.search-form {
  display: flex;
  flex-direction: row;
  gap: 12px;
  justify-content: flex-end;
  align-items: center;
}

.search-form :deep(.arco-form-item) {
  margin-bottom: 0;
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

/* 卡片网格 */
.card-grid {
  display: grid;
  grid-template-columns: repeat(4, 1fr);
  gap: 16px;
}

@media (max-width: 1200px) {
  .card-grid {
    grid-template-columns: repeat(2, 1fr);
  }
}

@media (max-width: 768px) {
  .card-grid {
    grid-template-columns: 1fr;
  }
}

.encryption-card {
  background: rgba(255, 255, 255, 0.5) !important;
  border: 1px solid rgba(0, 0, 0, 0.05);
  transition: all 0.3s;
}

.encryption-card:hover {
  transform: translateY(-2px);
  box-shadow: 0 4px 12px rgba(0, 0, 0, 0.1);
}

body[arco-theme='dark'] .encryption-card {
  background: rgba(50, 50, 50, 0.5) !important;
  border-color: rgba(255, 255, 255, 0.05);
}

.encryption-card :deep(.arco-card-body) {
  padding: 16px;
}

.card-header {
  display: flex;
  align-items: center;
  justify-content: space-between;
  margin-bottom: 12px;
}

.card-title {
  font-weight: 500;
  font-size: 14px;
}

.more-btn {
  padding: 4px;
}

.card-row {
  display: flex;
  align-items: center;
  justify-content: space-between;
  margin-bottom: 8px;
  font-size: 13px;
}

.card-row .label {
  color: var(--color-text-3);
}

/* 分页栏 */
.pagination-bar {
  display: flex;
  justify-content: space-between;
  align-items: center;
  padding: 16px 0;
  border-top: 1px solid var(--color-border-1);
  margin-top: 16px;
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

.pagination-info {
  font-size: 13px;
  color: var(--color-text-3);
}

.empty-state {
  padding: 60px 0;
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
    justify-content: flex-start;
  }

  .keyword-input {
    width: 100%;
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