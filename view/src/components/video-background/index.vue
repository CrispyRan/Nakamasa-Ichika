<template>
  <Transition name="video-fade">
    <div v-if="visible" class="video-bg-container">
      <video
        ref="videoRef"
        autoplay
        :muted="!appStore.videoSound"
        loop
        playsinline
        :poster="poster"
        @pause="handlePause"
        @ended="handleEnded"
        @error="handleError"
      >
        <source :src="src" type="video/mp4" />
      </video>
    </div>
  </Transition>
</template>

<script setup>
import { ref, watch, onMounted, onUnmounted } from 'vue'
import { useAppStore } from '@/store'

const props = defineProps({
  src: {
    type: String,
    default: '/video/background.mp4'
  },
  poster: {
    type: String,
    default: ''
  }
})

const appStore = useAppStore()
const videoRef = ref(null)
const visible = ref(false)
let retryTimer = null
let visibilityHandler = null

// 尝试播放视频
const tryPlay = () => {
  if (!videoRef.value || !visible.value) return
  
  // 确保静音状态正确
  videoRef.value.muted = !appStore.videoSound
  
  videoRef.value.play().catch((e) => {
    console.log('[VideoBG] 播放被阻止，稍后重试:', e.message)
    // 播放被阻止，延迟重试
    scheduleRetry()
  })
}

// 延迟重试播放
const scheduleRetry = () => {
  if (retryTimer) clearTimeout(retryTimer)
  retryTimer = setTimeout(() => {
    if (visible.value && videoRef.value?.paused) {
      tryPlay()
    }
  }, 1000)
}

// 更新视频静音状态
const updateMuted = () => {
  if (videoRef.value) {
    videoRef.value.muted = !appStore.videoSound
  }
}

// 视频暂停事件处理
const handlePause = (e) => {
  // 如果不是因为用户主动暂停（如切换标签页、浏览器节能等），尝试恢复播放
  if (visible.value && videoRef.value) {
    // 检查是否是页面不可见导致的暂停
    if (document.hidden) {
      // 页面不可见时不立即恢复，等页面可见时再恢复
      return
    }
    // 延迟检查是否需要恢复播放
    setTimeout(() => {
      if (visible.value && videoRef.value?.paused && !document.hidden) {
        tryPlay()
      }
    }, 100)
  }
}

// 视频结束事件处理
const handleEnded = () => {
  // loop 属性应该自动循环，但某些情况下可能失效，手动重新播放
  if (visible.value && videoRef.value) {
    videoRef.value.currentTime = 0
    tryPlay()
  }
}

// 视频错误事件处理
const handleError = (e) => {
  console.error('[VideoBG] 视频加载错误:', e)
  // 尝试重新加载
  if (videoRef.value) {
    videoRef.value.load()
    setTimeout(() => tryPlay(), 500)
  }
}

// 页面可见性变化处理
const handleVisibilityChange = () => {
  if (!document.hidden && visible.value && videoRef.value?.paused) {
    // 页面重新可见时，恢复播放
    tryPlay()
  }
}

// 监听皮肤变化
watch(
  () => appStore.skin,
  (newSkin) => {
    visible.value = newSkin === 'video'
    if (visible.value && videoRef.value) {
      updateMuted()
      tryPlay()
    }
  },
  { immediate: true }
)

// 监听声音开关变化
watch(
  () => appStore.videoSound,
  () => {
    updateMuted()
  }
)

onMounted(() => {
  visible.value = appStore.skin === 'video'
  
  // 尝试自动播放
  if (visible.value && videoRef.value) {
    updateMuted()
    tryPlay()
  }
  
  // 监听页面可见性变化
  visibilityHandler = handleVisibilityChange
  document.addEventListener('visibilitychange', visibilityHandler)
})

onUnmounted(() => {
  // 清理定时器
  if (retryTimer) {
    clearTimeout(retryTimer)
    retryTimer = null
  }
  
  // 移除事件监听
  if (visibilityHandler) {
    document.removeEventListener('visibilitychange', visibilityHandler)
    visibilityHandler = null
  }
})
</script>

<style scoped lang="less">
.video-fade-enter-active,
.video-fade-leave-active {
  transition: opacity 0.5s ease;
}

.video-fade-enter-from,
.video-fade-leave-to {
  opacity: 0;
}

.video-bg-container {
  position: fixed;
  top: 0;
  left: 0;
  width: 100vw;
  height: 100vh;
  z-index: -1;
  overflow: hidden;
  
  &::after {
    content: '';
    position: absolute;
    top: 0;
    left: 0;
    width: 100%;
    height: 100%;
    background: linear-gradient(
      135deg,
      rgba(0, 0, 0, 0.15) 0%,
      rgba(0, 0, 0, 0.08) 50%,
      rgba(0, 0, 0, 0.2) 100%
    );
    pointer-events: none;
  }
  
  video {
    width: 100%;
    height: 100%;
    object-fit: cover;
  }
}
</style>