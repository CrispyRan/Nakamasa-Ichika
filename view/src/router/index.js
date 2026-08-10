import { createRouter, createWebHashHistory, createWebHistory } from 'vue-router'
import { useUserStore } from '@/store'
import NProgress from 'nprogress'
import tool from '@/utils/tool'
import 'nprogress/nprogress.css'
import { request } from '@/utils/request.js'

import routes from './webRouter.js'

const title = import.meta.env.VITE_APP_TITLE
const defaultRoutePath = '/'
// 白名单路由：不需要登录即可访问
const whiteRoute = ['login', 'install', 'apps', 'portal']

// 安装状态缓存：null=未检查, true=已安装, false=未安装
let installChecked = null

/**
 * 检查系统是否已安装
 * @returns {Promise<boolean>}
 */
async function checkInstalled() {
  // 已检查过，返回缓存结果
  if (installChecked !== null) {
    return installChecked
  }
  
  try {
    const res = await request({
      url: '/install/check',
      method: 'get',
      timeout: 5000
    })
    // code === 200 表示已安装
    installChecked = res.code === 200
    return installChecked
  } catch (error) {
    console.error('安装检查失败:', error)
    // 检查失败时假设已安装，避免阻塞正常访问
    installChecked = true
    return true
  }
}

const router = createRouter({
  history: createWebHashHistory(),
  routes
})

router.beforeEach(async (to, from, next) => {
  NProgress.start()
  const userStore = useUserStore()
  let toTitle = to.meta.title ? to.meta.title : to.name
  document.title = `${toTitle} - ${title}`
  const token = tool.session.get(import.meta.env.VITE_APP_TOKEN_PREFIX)

  // 安装页面直接放行
  if (to.name === 'install') {
    next()
    return
  }

  // 检查是否已安装（访问 admin 相关路由时）
  const isAdminRoute = to.path.startsWith('/admin') || to.path.startsWith('/dashboard') || to.path.startsWith('/usercenter')
  if (isAdminRoute || to.name === 'login') {
    const installed = await checkInstalled()
    if (!installed) {
      // 未安装，跳转到安装页面
      next({ name: 'install' })
      return
    }
  }
  
  // 登录状态下
  if (token) {
    if (to.name === 'login') {
      next({ path: defaultRoutePath })
      return
    }

    if (! userStore.user && userStore.user == undefined ) {
      const data = await userStore.requestUserInfo()
      const safeQuery = to.query ? { ...to.query } : {}
      delete safeQuery.redirect
      data && next({ path: to.path, query: safeQuery })
    } else {
      next()
    }
  } else {
    // 未登录的情况下允许访问的路由
    if (! whiteRoute.includes(to.name)) {
      next({ name: 'login', query: { redirect: to.fullPath } })
    } else {
      next()
    }
  }
})

router.afterEach((to, from) => {
  NProgress.done()
})

router.onError(error => {
  NProgress.done();
});


export default router