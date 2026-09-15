import { createRouter, createWebHashHistory, createWebHistory } from 'vue-router'
import { useUserStore } from '@/store'
import NProgress from 'nprogress'
import tool from '@/utils/tool'
import 'nprogress/nprogress.css'
import { request } from '@/utils/request.js'

import routes from './webRouter.js'

const title = import.meta.env.VITE_APP_TITLE
const defaultRoutePath = '/'
// 白名单路由：不需要登录即可访问。
// 注意不要把业务页（apps/portal）放进去：它们强依赖登录态与 token（appApi.getList 等
// 都走 AdminAuth），无 token 访问只会渲染出空白页。
// 之前 apps、portal 在白名单里，导致「关标签页后 sessionStorage 清空 token」再重开首页时
// 守卫直接 next() 放行、不跳登录，用户看到的是空白页而不是登录页。
const whiteRoute = ['login', 'install']

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
  // 登录态用 localStorage（而非 sessionStorage），这样关掉标签页/重启浏览器后
  // 不用重新登录。守卫每次进入受控页都会用 token 调 /admin/admin/verify 重新校验，
  // 失效会 clearToken 并跳登录，所以持久化不会带来安全隐患。
  const token = tool.local.get(import.meta.env.VITE_APP_TOKEN_PREFIX)

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
      let data = null
      try {
        data = await userStore.requestUserInfo()
      } catch (e) {
        // requestUserInfo 会 reject：后端无 data（含接口 500 被拦截器 resolve 成
        // 无 data 的对象）走 reject(false)，或处理链路上抛错走 reject(e)。
        // 不 catch 的话 await 抛错会让下面的 next() 永不执行，
        // Vue Router 导航因此永久挂起、NProgress 一直转圈 —— 这是必现的前端卡死
        console.warn('[Router] 获取用户信息失败:', e)
      }
      if (data) {
        const safeQuery = to.query ? { ...to.query } : {}
        delete safeQuery.redirect
        next({ path: to.path, query: safeQuery })
      } else {
        // 信息获取失败：中止本次导航（requestUserInfo 内部已 push 到 login）
        // 仍必须调用 next()，否则导航挂起
        next(false)
      }
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