<template>
  <div class="block">
    <div class="user-header rounded-sm text-center">
      <div class="pt-3 mx-auto avatar-box">
        <sa-upload-image v-model="userInfo.avatar" rounded />
      </div>
      <div>
        <a-tag size="large" class="mt-3 rounded-full tag-primary">
          {{ (userStore.user && userStore.user.nickname) || (userStore.user && userStore.user.username) }}
        </a-tag>
      </div>
    </div>

    <a-layout-content class="block lg:flex lg:justify-between">
      <div class="ma-content-block w-full lg:w-6/12 mt-3 p-4">
        <a-tabs type="rounded">
          <a-tab-pane key="info" title="个人资料">
            <user-infomation />
          </a-tab-pane>
          <a-tab-pane key="safe" title="安全设置">
            <modify-password />
          </a-tab-pane>
        </a-tabs>
      </div>
      <div class="ma-content-block w-full lg:w-6/12 mt-3 p-4 ml-0 lg:ml-3">
        <a-tabs type="rounded">
          <a-tab-pane key="update-log" title="更新日志">
            <div class="update-log-container">
              <a-spin :loading="uplogLoading" class="w-full">
                <a-timeline v-if="uplogList && uplogList.length">
                  <a-timeline-item v-for="(item, idx) in uplogList" :key="idx">
                    <div class="update-item">
                      <div class="update-header">
                        <span class="version-badge">
                          <span class="ver-num">v{{ item.ver }}</span>
                          <span v-if="item.revision" class="revision">r{{ item.revision }}</span>
                        </span>
                        <span class="update-time">{{ formatTime(item.time) }}</span>
                        <a-tag v-if="item.type === 'official'" color="arcoblue" size="small">正式版</a-tag>
                      </div>
                      <div class="update-content" v-html="DOMPurify.sanitize(item.content)"></div>
                    </div>
                  </a-timeline-item>
                </a-timeline>
                <a-empty v-else-if="!uplogLoading" />
              </a-spin>
            </div>
          </a-tab-pane>
          <a-tab-pane key="login-log" title="登录日志">
            <a-timeline class="pl-5 mt-3" v-if="loginLogList && loginLogList.length">
              <a-timeline-item :label="`地理位置；${item.ip_location}，操作系统：${item.os}`" v-for="(item, idx) in loginLogList" :key="idx">
                您于 {{ item.login_time }} 登录系统，{{ item.message }}
              </a-timeline-item>
            </a-timeline>
            <a-empty v-else />
          </a-tab-pane>
          <a-tab-pane key="operation-log" title="操作日志">
            <a-timeline class="pl-5 mt-3" v-if="operationLogList && operationLogList.length">
              <a-timeline-item
                :label="`地理位置；${item.ip_location}，方式：${item.method}，路由：${item.router}`"
                v-for="(item, idx) in operationLogList"
                :key="idx">
                您于 {{ item.create_time }} 执行了 {{ item.service_name }}
              </a-timeline-item>
            </a-timeline>
            <a-empty v-else />
          </a-tab-pane>
        </a-tabs>
      </div>
    </a-layout-content>
  </div>
</template>

<script setup>
import { ref, reactive, onMounted, watch } from 'vue'
import { useUserStore } from '@/store'
import { Message } from '@arco-design/web-vue'
import user from '@/api/system/user'
import commonApi from '@/api/common'
import systemApi from '@/api/system/system'

import ModifyPassword from './components/modifyPassword.vue'
import DOMPurify from 'dompurify'
import UserInfomation from './components/userInfomation.vue'

const userStore = useUserStore()
const userInfo = reactive({
  ...userStore.user,
})

const loginLogList = ref([])
const operationLogList = ref([])
const uplogList = ref([])
const uplogLoading = ref(false)

const requestParams = reactive({
  limit: 5,
})

const formatTime = (timestamp) => {
  const date = new Date(timestamp * 1000)
  return date.toLocaleDateString('zh-CN', {
    year: 'numeric',
    month: '2-digit',
    day: '2-digit'
  })
}

const fetchUplog = async () => {
  uplogLoading.value = true
  try {
    const res = await systemApi.getUplog()
    if (res.code === 200 && res.data) {
      uplogList.value = res.data
    }
  } catch (e) {
    console.error('获取更新日志失败', e)
  } finally {
    uplogLoading.value = false
  }
}

onMounted(() => {
  commonApi.getLoginLogList(Object.assign(requestParams, { orderBy: 'login_time', orderType: 'desc' })).then((res) => {
    loginLogList.value = res.data.data
  })

  commonApi.getOperationLogList(Object.assign(requestParams, { orderBy: 'create_time', orderType: 'desc' })).then((res) => {
    operationLogList.value = res.data.data
  })

  fetchUplog()
})

userInfo.avatar = userStore?.user?.avatar ?? undefined

watch(
  () => userInfo.avatar,
  async (newAvatar) => {
    if (newAvatar) {
      const response = await user.updateInfo({ avatar: newAvatar })
      if (response.code === 200) {
        Message.success('头像修改成功')
        userStore.user.avatar = newAvatar
      }
    }
  }
)
</script>
<script>
export default { name: 'userCenter' }
</script>

<style scoped>
.avatar-box {
  width: 130px;
}
.user-header {
  width: 100%;
  height: 200px;
  background: url('@/assets/userBanner.jpg') no-repeat;
  background-size: cover;
}

.update-log-container {
  max-height: 500px;
  overflow-y: auto;
  padding-right: 8px;
}

.update-log-container :deep(.arco-timeline-item-dot) {
  background: #165dff !important;
  border: 2px solid #165dff !important;
  width: 8px !important;
  height: 8px !important;
  min-width: 8px !important;
}

.update-log-container :deep(.arco-timeline-item-line) {
  background: #e5e6eb !important;
  width: 2px !important;
}

.version-badge {
  display: inline-flex;
  align-items: center;
  gap: 4px;
  background: var(--color-primary);
  color: #fff;
  padding: 2px 8px;
  border-radius: 4px;
  font-size: 12px;
  font-weight: 500;
}

.version-badge .ver-num {
  font-size: 13px;
}

.version-badge .revision {
  font-size: 11px;
  opacity: 0.85;
}

.update-item {
  padding-left: 12px;
}

.update-header {
  display: flex;
  align-items: center;
  gap: 10px;
  margin-bottom: 6px;
}

.update-time {
  color: var(--color-text-3);
  font-size: 12px;
}

.update-content {
  color: var(--color-text-2);
  font-size: 13px;
  line-height: 1.7;
}

.update-content :deep(ol),
.update-content :deep(ul) {
  margin: 0;
  padding-left: 18px;
}

.update-content :deep(li) {
  margin: 2px 0;
}

.update-content :deep(p) {
  margin: 2px 0;
}
</style>
