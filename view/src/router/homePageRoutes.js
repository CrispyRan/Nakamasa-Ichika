const homePageRoutes = [
  // ========== 仪表盘 ==========
  {
    name: 'dashboard',
    path: '/dashboard',
    meta: {
      title: '仪表盘',
      icon: 'icon-dashboard',
      type: 'C',
      affix: true
    },
    component: () => import('@/views/dashboard/index.vue')
  },

  // ========== 个人中心 ==========
  {
    name: 'userCenter',
    path: '/usercenter',
    meta: { title: '个人中心', icon: 'icon-user', type: 'C' },
    component: () => import('@/views/dashboard/userCenter/index.vue')
  },

  // ========== 应用管理（有子菜单） ==========
  {
    name: 'app',
    path: '/app',
    meta: { title: '应用管理', icon: 'icon-apps', type: 'M' },
    redirect: '/app/info',
    children: [
      {
        name: 'appInfo',
        path: '/app/info',
        meta: { title: '应用信息', icon: 'mdi:information', type: 'C' },
        component: () => import('@/views/app/info.vue')
      },
      {
        name: 'appRegLogin',
        path: '/app/reglogin',
        meta: { title: '注册登录设置', icon: 'mdi:login', type: 'C' },
        component: () => import('@/views/app/reglogin.vue')
      }
    ]
  },

  // ========== 用户管理 ==========
  {
    name: 'userList',
    path: '/user/list',
    meta: { title: '用户管理', icon: 'icon-user-group', type: 'C' },
    component: () => import('@/views/user/index.vue')
  },
  {
    name: 'UserEdit',
    path: '/user/edit/:uid',
    meta: { title: '编辑用户', icon: 'icon-edit', type: 'C', hidden: true },
    component: () => import('@/views/user/edit.vue')
  },

  // ========== 卡密管理（有子菜单，保留折叠） ==========
  {
    name: 'kami',
    path: '/kami',
    meta: { title: '卡密管理', icon: 'icon-code-square', type: 'M' },
    redirect: '/kami/list',
    children: [
      {
        name: 'kamiList',
        path: '/kami/list',
        meta: { title: '卡密列表', icon: 'icon-list', type: 'C' },
        component: () => import('@/views/kami/index.vue')
      },
      {
        name: 'kamiGroup',
        path: '/kami/group',
        meta: { title: '卡密分组', icon: 'icon-folder', type: 'C' },
        component: () => import('@/views/kami/group.vue')
      }
    ]
  },

  // ========== 版本管理 ==========
  {
    name: 'verList',
    path: '/ver/list',
    meta: { title: '版本管理', icon: 'mdi:package-variant', type: 'C' },
    component: () => import('@/views/ver/index.vue')
  },

  // ========== 代理管理 ==========
  {
    name: 'agentList',
    path: '/agent/list',
    meta: { title: '代理管理', icon: 'mdi:account-supervisor', type: 'C' },
    component: () => import('@/views/agent/index.vue')
  },

  // ========== 订单管理 ==========
  {
    name: 'orderList',
    path: '/order/list',
    meta: { title: '订单管理', icon: 'icon-file', type: 'C' },
    component: () => import('@/views/order/index.vue')
  },

  // ========== 支付配置 ==========
  {
    name: 'payConfig',
    path: '/pay/config',
    meta: { title: '支付配置', icon: 'mdi:credit-card-outline', type: 'C' },
    component: () => import('@/views/pay/index.vue')
  },

  // ========== 云函数 ==========
  {
    name: 'functionList',
    path: '/function/list',
    meta: { title: '云函数', icon: 'icon-code', type: 'C' },
    component: () => import('@/views/function/index.vue')
  },

  // ========== 加密方案 ==========
  {
    name: 'encryptionList',
    path: '/encryption/list',
    meta: { title: '加密方案', icon: 'icon-lock', type: 'C' },
    component: () => import('@/views/encryption/index.vue')
  },

  // ========== 黑名单 ==========
  {
    name: 'blocklistList',
    path: '/blocklist/list',
    meta: { title: '黑名单', icon: 'icon-close-circle', type: 'C' },
    component: () => import('@/views/blocklist/index.vue')
  },

  // ========== 日志管理 ==========
  {
    name: 'logsList',
    path: '/logs/list',
    meta: { title: '日志管理', icon: 'mdi:file-document-outline', type: 'C' },
    component: () => import('@/views/logs/index.vue')
  },

  // ========== 公告管理 ==========
  {
    name: 'noticeList',
    path: '/notice/list',
    meta: { title: '公告管理', icon: 'icon-notification', type: 'C' },
    component: () => import('@/views/notice/index.vue')
  },

  // ========== 留言管理 ==========
  {
    name: 'messageList',
    path: '/message/list',
    meta: { title: '留言管理', icon: 'icon-message', type: 'C' },
    component: () => import('@/views/message/index.vue')
  },

  // ========== 发信控制 ==========
  {
    name: 'sendControl',
    path: '/message/send',
    meta: { title: '发信控制', icon: 'icon-email', type: 'C' },
    component: () => import('@/views/message/send.vue')
  },

  // ========== 积分管理 ==========
  {
    name: 'fenIndex',
    path: '/fen/index',
    meta: { title: '积分管理', icon: 'icon-star', type: 'C' },
    component: () => import('@/views/fen/index.vue')
  },

  // ========== 商品管理 ==========
  {
    name: 'goodsList',
    path: '/goods/list',
    meta: { title: '商品管理', icon: 'mdi:cart-outline', type: 'C' },
    component: () => import('@/views/goods/index.vue')
  },

  // ========== 扩展字段 ==========
  {
    name: 'extendList',
    path: '/extend/list',
    meta: { title: '扩展字段', icon: 'mdi:form-textbox', type: 'C' },
    component: () => import('@/views/extend/index.vue')
  },

  // ========== 系统设置（有子菜单，保留折叠） ==========
  {
    name: 'system',
    path: '/system',
    meta: { title: '系统设置', icon: 'icon-settings', type: 'M' },
    redirect: '/system/set',
    children: [
      {
        name: 'systemSet',
        path: '/system/set',
        meta: { title: '基础设置', icon: 'icon-settings', type: 'C' },
        component: () => import('@/views/system/set.vue')
      },
      {
        name: 'systemRouter',
        path: '/system/router',
        meta: { title: 'API路由', icon: 'icon-route', type: 'C' },
        component: () => import('@/views/system/router.vue')
      },
      {
        name: 'systemCode',
        path: '/system/code',
        meta: { title: 'API代码', icon: 'icon-code', type: 'C' },
        component: () => import('@/views/system/code.vue')
      }
    ]
  },

  // ========== 管理员 ==========
  {
    name: 'adminList',
    path: '/adminMgmt/list',
    meta: { title: '管理员', icon: 'mdi:shield-account', type: 'C' },
    component: () => import('@/views/admin/index.vue')
  },

  // ========== 数据统计（有子菜单，保留折叠） ==========
  {
    name: 'statistics',
    path: '/statistics',
    meta: { title: '数据统计', icon: 'icon-bar-chart', type: 'M' },
    redirect: '/statistics/index',
    children: [
      {
        name: 'statisticsIndex',
        path: '/statistics/index',
        meta: { title: '统计概览', icon: 'icon-dashboard', type: 'C' },
        component: () => import('@/views/statistics/index.vue')
      },
      {
        name: 'dataAnalysis',
        path: '/statistics/data-analysis',
        meta: { title: '分析页', icon: 'icon-line-chart', type: 'C' },
        component: () => import('@/views/visualization/data-analysis/index.vue')
      },
      {
        name: 'multiDimensionDataAnalysis',
        path: '/statistics/multi-dimension',
        meta: { title: '多维数据分析', icon: 'icon-apps', type: 'C' },
        component: () => import('@/views/visualization/multi-dimension-data-analysis/index.vue')
      }
    ]
  },

  // ========== 插件市场 ==========
  {
    name: 'appStore',
    path: 'https://saas.saithink.top/#/appStore',
    meta: {
      title: '插件市场',
      icon: 'icon-apps',
      type: 'L'
    }
  }
]

export const homePage = {
  name: 'home',
  path: '/home',
  meta: { title: '首页', icon: 'icon-home', hidden: false, type: 'M' }
}

export default homePageRoutes
