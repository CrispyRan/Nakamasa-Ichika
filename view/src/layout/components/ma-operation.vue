<template>
  <div class="mr-2 flex justify-end lg:justify-between w-full lg:w-auto">
    <a-space class="mr-0 lg:mr-5" size="medium">
      <a-tooltip :content="$t('sys.store')" v-if="isDev">
        <a-button :shape="'circle'" @click="handleAppStore">
          <template #icon>
            <icon-apps :size="16" :rotate="45" />
          </template>
        </a-button>
      </a-tooltip>

      <a-tooltip :content="$t('sys.search')">
        <a-button :shape="'circle'" @click="() => (appStore.searchOpen = true)">
          <template #icon>
            <icon-search />
          </template>
        </a-button>
      </a-tooltip>

      <!-- 黑白模式切换 -->
      <a-tooltip :content="appStore.mode === 'dark' ? $t('sys.lightMode') : $t('sys.darkMode')">
        <a-button :shape="'circle'" @click="toggleDarkMode">
          <template #icon>
            <icon-moon-fill v-if="appStore.mode === 'light'" />
            <icon-sun-fill v-else />
          </template>
        </a-button>
      </a-tooltip>

      <!-- 全屏切换 -->
      <a-tooltip :content="isFullScreen ? $t('sys.closeFullScreen') : $t('sys.fullScreen')">
        <a-button :shape="'circle'" @click="screen">
          <template #icon>
            <icon-fullscreen-exit v-if="isFullScreen" />
            <icon-fullscreen v-else />
          </template>
        </a-button>
      </a-tooltip>

      <!-- 消息通知 -->
      <a-trigger trigger="click">
        <a-button :shape="'circle'">
          <template #icon>
            <a-badge
              :count="messageStore.messageList?.length || 0"
              :dotStyle="{ width: '5px', height: '5px' }"
              v-if="messageStore.messageList && messageStore.messageList.length > 0">
              <icon-notification />
            </a-badge>
            <icon-notification v-else />
          </template>
        </a-button>

        <template #content>
          <message-notification />
        </template>
      </a-trigger>

      <!-- 页面设置 -->
      <a-tooltip :content="$t('sys.pageSetting')">
        <a-button :shape="'circle'" @click="() => (appStore.settingOpen = true)">
          <template #icon>
            <icon-settings />
          </template>
        </a-button>
      </a-tooltip>
    </a-space>
    <a-dropdown @select="handleSelect" trigger="hover">
      <a-avatar class="bg-blue-500 text-3xl avatar" style="top: -1px">
        <img :src="userStore.user && userStore.user.avatar ? userStore.user.avatar : avatar" />
      </a-avatar>

      <template #content>
        <a-doption value="userCenter"><icon-user /> {{ $t('sys.userCenter') }}</a-doption>
        <a-doption value="clearCache"><icon-delete /> {{ $t('sys.clearCache') }}</a-doption>
        <a-divider style="margin: 5px 0" />
        <a-doption value="logout"><icon-poweroff /> {{ $t('sys.logout') }}</a-doption>
      </template>
    </a-dropdown>

    <a-modal v-model:visible="showLogoutModal" @ok="handleLogout" @cancel="handleLogoutCancel">
      <template #title>{{ $t('sys.logoutAlert') }}</template>
      <div>{{ $t('sys.logoutMessage') }}</div>
    </a-modal>
  </div>
</template>

<script setup>
import { ref, onMounted, onUnmounted, watch } from 'vue'
import { useAppStore, useUserStore, useMessageStore } from '@/store'
import tool from '@/utils/tool'
import MessageNotification from './components/message-notification.vue'
import { useRouter } from 'vue-router'
import { useI18n } from 'vue-i18n'
import { Message } from '@arco-design/web-vue'
import { Push } from '@/utils/push-vue'
import { info } from '@/utils/common'
import commonApi from '@/api/common'

import avatar from '@/assets/avatar.jpg'

const { t } = useI18n()
const messageStore = useMessageStore()
const userStore = useUserStore()
const appStore = useAppStore()
const setting = ref(null)
const router = useRouter()
const isFullScreen = ref(false)
const showLogoutModal = ref(false)
const isDev = ref(import.meta.env.DEV)

// WebSocket 连接实例
let wsConnection = null
let wsChannel = null

const handleSelect = async (name) => {
  if (name === 'userCenter') {
    router.push({ name: 'userCenter' })
  }
  if (name === 'clearCache') {
    const res = await commonApi.clearAllCache()
    tool.local.remove('dictData')
    res.code === 200 && Message.success(res.message)
  }
  if (name === 'logout') {
    showLogoutModal.value = true
    document.querySelector('#app').style.filter = 'grayscale(1)'
  }
}

const handleAppStore = async () => {
  window.open('https://saas.saithink.top/#/appStore', '_blank', 'noopener,noreferrer')
}

const handleLogout = async () => {
  // 断开 WebSocket
  disconnectWebSocket()
  await userStore.logout()
  document.querySelector('#app').style.filter = 'grayscale(0)'
  router.push({ name: 'login' })
}

const handleLogoutCancel = () => {
  document.querySelector('#app').style.filter = 'grayscale(0)'
}

const screen = () => {
  tool.screen(document.documentElement)
  isFullScreen.value = !isFullScreen.value
}

// 黑白模式切换
const toggleDarkMode = () => {
  const newMode = appStore.mode === 'dark' ? 'light' : 'dark'
  appStore.toggleMode(newMode)
}

// 初始化 WebSocket 连接
const initWebSocket = () => {
  if (!appStore.ws || wsConnection) return

  const env = import.meta.env
  const baseURL = env.VITE_APP_OPEN_PROXY === 'true' ? env.VITE_APP_PROXY_PREFIX : env.VITE_APP_BASE_URL
  const wsURL = env.VITE_APP_WS_URL || ''
  const appKey = env.VITE_APP_WS_APPKEY || ''

  if (!wsURL || !appKey) {
    console.warn('[WebSocket] 缺少配置: VITE_APP_WS_URL 或 VITE_APP_WS_APPKEY')
    return
  }

  try {
    // 建立连接
    wsConnection = new Push({
      url: wsURL,
      app_key: appKey,
      auth: baseURL + '/plugin/webman/push/auth',
    })

    // 创建监听频道
    wsChannel = wsConnection.subscribe('saiadmin')

    // 监听消息
    wsChannel.on('message', function (message) {
      info('新消息提示', '您有新的消息，请注意查收！')
      if (message && message.data) {
        messageStore.messageList = message.data
      }
    })

  } catch (error) {
    console.error('[WebSocket] 连接失败:', error)
  }
}

// 断开 WebSocket 连接
const disconnectWebSocket = () => {
  if (wsConnection) {
    try {
      wsConnection.unsubscribe('saiadmin')
      wsConnection.disconnect()
    } catch (e) {
      console.warn('[WebSocket] 断开连接异常:', e)
    }
    wsConnection = null
    wsChannel = null
  }
}

// 监听 ws 设置变化
watch(
  () => appStore.ws,
  (enabled) => {
    if (enabled) {
      initWebSocket()
    } else {
      disconnectWebSocket()
    }
  }
)

onMounted(() => {
  // 初始化时如果 ws 已启用，则建立连接
  if (appStore.ws) {
    initWebSocket()
  }
})

onUnmounted(() => {
  disconnectWebSocket()
})
</script>
<style scoped>
:deep(.arco-avatar-text) {
  top: 1px;
}
:deep(.arco-divider-horizontal) {
  margin: 5px 0;
}
.avatar {
  cursor: pointer;
  margin-top: 6px;
}
</style>
