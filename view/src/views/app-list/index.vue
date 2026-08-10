<template>
  <div class="app-list-page w-full h-full p-6">
    <!-- 搜索和操作栏 -->
    <div class="mb-6 flex flex-col lg:flex-row lg:items-center lg:justify-between gap-4">
      <div class="flex items-center gap-3">
        <h2 class="text-xl font-semibold m-0">应用中心</h2>
        <a-tag color="arcoblue" size="small">{{ pagination.total }} 个应用</a-tag>
      </div>
      <div class="flex items-center gap-3">
        <a-input-search
          v-model="searchKeyword"
          placeholder="搜索应用名称或ID"
          style="width: 240px"
          allow-clear
          @search="handleSearch"
          @clear="handleSearch"
        />
        <a-select
          v-model="filterType"
          placeholder="应用类型"
          style="width: 120px"
          allow-clear
          @change="handleSearch"
        >
          <a-option value="user">用户版</a-option>
          <a-option value="kami">卡密版</a-option>
          <a-option value="api">API服务</a-option>
        </a-select>
        <a-button type="primary" @click="handleAddApp">
          <template #icon><icon-plus /></template>
          添加应用
        </a-button>
      </div>
    </div>

    <!-- 应用卡片列表 -->
    <a-spin :loading="loading" class="w-full">
      <div v-if="appList.length > 0" class="grid gap-5 grid-cols-1 md:grid-cols-2 lg:grid-cols-3 xl:grid-cols-4">
        <a-card
          v-for="app in appList"
          :key="app.id"
          class="app-card cursor-pointer hover:shadow-lg transition-all duration-300"
          :bordered="true"
          hoverable
          @click="handleEnterApp(app)"
        >
          <template #title>
            <div class="flex items-center gap-3">
              <a-avatar :size="40" class="bg-gradient-to-br from-blue-500 to-purple-600">
                <img v-if="app.app_logo" :src="tool.attachUrl(app.app_logo)" :alt="app.app_name" />
                <icon-apps v-else />
              </a-avatar>
              <div class="flex-1 min-w-0">
                <div class="flex items-center gap-2">
                  <span class="font-medium truncate">{{ app.app_name }}</span>
                  <a-tag v-if="app.app_state === 'on'" color="green" size="small">运行中</a-tag>
                  <a-tag v-else color="red" size="small">已停止</a-tag>
                </div>
                <span class="text-xs text-gray-500">{{ getAppTypeName(app.app_type) }}</span>
              </div>
            </div>
          </template>
          <template #extra>
            <a-dropdown @select="(val) => handleAction(val, app)" trigger="click" @click.stop>
              <a-button type="text" size="small">
                <template #icon><icon-more /></template>
              </a-button>
              <template #content>
                <a-doption value="edit"><icon-edit /> 编辑</a-doption>
                <a-doption value="copy"><icon-copy /> 复制AppKey</a-doption>
                <a-doption value="delete" class="text-red-500"><icon-delete /> 删除</a-doption>
              </template>
            </a-dropdown>
          </template>
          <div class="text-sm text-gray-600">
            <p class="mb-2 truncate">{{ app.app_off_msg || '暂无描述' }}</p>
            <div class="flex items-center justify-between text-xs text-gray-400 mb-3">
              <span>Key: {{ app.app_key }}</span>
              <span>ID: {{ app.id }}</span>
            </div>
            <a-button type="primary" long size="small" @click.stop="handleEnterApp(app)">
              <template #icon><icon-export /></template>
              进入管理
            </a-button>
          </div>
        </a-card>
      </div>

      <!-- 空状态 -->
      <a-empty v-else class="py-20">
        <template #image>
          <icon-apps :size="64" :style="{ color: 'var(--color-text-3)' }" />
        </template>
        <template #description>
          <span class="text-gray-400">暂无应用，点击上方按钮添加</span>
        </template>
      </a-empty>
    </a-spin>

    <!-- 分页 -->
    <div v-if="appList.length > 0" class="mt-6 flex justify-center">
      <a-pagination
        v-model:current="pagination.current"
        :total="pagination.total"
        :page-size="pagination.pageSize"
        show-total
        show-jumper
        @change="handlePageChange"
      />
    </div>

    <!-- 添加应用弹窗 -->
    <a-modal
      v-model:visible="addModalVisible"
      title="添加应用"
      :width="480"
      :loading="addLoading"
      @ok="handleAddConfirm"
      @cancel="addModalVisible = false"
    >
      <a-form :model="addForm" layout="vertical">
        <a-form-item label="应用名称" required>
          <a-input v-model="addForm.app_name" placeholder="请输入应用名称 (2-64位)" />
        </a-form-item>
        <a-form-item label="应用类型">
          <a-select v-model="addForm.app_type" placeholder="请选择应用类型">
            <a-option value="user">用户版</a-option>
            <a-option value="kami">卡密版</a-option>
            <a-option value="api">API服务</a-option>
          </a-select>
        </a-form-item>
        <a-form-item label="继承应用">
          <a-select 
            v-model="addForm.app_inherit" 
            placeholder="可选：继承已有应用的配置"
            allow-clear
          >
            <a-option 
              v-for="inherit in inheritApps" 
              :key="inherit.id" 
              :value="inherit.id"
            >
              {{ inherit.app_name }}
            </a-option>
          </a-select>
        </a-form-item>
        <a-form-item label="应用Logo">
          <sa-upload-image v-model="addForm.app_logo" />
        </a-form-item>
      </a-form>
    </a-modal>
  </div>
</template>

<script setup>
import { ref, onMounted } from 'vue'
import { useRouter } from 'vue-router'
import { Message, Modal } from '@arco-design/web-vue'
import appApi from '@/api/system/app'
import { useUserStore } from '@/store'
import tool from '@/utils/tool'

const router = useRouter()
const userStore = useUserStore()

// 加载状态
const loading = ref(false)
const addLoading = ref(false)

// 搜索关键词
const searchKeyword = ref('')
const filterType = ref('')

// 应用列表数据
const appList = ref([])
const inheritApps = ref([])

// 分页配置
const pagination = ref({
  current: 1,
  pageSize: 12,
  total: 0
})

// 添加应用弹窗
const addModalVisible = ref(false)
const addForm = ref({
  app_name: '',
  app_type: 'user',
  app_inherit: null,
  app_logo: ''
})

// 获取应用类型名称
const getAppTypeName = (type) => {
  const typeMap = {
    'user': '用户版',
    'kami': '卡密版',
    'api': 'API服务',
    'web': 'Web应用',
    'miniapp': '小程序'
  }
  return typeMap[type] || type
}

// 获取应用列表
const fetchAppList = async () => {
  loading.value = true
  try {
    const res = await appApi.getList({
      pg: pagination.value.current,
      so: {
        keyword: searchKeyword.value || undefined,
        type: filterType.value || undefined
      }
    })
    
    if (res.code === 200 && res.data) {
      appList.value = res.data.list || []
      pagination.value.total = res.data.dataTotal || 0
    }
  } catch (e) {
    console.error('获取应用列表失败:', e)
    Message.error('获取应用列表失败')
  } finally {
    loading.value = false
  }
}

// 获取可继承的应用列表
const fetchInheritApps = async () => {
  try {
    const res = await appApi.getInherit()
    if (res.code === 200 && res.data) {
      inheritApps.value = [
        ...(res.data.user || []),
        ...(res.data.kami || [])
      ]
    }
  } catch (e) {
    console.error('获取继承列表失败:', e)
  }
}

// 搜索处理
const handleSearch = () => {
  pagination.value.current = 1
  fetchAppList()
}

// 分页处理
const handlePageChange = (page) => {
  pagination.value.current = page
  fetchAppList()
}

// 添加应用
const handleAddApp = () => {
  addForm.value = {
    app_name: '',
    app_type: 'user',
    app_inherit: null,
    app_logo: ''
  }
  fetchInheritApps()
  addModalVisible.value = true
}

// 确认添加
const handleAddConfirm = async () => {
  if (!addForm.value.app_name) {
    Message.warning('请输入应用名称')
    return
  }
  
  addLoading.value = true
  try {
    const res = await appApi.add(addForm.value)
    if (res.code === 200) {
      Message.success('添加成功')
      addModalVisible.value = false
      fetchAppList()
    } else {
      Message.error(res.msg || '添加失败')
    }
  } catch (e) {
    Message.error('添加失败')
  } finally {
    addLoading.value = false
  }
}

// 进入应用
const handleEnterApp = (app) => {
  // 使用 userStore 设置当前应用（会自动刷新路由）
  userStore.setCurrentApp({
    id: app.id,
    name: app.app_name,
    app_type: app.app_type,
    logo: app.app_logo
  })
  
  // 跳转到仪表盘
  router.push('/dashboard')
}

// 操作处理
const handleAction = async (action, app) => {
  switch (action) {
    case 'edit':
      // 使用 userStore 设置当前应用
      userStore.setCurrentApp({
        id: app.id,
        name: app.app_name,
        app_type: app.app_type,
        logo: app.app_logo
      })
      router.push('/app/info')
      break
    case 'copy':
      try {
        await navigator.clipboard.writeText(app.app_key)
        Message.success('AppKey 已复制到剪贴板')
      } catch {
        Message.success(`AppKey: ${app.app_key}`)
      }
      break
    case 'delete':
      const confirmed = await new Promise(resolve => {
        Modal.confirm({
          title: '确认删除',
          content: `确定要删除应用"${app.app_name}"吗？此操作不可恢复。`,
          onOk: () => resolve(true),
          onCancel: () => resolve(false)
        })
      })
      
      if (confirmed) {
        try {
          const res = await appApi.del(app.id)
          if (res.code === 200) {
            Message.success('删除成功')
            fetchAppList()
          } else {
            Message.error(res.msg || '删除失败')
          }
        } catch {
          Message.error('删除失败')
        }
      }
      break
  }
}

onMounted(() => {
  fetchAppList()
})
</script>

<script>
export default { name: 'appList' }
</script>

<style scoped lang="less">
.app-list-page {
  min-height: calc(100% - 12px);
}

.app-card {
  :deep(.arco-card-header) {
    border-bottom: none;
    padding-bottom: 0;
  }
}
</style>