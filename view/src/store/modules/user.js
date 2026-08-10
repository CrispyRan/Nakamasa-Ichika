import { defineStore } from 'pinia'
import loginApi from '@/api/login'
import tool from '@/utils/tool'
import router from '@/router'
import webRouter from '@/router/webRouter'
import { isUndefined } from 'lodash'
import { homePage } from '@/router/homePageRoutes'
import { useAppStore, useTagStore, useDictStore } from '@/store'
import { menuSeed } from '@/mock/index.js'

// 构建菜单树
function buildMenuTree(items, parentId = '0') {
  return items
    .filter(item => item.parent_id === parentId)
    .sort((a, b) => a.sort - b.sort)
    .map(item => ({
      ...item,
      children: buildMenuTree(items, item.id).length > 0 ? buildMenuTree(items, item.id) : undefined
    }))
}

// 根据应用类型过滤菜单（使用后端字典配置）
function filterMenuByAppType(menus, appType, dictData) {
  // 从字典获取允许的菜单列表
  const dictKey = `app_menus_${appType}`
  const allowedMenus = dictData[dictKey]?.map(item => item.value) || dictData['app_menus_user']?.map(item => item.value) || []
  
  // 如果没有配置，默认显示所有菜单
  if (allowedMenus.length === 0) {
    return menus
  }
  
  // 过滤菜单
  return menus.filter(item => {
    // 检查主菜单
    if (!allowedMenus.includes(item.name)) {
      return false
    }
    // 如果父菜单在允许列表中，保留所有子菜单
    // 这样就不需要在后端配置每个子菜单项
    // 子菜单会随父菜单自动显示
    return true
  })
}

const useUserStore = defineStore('user', {
  state: () => ({
    codes: undefined,
    roles: undefined,
    routers: undefined,
    user: undefined,
    menus: undefined,
    // 当前应用信息
    currentApp: null
  }),

  getters: {
    getState() {
      return { ...this.$state }
    },
    // 获取当前应用类型
    appType() {
      return this.currentApp?.app_type || 'user'
    }
  },

  actions: {
    setToken(token) {
      tool.session.set(import.meta.env.VITE_APP_TOKEN_PREFIX, token)
    },

    getToken() {
      return tool.session.get(import.meta.env.VITE_APP_TOKEN_PREFIX)
    },

    clearToken() {
      tool.session.remove(import.meta.env.VITE_APP_TOKEN_PREFIX)
    },

    setInfo(data) {
      this.$patch(data)
    },

    resetUserInfo() {
      this.$reset()
    },

    // 设置当前应用
    setCurrentApp(app) {
      const oldAppType = this.currentApp?.app_type
      const newAppType = app?.app_type
      
      this.currentApp = app
      if (app) {
        // tool.local.set 会自动 JSON.stringify，不要重复编码
        tool.local.set('currentApp', app)
        tool.local.set('currentAppId', app.id)
      } else {
        tool.local.remove('currentApp')
        tool.local.remove('currentAppId')
      }
      
      // 如果应用类型发生变化，刷新路由
      if (oldAppType !== newAppType && this.routers) {
        this.refreshAppRoutes()
      }
    },
    
    // 刷新应用路由（切换应用时调用）
    refreshAppRoutes() {
      if (!this.routers) return
      
      const appType = this.currentApp?.app_type || 'user'
      const dictStore = useDictStore()
      
      // 需要保留的关键路由（portal、apps、login、layout）
      const keepRoutes = ['portal', 'apps', 'login', 'layout']
      
      // 清除旧的路由（移除动态添加的路由，保留关键路由）
      const existingRoutes = router.getRoutes()
      existingRoutes.forEach(route => {
        if (route.name && !keepRoutes.includes(route.name) && !route.meta?.affix) {
          router.removeRoute(route.name)
        }
      })
      
      // 重新构建和过滤菜单
      let menus = buildMenuTree(menuSeed)
      menus = filterMenuByAppType(menus, appType, dictStore.data || {})
      
      // 更新菜单数据
      this.routers = menus
      
      // 添加新路由
      this.setMenu(this.routers, appType)
      
      // 处理菜单显示（移除按钮菜单，添加首页）
      this.routers = removeButtonMenu(this.routers)
      this.routers.unshift(homePage)
    },

    // 获取当前应用
    getCurrentApp() {
      if (!this.currentApp) {
        const appData = tool.local.get('currentApp')
        if (appData) {
          // tool.local.get 已经返回解析后的对象
          // 兼容旧数据：如果是字符串则解析，否则直接使用
          if (typeof appData === 'string') {
            try {
              this.currentApp = JSON.parse(appData)
            } catch (e) {
              this.currentApp = null
            }
          } else if (typeof appData === 'object') {
            this.currentApp = appData
          }
        }
      }
      return this.currentApp
    },

    setMenu(data, appType = 'user') {
      const routers = flatAsyncRoutes(filterAsyncRouter(data, appType))
      routers.map((item) => {
        if (isUndefined(item.meta.layout)) {
          router.addRoute('layout', item)
        } else {
          if (item.meta.layout) {
            router.addRoute('layout', item)
          } else {
            router.addRoute(item)
          }
        }
      })
    },

    requestUserInfo() {
      return new Promise((resolve, reject) => {
        loginApi.getInfo().then(async (response) => {
          if (!response || !response.data) {
            this.clearToken()
            await router.push({ name: 'login' })
            reject(false)
          } else {
            // 获取当前应用信息
            this.getCurrentApp()
            
            // 适配后端响应格式：data.info 包含用户信息
            const userData = response.data.info || response.data
            
            // 先初始化字典数据
            const dictStore = useDictStore()
            await dictStore.initData()
            
            // 构建菜单树
            let menus = buildMenuTree(menuSeed)
            
            // 根据应用类型和字典配置过滤菜单
            const appType = this.currentApp?.app_type || 'user'
            menus = filterMenuByAppType(menus, appType, dictStore.data || {})
            
            this.setInfo({
              user: {
                id: userData.id,
                username: userData.user,
                nickname: userData.notes || userData.user,
                avatar: userData.avatars || '',
                email: '',
                phone: '',
                dept_id: 1,
                dashboard: 'statistics',
                backend_setting: '{"mode":"light"}'
              },
              roles: [{ id: 1, name: '超级管理员', code: 'super_admin' }],
              codes: ['*'],
              routers: menus
            })
            homePage.children = webRouter[0].children
            this.setMenu(this.routers, appType)
            this.routers = removeButtonMenu(this.routers)
            this.routers.unshift(homePage)
            await this.setApp()
            resolve(response.data)
          }
        })
      })
    },

    login(form) {
      return loginApi
        .login(form)
        .then((r) => {
          if (r.code === 200) {
            this.setToken(r.data.token)
            return true
          } else {
            return false
          }
        })
        .catch((e) => {
          console.error('[UserStore] login error:', e)
          return false
        })
    },

    async logout() {
      // await loginApi.logout()
      const tagStore = useTagStore()
      tool.local.remove('tags')
      tagStore.clearTags()
      this.clearToken()
      this.resetUserInfo()
    },

    async setApp() {
      const appStore = useAppStore()
      const setting =
        typeof this.user.backend_setting === 'string'
          ? JSON.parse(this.user.backend_setting)
          : this.user.backend_setting
      appStore.toggleMode(setting?.mode ?? appStore.mode)
      appStore.toggleMenu(setting?.menuCollapse ?? appStore.menuCollapse)
      appStore.toggleTag(setting?.tag ?? appStore.tag)
      appStore.toggleRound(setting?.roundOpen ?? appStore.roundOpen)
      appStore.toggleWs(setting?.ws ?? appStore.ws)
      appStore.changeMenuWidth(setting?.menuWidth ?? appStore.menuWidth)
      appStore.changeLayout(setting?.layout ?? appStore.layout)
      appStore.useSkin(setting?.skin ?? appStore.skin)
      appStore.changeColor(setting?.color ?? appStore.color)
      appStore.toggleWater(setting?.waterMark ?? appStore.waterMark)
      appStore.changeWaterContent(
        setting?.waterContent ?? appStore.waterContent
      )
    }
  }
})

//路由扁平化
const flatAsyncRoutes = (routes, breadcrumb = []) => {
  let res = []
  routes.forEach((route) => {
    const tmp = { ...route }
    if (tmp.children) {
      let childrenBreadcrumb = [...breadcrumb]
      childrenBreadcrumb.push(route)
      let tmpRoute = { ...route }
      tmpRoute.meta.breadcrumb = childrenBreadcrumb
      delete tmpRoute.children
      res.push(tmpRoute)
      let childrenRoutes = flatAsyncRoutes(tmp.children, childrenBreadcrumb)
      childrenRoutes.map((item) => {
        res.push(item)
      })
    } else {
      let tmpBreadcrumb = [...breadcrumb]
      tmpBreadcrumb.push(tmp)
      tmp.meta.breadcrumb = tmpBreadcrumb
      res.push(tmp)
    }
  })
  return res
}

const views = import.meta.glob('../../views/**/**.vue')
const empty = import.meta.glob('../../layout/empty.vue')

// 菜单转换路由
// appType: 应用类型，用于动态替换组件路径
const filterAsyncRouter = (routerMap, appType = 'user') => {
  const accessedRouters = []
  routerMap.forEach((item) => {
    if (item.meta.type !== 'B') {
      if (item.meta.type === 'I') {
        item.meta.url = item.path
        item.path = `/maIframe/${item.name}`
      }

      // 根据应用类型动态替换组件路径
      let componentPath = item.component
      
      // 卡密版应用：卡密列表使用 kamiKami.vue（对接 cdkKami API）
      // 用户版应用：卡密列表使用 index.vue（对接 cdkUser API）
      if (appType === 'kami' && item.component === 'kami/index') {
        componentPath = 'kami/kamiKami'
      }

      const route = {
        path: item.path,
        name: item.name,
        hidden: item.hidden === 1,
        meta: item.meta,
        children: item.children ? filterAsyncRouter(item.children, appType) : null,
        component: views[`../../views/${componentPath}.vue`]
      }
      accessedRouters.push(route)
    }
  })
  return accessedRouters
}

// 去除按钮菜单
const removeButtonMenu = (routers) => {
  let handlerAfterRouters = []
  routers.forEach((item) => {
    if (item.meta.type !== 'B' && !item.meta.hidden) {
      let route = item
      if (item.children && item.children.length > 0) {
        route.children = removeButtonMenu(item.children)
      }
      handlerAfterRouters.push(route)
    }
  })
  return handlerAfterRouters
}
export default useUserStore
