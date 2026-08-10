import homePageRoutes from './homePageRoutes'
//系统路由
const routes = [
  {
    name: 'portal',
    path: '/',
    component: () => import('@/layout/components/portal/index.vue'),
    redirect: 'apps',
    children: [
      {
        name: 'apps',
        path: '/apps',
        meta: {
          title: '应用中心',
          icon: 'icon-apps',
          type: 'M'
        },
        component: () => import('@/views/app-list/index.vue')
      }
    ]
  },
  {
    name: 'layout',
    path: '/admin',
    component: () => import('@/layout/index.vue'),
    redirect: 'dashboard',
    children: homePageRoutes
  },
  {
    name: 'login',
    path: '/login',
    component: () => import('@/views/login.vue'),
    meta: { title: '登录' }
  },
  {
    name: 'install',
    path: '/install',
    component: () => import('@/views/install/index.vue'),
    meta: { title: '系统安装' }
  },
  {
    path: '/:pathMatch(.*)*',
    hidden: true,
    meta: { title: '访问的页面不存在' },
    component: () => import('@/layout/404.vue')
  }
]

export default routes
