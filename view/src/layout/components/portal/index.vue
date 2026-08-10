<template>
  <a-layout-content class="h-full main-container">
    <!-- 视频背景 -->
    <video-background />
    
    <a-layout class="layout flex flex-col h-full layout-portal">
      <!-- 顶部状态栏 -->
      <a-layout-header class="ma-ui-header flex justify-between h-12 layout-banner-header operation-area">
        <div class="flex items-center logo">
          <a-avatar class="ml-2" :size="32">
            <img src="../../../assets/logo.png" class="bg-white" />
          </a-avatar>
          <span class="ml-2 text-lg hidden md:block">{{ $title }}</span>
        </div>
        <ma-operation />
      </a-layout-header>
      
      <!-- 内容区域 -->
      <a-layout-content class="work-area customer-scrollbar">
        <router-view v-slot="{ Component }">
          <transition :name="appStore.animation" mode="out-in">
            <keep-alive :include="keepStore.keepAlives">
              <component :is="Component" :key="$route.fullPath" v-if="keepStore.show" />
            </keep-alive>
          </transition>
        </router-view>
      </a-layout-content>
    </a-layout>

    <!-- 设置面板 -->
    <setting ref="settingRef"/>

    <!-- 搜索 -->
    <transition name="ma-slide-down" mode="out-in">
      <system-search ref="systemSearchRef" v-show="appStore.searchOpen" />
    </transition>

    <!-- 快捷菜单 -->
    <ma-button-menu />

    <!-- 全屏退出 -->
    <div class="max-size-exit" @click="tagExitMaxSize"><icon-close /></div>
  </a-layout-content>
</template>

<script setup>
import { onMounted, ref, watch } from 'vue'
import { useAppStore, useUserStore, useKeepAliveStore } from '@/store'

import MaOperation from '../ma-operation.vue'
import Setting from '../../setting.vue'
import SystemSearch from '../../search.vue'
import MaButtonMenu from '../ma-buttonMenu.vue'
import VideoBackground from '@/components/video-background/index.vue'

const appStore = useAppStore()
const userStore = useUserStore()
const keepStore = useKeepAliveStore()

const settingRef = ref()
const systemSearchRef = ref()

watch(() => appStore.settingOpen, vl => {
  if (vl === true) {
    settingRef.value.open()
    appStore.settingOpen = false
  }
})

const tagExitMaxSize = () => {
  document.getElementById('app').classList.remove('max-size')
}

onMounted(() => {
  document.addEventListener('keydown', e => {
    const keyCode = e.keyCode ?? e.which ?? e.charCode
    const altKey = e.altKey ?? e.metaKey
    if (altKey && keyCode === 83) {
      appStore.searchOpen = true
      return
    }

    if (keyCode === 27) {
      appStore.searchOpen = false
      return
    }
  })
})
</script>

<style scoped lang="less">
.layout-portal {
  .logo {
    width: auto;
    padding-bottom: 0;
    border-bottom: 0;
  }
}
</style>