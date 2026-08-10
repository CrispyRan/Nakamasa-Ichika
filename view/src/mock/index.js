/**
 * Mock 数据配置
 */

import { ok, fail, page, forbidden, store, generateId } from '@/utils/mock'

// ========== 菜单配置 ==========
const menuSeed = [
  // 仪表盘
  { id: '1', parent_id: '0', name: 'dashboard', path: '/dashboard', component: 'dashboard/index', icon: 'icon-dashboard', type: 'C', sort: 1, status: 1, meta: { title: '仪表盘', icon: 'icon-dashboard', type: 'C' } },
  
  // 应用管理 - 有子菜单
  { id: '2', parent_id: '0', name: 'app', path: '/app', icon: 'icon-apps', type: 'M', sort: 10, status: 1, meta: { title: '应用管理', icon: 'icon-apps', type: 'M' } },
  { id: '2-1', parent_id: '2', name: 'appInfo', path: '/app/info', component: 'app/info', icon: 'mdi:information', type: 'C', sort: 1, status: 1, meta: { title: '应用信息', icon: 'mdi:information', type: 'C' } },
  { id: '2-2', parent_id: '2', name: 'appRegLogin', path: '/app/reglogin', component: 'app/reglogin', icon: 'mdi:login', type: 'C', sort: 2, status: 1, meta: { title: '注册登录设置', icon: 'mdi:login', type: 'C' } },
  
  // 用户管理 - 直接显示
  { id: '3', parent_id: '0', name: 'userList', path: '/user/list', component: 'user/index', icon: 'icon-user-group', type: 'C', sort: 20, status: 1, meta: { title: '用户管理', icon: 'icon-user-group', type: 'C' } },
  
  // 卡密管理 - 有两个子菜单，保留折叠
  { id: '4', parent_id: '0', name: 'kami', path: '/kami', icon: 'icon-code-square', type: 'M', sort: 30, status: 1, meta: { title: '卡密管理', icon: 'icon-code-square', type: 'M' } },
  { id: '4-1', parent_id: '4', name: 'kamiList', path: '/kami/list', component: 'kami/index', icon: 'icon-list', type: 'C', sort: 1, status: 1, meta: { title: '卡密列表', icon: 'icon-list', type: 'C' } },
  { id: '4-2', parent_id: '4', name: 'kamiGroup', path: '/kami/group', component: 'kami/group', icon: 'icon-folder', type: 'C', sort: 2, status: 1, meta: { title: '卡密分组', icon: 'icon-folder', type: 'C' } },
  
  // 版本管理 - 直接显示
  { id: '5', parent_id: '0', name: 'verList', path: '/ver/list', component: 'ver/index', icon: 'mdi:package-variant', type: 'C', sort: 40, status: 1, meta: { title: '版本管理', icon: 'mdi:package-variant', type: 'C' } },
  
  // 代理管理 - 直接显示
  { id: '6', parent_id: '0', name: 'agentList', path: '/agent/list', component: 'agent/index', icon: 'mdi:account-supervisor', type: 'C', sort: 50, status: 1, meta: { title: '代理管理', icon: 'mdi:account-supervisor', type: 'C' } },
  
  // 订单管理 - 直接显示
  { id: '7', parent_id: '0', name: 'orderList', path: '/order/list', component: 'order/index', icon: 'icon-file', type: 'C', sort: 60, status: 1, meta: { title: '订单管理', icon: 'icon-file', type: 'C' } },
  
  // 支付配置 - 直接显示
  { id: '8', parent_id: '0', name: 'payConfig', path: '/pay/config', component: 'pay/index', icon: 'mdi:credit-card-outline', type: 'C', sort: 70, status: 1, meta: { title: '支付配置', icon: 'mdi:credit-card-outline', type: 'C' } },
  
  // 云函数 - 直接显示
  { id: '9', parent_id: '0', name: 'functionList', path: '/function/list', component: 'function/index', icon: 'icon-code', type: 'C', sort: 80, status: 1, meta: { title: '云函数', icon: 'icon-code', type: 'C' } },
  
  // 加密方案 - 直接显示
  { id: '10', parent_id: '0', name: 'encryptionList', path: '/encryption/list', component: 'encryption/index', icon: 'icon-lock', type: 'C', sort: 90, status: 1, meta: { title: '加密方案', icon: 'icon-lock', type: 'C' } },
  
  // 黑名单 - 直接显示
  { id: '11', parent_id: '0', name: 'blocklistList', path: '/blocklist/list', component: 'blocklist/index', icon: 'icon-close-circle', type: 'C', sort: 100, status: 1, meta: { title: '黑名单', icon: 'icon-close-circle', type: 'C' } },
  
  // 日志管理 - 直接显示
  { id: '12', parent_id: '0', name: 'logsList', path: '/logs/list', component: 'logs/index', icon: 'mdi:file-document-outline', type: 'C', sort: 110, status: 1, meta: { title: '日志管理', icon: 'mdi:file-document-outline', type: 'C' } },
  
  // 公告管理 - 直接显示
  { id: '13', parent_id: '0', name: 'noticeList', path: '/notice/list', component: 'notice/index', icon: 'icon-notification', type: 'C', sort: 120, status: 1, meta: { title: '公告管理', icon: 'icon-notification', type: 'C' } },
  
  // 留言管理 - 直接显示
  { id: '14', parent_id: '0', name: 'messageList', path: '/message/list', component: 'message/index', icon: 'icon-message', type: 'C', sort: 130, status: 1, meta: { title: '留言管理', icon: 'icon-message', type: 'C' } },
  
  // 发信控制 - 直接显示
  { id: '14-1', parent_id: '0', name: 'sendControl', path: '/message/send', component: 'message/send', icon: 'icon-email', type: 'C', sort: 135, status: 1, meta: { title: '发信控制', icon: 'icon-email', type: 'C' } },
  
  // 积分管理 - 直接显示
  { id: '15', parent_id: '0', name: 'fenIndex', path: '/fen/index', component: 'fen/index', icon: 'icon-star', type: 'C', sort: 140, status: 1, meta: { title: '积分管理', icon: 'icon-star', type: 'C' } },
  
  // 商品管理 - 直接显示
  { id: '16', parent_id: '0', name: 'goodsList', path: '/goods/list', component: 'goods/index', icon: 'mdi:cart-outline', type: 'C', sort: 150, status: 1, meta: { title: '商品管理', icon: 'mdi:cart-outline', type: 'C' } },
  
  // 扩展字段 - 直接显示
  { id: '17', parent_id: '0', name: 'extendList', path: '/extend/list', component: 'extend/index', icon: 'mdi:form-textbox', type: 'C', sort: 160, status: 1, meta: { title: '扩展字段', icon: 'mdi:form-textbox', type: 'C' } },
  
  // 系统设置 - 有多个子菜单，保留折叠
  { id: '18', parent_id: '0', name: 'system', path: '/system', icon: 'icon-settings', type: 'M', sort: 170, status: 1, meta: { title: '系统设置', icon: 'icon-settings', type: 'M' } },
  { id: '18-1', parent_id: '18', name: 'systemSet', path: '/system/set', component: 'system/set', icon: 'icon-settings', type: 'C', sort: 1, status: 1, meta: { title: '基础设置', icon: 'icon-settings', type: 'C' } },
  { id: '18-2', parent_id: '18', name: 'systemRouter', path: '/system/router', component: 'system/router', icon: 'icon-route', type: 'C', sort: 2, status: 1, meta: { title: 'API路由', icon: 'icon-route', type: 'C' } },
  { id: '18-3', parent_id: '18', name: 'systemCode', path: '/system/code', component: 'system/code', icon: 'icon-code', type: 'C', sort: 3, status: 1, meta: { title: 'API代码', icon: 'icon-code', type: 'C' } },
  
  // 管理员 - 直接显示
  { id: '19', parent_id: '0', name: 'adminList', path: '/adminMgmt/list', component: 'admin/index', icon: 'mdi:shield-account', type: 'C', sort: 180, status: 1, meta: { title: '管理员', icon: 'mdi:shield-account', type: 'C' } },
  
  // 数据统计 - 有多个子菜单，保留折叠
  { id: '20', parent_id: '0', name: 'statistics', path: '/statistics', icon: 'icon-bar-chart', type: 'M', sort: 190, status: 1, meta: { title: '数据统计', icon: 'icon-bar-chart', type: 'M' } },
  { id: '20-1', parent_id: '20', name: 'statisticsIndex', path: '/statistics/index', component: 'statistics/index', icon: 'icon-dashboard', type: 'C', sort: 1, status: 1, meta: { title: '统计概览', icon: 'icon-dashboard', type: 'C' } },
  { id: '20-2', parent_id: '20', name: 'dataAnalysis', path: '/statistics/data-analysis', component: 'visualization/data-analysis/index', icon: 'icon-line-chart', type: 'C', sort: 2, status: 1, meta: { title: '分析页', icon: 'icon-line-chart', type: 'C' } },
  { id: '20-3', parent_id: '20', name: 'multiDimensionDataAnalysis', path: '/statistics/multi-dimension', component: 'visualization/multi-dimension-data-analysis/index', icon: 'icon-apps', type: 'C', sort: 3, status: 1, meta: { title: '多维数据分析', icon: 'icon-apps', type: 'C' } },
  
  // 插件市场 - 外部链接
  { id: '21', parent_id: '0', name: 'appStore', path: 'https://saas.saithink.top/#/appStore', icon: 'icon-apps', type: 'L', sort: 200, status: 1, meta: { title: '插件市场', icon: 'icon-apps', type: 'L' } },
]

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

// ========== 种子数据 ==========
const userSeed = [
  { id: '1', username: 'admin', nickname: '超级管理员', email: 'admin@example.com', phone: '13800138000', dept_id: '1', role_ids: ['1'], status: 1, create_time: '2024-01-01 00:00:00' },
  { id: '2', username: 'user', nickname: '普通用户', email: 'user@example.com', phone: '13800138001', dept_id: '2', role_ids: ['2'], status: 1, create_time: '2024-01-02 00:00:00' },
]

const deptSeed = [
  { id: '1', parent_id: '0', name: '总公司', leader: 'admin', phone: '13800138000', sort: 1, status: 1 },
  { id: '2', parent_id: '1', name: '技术部', leader: 'user', phone: '13800138001', sort: 1, status: 1 },
]

const roleSeed = [
  { id: '1', name: '超级管理员', code: 'super_admin', sort: 1, status: 1, remark: '拥有所有权限' },
  { id: '2', name: '普通用户', code: 'user', sort: 2, status: 1, remark: '普通用户权限' },
]

const loginLogSeed = [
  { id: '1', username: 'admin', ip: '127.0.0.1', browser: 'Chrome', os: 'Windows', status: 1, message: '登录成功', create_time: '2024-03-20 10:00:00' },
]

const dictData = {
  data_status: [{ label: '启用', value: 1 }, { label: '禁用', value: 0 }],
  sex: [{ label: '男', value: 1 }, { label: '女', value: 2 }],
}

// ========== 业务种子数据 ==========
const appSeed = [
  { id: 'app001', name: '用户管理系统', type: 'web', description: '用户注册、登录、权限管理的后台系统', status: 1, version: '1.0.0' },
  { id: 'app002', name: '数据分析平台', type: 'api', description: '提供数据统计、可视化分析功能', status: 1, version: '2.1.0' },
  { id: 'app003', name: '移动端商城', type: 'miniapp', description: '微信小程序电商商城', status: 0, version: '1.5.0' },
]

const bizUserSeed = [
  { id: 10001, acctno: 'zhangsan', nickname: '张三', email: 'zhangsan@example.com', phone: '13800138001', avatars: '', open_wx: 'wx_123', open_qq: null, vip: 1747449600, ug: 2, fen: 1580, ban: 0, ban_msg: null, sn_list: [{ udid: 'DEV-001-ABCD', time: 1710915600 }], sn_max: 0, reg_sn: 'DEV-REG-001', reg_time: 1704067200, reg_ip: '192.168.1.100', last_login_info: { time: 1713244800, ip: '192.168.1.200', device: 'Chrome/Windows' }, online: 1, inviter_id: 0, extend: { real_name: '张三丰', gender: '男' } },
  { id: 10002, acctno: 'lisi', nickname: '李四', email: 'lisi@example.com', phone: '13800138002', avatars: '', open_wx: null, open_qq: 'qq_456', vip: 9999999999, ug: 3, fen: 3200, ban: 0, ban_msg: null, sn_list: [{ udid: 'DEV-002-EFGH', time: 1710829200 }, { udid: 'DEV-002-IJKL', time: 1710915600 }], sn_max: 1, reg_sn: 'DEV-REG-002', reg_time: 1704153600, reg_ip: '192.168.1.101', last_login_info: { time: 1713158400, ip: '192.168.1.201', device: 'Safari/macOS' }, online: 0, inviter_id: 10001, extend: null },
  { id: 10003, acctno: 'wangwu', nickname: '王五', email: 'wangwu@example.com', phone: '13800138003', avatars: '', open_wx: null, open_qq: null, vip: 0, ug: 1, fen: 50, ban: 1715923200, ban_msg: '违反用户使用协议', sn_list: [], sn_max: 0, reg_sn: '', reg_time: 1704240000, reg_ip: '192.168.1.102', last_login_info: null, online: 0, inviter_id: 0, extend: null },
  { id: 10004, acctno: 'zhaoliu', nickname: '赵六', email: 'zhaoliu@example.com', phone: '13800138004', avatars: '', open_wx: 'wx_789', open_qq: 'qq_012', vip: 1718774400, ug: 2, fen: 890, ban: 0, ban_msg: null, sn_list: [{ udid: 'DEV-004-MNOP', time: 1710742800 }], sn_max: 0, reg_sn: 'DEV-REG-004', reg_time: 1704326400, reg_ip: '192.168.1.103', last_login_info: { time: 1713331200, ip: '192.168.1.202', device: 'Mobile/Android' }, online: 1, inviter_id: 10002, extend: { company: '测试公司' } },
  { id: 10005, acctno: 'sunqi', nickname: '孙七', email: 'sunqi@example.com', phone: '13800138005', avatars: '', open_wx: null, open_qq: null, vip: 0, ug: 1, fen: 120, ban: 0, ban_msg: null, sn_list: [], sn_max: 0, reg_sn: '', reg_time: 1704412800, reg_ip: '192.168.1.104', last_login_info: { time: 1713072000, ip: '192.168.1.203', device: 'Firefox/Linux' }, online: 0, inviter_id: 0, extend: null },
  { id: 10006, acctno: 'zhouba', nickname: '周八', email: 'zhouba@example.com', phone: '13800138006', avatars: '', open_wx: 'wx_345', open_qq: null, vip: 1752364800, ug: 2, fen: 2100, ban: 0, ban_msg: null, sn_list: [{ udid: 'DEV-006-QRST', time: 1710656400 }], sn_max: 2, reg_sn: 'DEV-REG-006', reg_time: 1704499200, reg_ip: '192.168.1.105', last_login_info: { time: 1713504000, ip: '192.168.1.204', device: 'Edge/Windows' }, online: 0, inviter_id: 10001, extend: null },
  { id: 10007, acctno: 'wujiu', nickname: '吴九', email: 'wujiu@example.com', phone: '13800138007', avatars: '', open_wx: null, open_qq: 'qq_678', vip: 9999999999, ug: 3, fen: 5000, ban: 0, ban_msg: null, sn_list: [{ udid: 'DEV-007-UVWX', time: 1710570000 }, { udid: 'DEV-007-YZ01', time: 1710656400 }], sn_max: 1, reg_sn: 'DEV-REG-007', reg_time: 1704585600, reg_ip: '192.168.1.106', last_login_info: { time: 1713417600, ip: '192.168.1.205', device: 'Chrome/macOS' }, online: 1, inviter_id: 10003, extend: { vip_source: '活动赠送' } },
  { id: 10008, acctno: 'zhengshi', nickname: '郑十', email: 'zhengshi@example.com', phone: '13800138008', avatars: '', open_wx: null, open_qq: null, vip: 0, ug: 1, fen: 80, ban: 0, ban_msg: null, sn_list: [], sn_max: 0, reg_sn: '', reg_time: 1704672000, reg_ip: '192.168.1.107', last_login_info: null, online: 0, inviter_id: 0, extend: null },
]

const cdkGroupSeed = [
  { id: 1, name: '月卡', type: 'vip', val: 2592000, price: 30.00, appid: 1000 },
  { id: 2, name: '季卡', type: 'vip', val: 7776000, price: 80.00, appid: 1000 },
  { id: 3, name: '年卡', type: 'vip', val: 31536000, price: 280.00, appid: 1000 },
]

const cdkKamiSeed = [
  { id: 'ck001', code: 'ABCD-1234-EFGH-5678', group_id: 'cg001', status: 0, used_by: null, used_at: null },
  { id: 'ck002', code: 'IJKL-9012-MNOP-3456', group_id: 'cg002', status: 1, used_by: 'u001', used_at: '2024-03-15 10:30:00' },
  { id: 'ck003', code: 'QRST-7890-UVWX-1234', group_id: 'cg001', status: 0, used_by: null, used_at: null },
]

const agentSeed = [
  { id: 'a001', username: 'agent001', nickname: '金牌代理', phone: '13900139001', level: 1, balance: 1580.00, commission_rate: 0.15, status: 1 },
  { id: 'a002', username: 'agent002', nickname: '银牌代理', phone: '13900139002', level: 2, balance: 890.00, commission_rate: 0.10, status: 1 },
]

const orderSeed = [
  { id: 'o001', order_no: 'ORD202403200001', user_id: 'u001', amount: 30.00, pay_type: 'alipay', status: 1, create_time: '2024-03-20 10:00:00' },
  { id: 'o002', order_no: 'ORD202403200002', user_id: 'u002', amount: 80.00, pay_type: 'wechat', status: 1, create_time: '2024-03-20 11:30:00' },
  { id: 'o003', order_no: 'ORD202403200003', user_id: 'u001', amount: 280.00, pay_type: 'alipay', status: 0, create_time: '2024-03-20 14:00:00' },
]

const encryptionSeed = [
  { id: 'e001', name: 'RC4加密', type: 'rc4', key: '******', status: 1 },
  { id: 'e002', name: 'AES-256加密', type: 'aes', key: '******', status: 1 },
]

const functionSeed = [
  { id: 'f001', name: '用户登录回调', type: 'login', code: 'function onLogin(user) { return user; }', status: 1 },
  { id: 'f002', name: '订单支付回调', type: 'payment', code: 'function onPayment(order) { return order; }', status: 1 },
]

const blocklistSeed = [
  { id: 'b001', type: 'ip', value: '192.168.1.100', reason: '恶意请求', create_time: '2024-03-19 10:00:00' },
  { id: 'b002', type: 'device', value: 'DEVICE-XXXX-YYYY', reason: '作弊行为', create_time: '2024-03-19 14:30:00' },
]

const goodsSeed = [
  { id: 'g001', name: '月卡会员', price: 30.00, days: 30, description: '30天会员权益', status: 1 },
  { id: 'g002', name: '季卡会员', price: 80.00, days: 90, description: '90天会员权益', status: 1 },
  { id: 'g003', name: '年卡会员', price: 280.00, days: 365, description: '365天会员权益', status: 1 },
]

const versionSeed = [
  { id: 'v001', app_id: 'app001', version: '1.0.0', version_code: 100, update_content: '初始版本', force_update: 0, status: 1 },
  { id: 'v002', app_id: 'app001', version: '1.1.0', version_code: 110, update_content: '新增功能', force_update: 0, status: 1 },
  { id: 'v003', app_id: 'app002', version: '2.0.0', version_code: 200, update_content: '重大更新', force_update: 1, status: 1 },
]

// ========== 初始化数据 ==========
const MOCK_DATA_VERSION = '1.1.0'  // 数据版本号，修改结构时需更新

export function initMockData() {
  // 检查数据版本，版本不匹配时清理旧数据
  const savedVersion = localStorage.getItem('__mock_version__')
  if (savedVersion !== MOCK_DATA_VERSION) {
    console.log('[Mock] 检测到数据版本变化，清理旧数据...')
    // 清理所有 mock 数据
    Object.keys(localStorage)
      .filter(k => k.startsWith('__mock_data__'))
      .forEach(k => localStorage.removeItem(k))
    localStorage.setItem('__mock_version__', MOCK_DATA_VERSION)
  }
  
  // 系统管理数据
  store.seed('users', userSeed)
  store.seed('depts', deptSeed)
  store.seed('roles', roleSeed)
  store.seed('menus', menuSeed)
  store.seed('loginLogs', loginLogSeed)
  
  // 业务数据
  store.seed('apps', appSeed)
  store.seed('bizUsers', bizUserSeed)
  store.seed('cdkGroups', cdkGroupSeed)
  store.seed('cdkKamis', cdkKamiSeed)
  store.seed('agents', agentSeed)
  store.seed('orders', orderSeed)
  store.seed('encryptions', encryptionSeed)
  store.seed('functions', functionSeed)
  store.seed('blocklist', blocklistSeed)
  store.seed('goods', goodsSeed)
  store.seed('versions', versionSeed)
  
  // 新增模块数据
  store.seed('userLogs', [
    { id: 'l001', type: 'login', username: 'zhangsan', ip: '192.168.1.100', content: '用户登录', status: 1, create_time: '2024-03-20 10:00:00' },
    { id: 'l002', type: 'recharge', username: 'zhangsan', ip: '192.168.1.100', content: '充值VIP月卡', status: 1, create_time: '2024-03-20 10:05:00' }
  ])
  store.seed('adminLogs', [
    { id: 'al001', type: 'login', username: 'admin', ip: '127.0.0.1', content: '管理员登录', status: 1, create_time: '2024-03-20 09:00:00' }
  ])
  store.seed('notices', [
    { id: 'n001', title: '系统上线公告', type: 1, content: '欢迎使用U验证系统', status: 1, create_time: '2024-01-01' },
    { id: 'n002', title: '维护通知', type: 2, content: '系统将于3月25日进行维护', status: 1, create_time: '2024-03-20' }
  ])
  store.seed('messages', [
    { id: 'm001', username: 'zhangsan', avatar: '', content: '请问如何充值VIP会员？', status: 1, reply: '您好！请在商品页面选择对应的会员套餐进行购买即可。', reply_time: '2024-03-20 10:30:00', images: [], create_time: '2024-03-20 10:00:00' },
    { id: 'm002', username: 'lisi', avatar: '', content: '我购买后显示激活失败怎么办？', status: 0, images: ['https://picsum.photos/200/150?random=1'], create_time: '2024-03-21 14:20:00' },
    { id: 'm003', username: 'wangwu', avatar: '', content: '能不能增加微信支付？', status: 2, reply: '感谢反馈，我们正在规划中。', reply_time: '2024-03-19 16:00:00', images: [], create_time: '2024-03-19 15:30:00' }
  ])
  store.seed('fenEvents', [
    { id: 'fe001', name: '签到奖励', type: 1, amount: 10, description: '每日签到获得10积分', status: 1 },
    { id: 'fe002', name: '注册奖励', type: 1, amount: 50, description: '新用户注册获得50积分', status: 1 }
  ])
  store.seed('fenOrders', [
    { id: 'fo001', username: 'zhangsan', type: 1, amount: 10, event_name: '签到奖励', balance: 100, create_time: '2024-03-20' }
  ])
  store.seed('extends', [
    { id: 'ex001', name: '真实姓名', field: 'real_name', type: 'text', required: false },
    { id: 'ex002', name: '性别', field: 'gender', type: 'select', options: '男\n女', required: false }
  ])
  store.seed('adminList', [
    { id: 'ad001', username: 'admin', nickname: '超级管理员', phone: '13800000000', email: 'admin@test.com', status: 1, role_name: '超级管理员', last_login: '2024-03-20 09:00:00' }
  ])
  
  console.log('[Mock] 数据初始化完成')
}

// ========== 从 URL 提取参数 ==========
function getUrlParam(url, param) {
  const match = url.match(new RegExp(`[?&]${param}=([^&]*)`))
  return match ? match[1] : null
}

// ========== Mock 配置列表 ==========
export const allMocks = [
  // ===== 登录 =====
  {
    url: '/admin/login',
    method: 'post',
    handler: () => ok({ 
      token: 'mock_token_' + Date.now(), 
      info: { id: 1, user: 'admin', notes: '超级管理员', avatars: '', lockin: false, auth: ['all'], state: 'on', appid: null },
      exp: Math.floor(Date.now() / 1000) + 259200
    })
  },
  {
    url: '/admin/login',
    method: 'post',
    handler: () => ok({ token: 'mock_token_' + Date.now(), expire: 7200 })
  },
  {
    url: '/admin/captcha',
    method: 'get',
    handler: () => ok({ captchaId: 'mock', captchaImage: '' })
  },
  {
    url: '/admin/logout',
    method: 'post',
    handler: () => ok({ success: true })
  },

  // ===== 用户信息 (Token验证) =====
  {
    url: '/admin/admin/verify',
    method: 'get',
    handler: () => ok({
      info: { id: 1, user: 'admin', password: '', notes: '超级管理员', avatars: '', lockin: false, auth: ['all'], state: 'on', appid: null }
    })
  },
  {
    url: '/admin/system/user',
    method: 'get',
    handler: () => ok({
      user: { id: 1, username: 'admin', nickname: '超级管理员', avatar: '', email: 'admin@test.com', phone: '13800000000', dept_id: 1, dashboard: 'statistics', backend_setting: '{"mode":"light"}' },
      roles: [{ id: 1, name: '超级管理员', code: 'super_admin' }],
      codes: ['*'],
      routers: buildMenuTree(menuSeed)
    })
  },
  {
    url: '/admin/system/user',
    method: 'get',
    handler: () => ok({
      user: { id: 1, username: 'admin', nickname: '超级管理员', avatar: '', email: 'admin@test.com', phone: '13800000000', dept_id: 1, dashboard: 'statistics', backend_setting: '{"mode":"light"}' },
      roles: [{ id: 1, name: '超级管理员', code: 'super_admin' }],
      codes: ['*'],
      routers: buildMenuTree(menuSeed)
    })
  },

  // ===== 统计 =====
  {
    url: '/admin/system/statistics',
    method: 'get',
    handler: () => ok({ user: 128, attach: 1024, login: 2048, operate: 512 })
  },
  {
    url: '/admin/system/loginChart',
    method: 'get',
    handler: () => ok({
      login_date: ['03-15', '03-16', '03-17', '03-18', '03-19', '03-20', '03-21'],
      login_count: [12, 18, 15, 22, 8, 16, 25]
    })
  },

  // ===== 用户管理 =====
  {
    url: '/admin/user/index',
    method: 'get',
    handler: ({ params }) => {
      const page = parseInt(params?.page || 1)
      const pageSize = parseInt(params?.page_size || 20)
      const result = store.list('users', null, page, pageSize)
      return page(result.data, result.total, page, pageSize)
    }
  },
  {
    url: '/admin/user/read',
    method: 'get',
    handler: ({ url }) => {
      const id = getUrlParam(url, 'id')
      const item = store.getById('users', id)
      return item ? ok(item) : fail('用户不存在')
    }
  },
  {
    url: '/admin/user/save',
    method: 'post',
    handler: ({ data, isDemoMode }) => {
      if (isDemoMode) return forbidden()
      return ok(store.add('users', data))
    }
  },
  {
    url: '/admin/user/update',
    method: 'put',
    handler: ({ url, data, isDemoMode }) => {
      if (isDemoMode) return forbidden()
      const id = getUrlParam(url, 'id')
      const item = store.update('users', id, data)
      return item ? ok(item) : fail('用户不存在')
    }
  },
  {
    url: '/admin/user/destroy',
    method: 'delete',
    handler: ({ data, isDemoMode }) => {
      if (isDemoMode) return forbidden()
      const ids = Array.isArray(data?.ids) ? data.ids : [data?.id]
      store.deleteMany('users', ids)
      return ok({ success: true })
    }
  },
  {
    url: '/admin/user/changeStatus',
    method: 'post',
    handler: ({ data, isDemoMode }) => {
      if (isDemoMode) return forbidden()
      const item = store.toggleStatus('users', data.id, 'status')
      return item ? ok(item) : fail('用户不存在')
    }
  },
  {
    url: '/admin/user/initUserPassword',
    method: 'post',
    handler: ({ isDemoMode }) => {
      if (isDemoMode) return forbidden()
      return ok({ success: true, message: '密码已重置为: 123456' })
    }
  },
  {
    url: '/admin/user/clearCache',
    method: 'post',
    handler: ({ isDemoMode }) => {
      if (isDemoMode) return forbidden()
      return ok({ success: true, message: '缓存已清除' })
    }
  },
  {
    url: '/admin/user/setHomePage',
    method: 'post',
    handler: ({ isDemoMode }) => {
      if (isDemoMode) return forbidden()
      return ok({ success: true })
    }
  },
  {
    url: '/admin/user/updateInfo',
    method: 'post',
    handler: ({ data }) => ok({ success: true })
  },
  {
    url: '/admin/user/modifyPassword',
    method: 'post',
    handler: () => ok({ success: true })
  },

  // ===== 部门管理 =====
  {
    url: '/admin/dept/index',
    method: 'get',
    handler: () => {
      const depts = store.getCollection('depts')
      return ok(depts.map(d => ({ ...d, label: d.name, value: d.id })))
    }
  },
  {
    url: '/admin/dept/leaders',
    method: 'get',
    handler: () => ok([
      { id: '1', username: 'admin', nickname: '管理员' }
    ])
  },
  {
    url: '/admin/dept/addLeader',
    method: 'post',
    handler: ({ isDemoMode }) => {
      if (isDemoMode) return forbidden()
      return ok({ success: true })
    }
  },
  {
    url: '/admin/dept/delLeader',
    method: 'delete',
    handler: ({ isDemoMode }) => {
      if (isDemoMode) return forbidden()
      return ok({ success: true })
    }
  },
  {
    url: '/admin/dept/save',
    method: 'post',
    handler: ({ data, isDemoMode }) => {
      if (isDemoMode) return forbidden()
      return ok(store.add('depts', data))
    }
  },
  {
    url: '/admin/dept/update',
    method: 'put',
    handler: ({ url, data, isDemoMode }) => {
      if (isDemoMode) return forbidden()
      const id = getUrlParam(url, 'id')
      const item = store.update('depts', id, data)
      return item ? ok(item) : fail('部门不存在')
    }
  },
  {
    url: '/admin/dept/destroy',
    method: 'delete',
    handler: ({ data, isDemoMode }) => {
      if (isDemoMode) return forbidden()
      store.delete('depts', data.id)
      return ok({ success: true })
    }
  },
  {
    url: '/admin/dept/changeStatus',
    method: 'post',
    handler: ({ data, isDemoMode }) => {
      if (isDemoMode) return forbidden()
      const item = store.toggleStatus('depts', data.id, 'status')
      return item ? ok(item) : fail('部门不存在')
    }
  },

  // ===== 角色管理 =====
  {
    url: '/admin/role/index',
    method: 'get',
    handler: ({ params }) => {
      const page = parseInt(params?.page || 1)
      const pageSize = parseInt(params?.page_size || 20)
      const result = store.list('roles', null, page, pageSize)
      return page(result.data, result.total, page, pageSize)
    }
  },
  {
    url: '/admin/role/getMenuByRole',
    method: 'get',
    handler: () => ok([])
  },
  {
    url: '/admin/role/getDeptByRole',
    method: 'get',
    handler: () => ok([])
  },
  {
    url: '/admin/role/save',
    method: 'post',
    handler: ({ data, isDemoMode }) => {
      if (isDemoMode) return forbidden()
      return ok(store.add('roles', data))
    }
  },
  {
    url: '/admin/role/update',
    method: 'put',
    handler: ({ url, data, isDemoMode }) => {
      if (isDemoMode) return forbidden()
      const id = getUrlParam(url, 'id')
      const item = store.update('roles', id, data)
      return item ? ok(item) : fail('角色不存在')
    }
  },
  {
    url: '/admin/role/destroy',
    method: 'delete',
    handler: ({ data, isDemoMode }) => {
      if (isDemoMode) return forbidden()
      store.delete('roles', data.id)
      return ok({ success: true })
    }
  },
  {
    url: '/admin/role/changeStatus',
    method: 'post',
    handler: ({ data, isDemoMode }) => {
      if (isDemoMode) return forbidden()
      const item = store.toggleStatus('roles', data.id, 'status')
      return item ? ok(item) : fail('角色不存在')
    }
  },
  {
    url: '/admin/role/menuPermission',
    method: 'post',
    handler: () => ok({ success: true })
  },
  {
    url: '/admin/role/dataPermission',
    method: 'post',
    handler: () => ok({ success: true })
  },

  // ===== 菜单管理 =====
  {
    url: '/admin/menu/index',
    method: 'get',
    handler: () => ok(store.getCollection('menus'))
  },
  {
    url: '/admin/menu/accessMenu',
    method: 'get',
    handler: () => ok(buildMenuTree(store.getCollection('menus')))
  },
  {
    url: '/admin/menu/save',
    method: 'post',
    handler: ({ data, isDemoMode }) => {
      if (isDemoMode) return forbidden()
      return ok(store.add('menus', data))
    }
  },
  {
    url: '/admin/menu/update',
    method: 'put',
    handler: ({ url, data, isDemoMode }) => {
      if (isDemoMode) return forbidden()
      const id = getUrlParam(url, 'id')
      const item = store.update('menus', id, data)
      return item ? ok(item) : fail('菜单不存在')
    }
  },
  {
    url: '/admin/menu/destroy',
    method: 'delete',
    handler: ({ data, isDemoMode }) => {
      if (isDemoMode) return forbidden()
      store.delete('menus', data.id)
      return ok({ success: true })
    }
  },

  // ===== 字典 =====
  {
    url: '/admin/system/dictAll',
    method: 'get',
    handler: () => ok(dictData)
  },
  {
    url: '/admin/dictType/index',
    method: 'get',
    handler: () => ok([
      { id: '1', name: '数据状态', code: 'data_status', status: 1 },
      { id: '2', name: '性别', code: 'sex', status: 1 }
    ])
  },
  {
    url: '/admin/dictType/save',
    method: 'post',
    handler: ({ data, isDemoMode }) => {
      if (isDemoMode) return forbidden()
      return ok(store.add('dictTypes', data))
    }
  },
  {
    url: '/admin/dictType/update',
    method: 'put',
    handler: ({ url, data, isDemoMode }) => {
      if (isDemoMode) return forbidden()
      const id = getUrlParam(url, 'id')
      return ok(store.update('dictTypes', id, data) || {})
    }
  },
  {
    url: '/admin/dictType/destroy',
    method: 'delete',
    handler: ({ data, isDemoMode }) => {
      if (isDemoMode) return forbidden()
      store.delete('dictTypes', data.id)
      return ok({ success: true })
    }
  },
  {
    url: '/admin/dictType/changeStatus',
    method: 'post',
    handler: ({ data, isDemoMode }) => {
      if (isDemoMode) return forbidden()
      return ok({ success: true })
    }
  },
  {
    url: '/admin/dictData/index',
    method: 'get',
    handler: ({ params }) => {
      const page = parseInt(params?.page || 1)
      const pageSize = parseInt(params?.page_size || 20)
      const dictDataList = [
        { id: '1', label: '启用', value: 1, type_code: 'data_status', status: 1, sort: 1 },
        { id: '2', label: '禁用', value: 0, type_code: 'data_status', status: 1, sort: 2 },
        { id: '3', label: '男', value: 1, type_code: 'sex', status: 1, sort: 1 },
        { id: '4', label: '女', value: 2, type_code: 'sex', status: 1, sort: 2 }
      ]
      return page(dictDataList, dictDataList.length, page, pageSize)
    }
  },
  {
    url: '/admin/dictData/save',
    method: 'post',
    handler: ({ data, isDemoMode }) => {
      if (isDemoMode) return forbidden()
      return ok(store.add('dictDatas', data))
    }
  },
  {
    url: '/admin/dictData/update',
    method: 'put',
    handler: ({ url, data, isDemoMode }) => {
      if (isDemoMode) return forbidden()
      const id = getUrlParam(url, 'id')
      return ok(store.update('dictDatas', id, data) || {})
    }
  },
  {
    url: '/admin/dictData/destroy',
    method: 'delete',
    handler: ({ data, isDemoMode }) => {
      if (isDemoMode) return forbidden()
      store.delete('dictDatas', data.id)
      return ok({ success: true })
    }
  },
  {
    url: '/admin/dictData/changeStatus',
    method: 'post',
    handler: ({ isDemoMode }) => {
      if (isDemoMode) return forbidden()
      return ok({ success: true })
    }
  },
  {
    url: '/admin/dataDict/index',
    method: 'get',
    handler: ({ url }) => {
      const code = getUrlParam(url, 'code')
      return ok(dictData[code] || [])
    }
  },

  // ===== 岗位管理 =====
  {
    url: '/admin/post/index',
    method: 'get',
    handler: () => ok([
      { id: '1', name: '总经理', code: 'ceo', sort: 1, status: 1, remark: '公司最高管理者' },
      { id: '2', name: '技术总监', code: 'cto', sort: 2, status: 1, remark: '技术部门负责人' },
      { id: '3', name: '开发工程师', code: 'dev', sort: 3, status: 1, remark: '开发岗位' }
    ])
  },
  {
    url: '/admin/post/read',
    method: 'get',
    handler: ({ url }) => {
      const id = getUrlParam(url, 'id')
      const posts = [
        { id: '1', name: '总经理', code: 'ceo', sort: 1, status: 1, remark: '公司最高管理者' },
        { id: '2', name: '技术总监', code: 'cto', sort: 2, status: 1, remark: '技术部门负责人' }
      ]
      const item = posts.find(p => p.id === id)
      return item ? ok(item) : fail('岗位不存在')
    }
  },
  {
    url: '/admin/post/save',
    method: 'post',
    handler: ({ data, isDemoMode }) => {
      if (isDemoMode) return forbidden()
      return ok(store.add('posts', data))
    }
  },
  {
    url: '/admin/post/update',
    method: 'put',
    handler: ({ url, data, isDemoMode }) => {
      if (isDemoMode) return forbidden()
      const id = getUrlParam(url, 'id')
      return ok(store.update('posts', id, data) || {})
    }
  },
  {
    url: '/admin/post/changeStatus',
    method: 'post',
    handler: ({ isDemoMode }) => {
      if (isDemoMode) return forbidden()
      return ok({ success: true })
    }
  },
  {
    url: '/admin/post/destroy',
    method: 'delete',
    handler: ({ data, isDemoMode }) => {
      if (isDemoMode) return forbidden()
      store.delete('posts', data.id)
      return ok({ success: true })
    }
  },

  // ===== 配置管理 =====
  {
    url: '/admin/config/index',
    method: 'get',
    handler: () => ok([
      { id: '1', key: 'site_name', value: 'U验证系统', group_id: '1', remark: '网站名称' },
      { id: '2', key: 'site_logo', value: '', group_id: '1', remark: '网站Logo' }
    ])
  },
  {
    url: '/admin/config/save',
    method: 'post',
    handler: ({ data, isDemoMode }) => {
      if (isDemoMode) return forbidden()
      return ok(store.add('configs', data))
    }
  },
  {
    url: '/admin/config/update',
    method: 'put',
    handler: ({ url, data, isDemoMode }) => {
      if (isDemoMode) return forbidden()
      const id = getUrlParam(url, 'id')
      return ok(store.update('configs', id, data) || {})
    }
  },
  {
    url: '/admin/config/updateByKeys',
    method: 'post',
    handler: ({ isDemoMode }) => {
      if (isDemoMode) return forbidden()
      return ok({ success: true })
    }
  },
  {
    url: '/admin/config/batchUpdate',
    method: 'post',
    handler: ({ isDemoMode }) => {
      if (isDemoMode) return forbidden()
      return ok({ success: true })
    }
  },
  {
    url: '/admin/config/destroy',
    method: 'delete',
    handler: ({ data, isDemoMode }) => {
      if (isDemoMode) return forbidden()
      store.delete('configs', data.id)
      return ok({ success: true })
    }
  },
  {
    url: '/admin/configGroup/index',
    method: 'get',
    handler: () => ok([
      { id: '1', name: '基础设置', code: 'basic', sort: 1 },
      { id: '2', name: '邮件设置', code: 'email', sort: 2 }
    ])
  },
  {
    url: '/admin/configGroup/save',
    method: 'post',
    handler: ({ data, isDemoMode }) => {
      if (isDemoMode) return forbidden()
      return ok(store.add('configGroups', data))
    }
  },
  {
    url: '/admin/configGroup/update',
    method: 'put',
    handler: ({ url, data, isDemoMode }) => {
      if (isDemoMode) return forbidden()
      const id = getUrlParam(url, 'id')
      return ok(store.update('configGroups', id, data) || {})
    }
  },
  {
    url: '/admin/configGroup/destroy',
    method: 'delete',
    handler: ({ data, isDemoMode }) => {
      if (isDemoMode) return forbidden()
      store.delete('configGroups', data.id)
      return ok({ success: true })
    }
  },
  {
    url: '/admin/configGroup/email',
    method: 'post',
    handler: () => ok({ success: true, message: '邮件发送成功' })
  },

  // ===== 数据库管理 =====
  {
    url: '/admin/database/index',
    method: 'get',
    handler: () => ok([
      { name: 'users', rows: 12580, size: '2.5MB', create_time: '2024-01-01', update_time: '2024-03-20' },
      { name: 'orders', rows: 8960, size: '1.8MB', create_time: '2024-01-01', update_time: '2024-03-20' },
      { name: 'products', rows: 520, size: '0.3MB', create_time: '2024-01-01', update_time: '2024-03-19' }
    ])
  },
  {
    url: '/admin/database/dataSource',
    method: 'get',
    handler: () => ok([
      { name: 'master', type: 'MySQL', host: '127.0.0.1', port: 3306, database: 'nakamasa', status: 'online' }
    ])
  },
  {
    url: '/admin/database/detailed',
    method: 'get',
    handler: () => ok([
      { field: 'id', type: 'int(11)', null: 'NO', key: 'PRI', default: null, extra: 'auto_increment' },
      { field: 'username', type: 'varchar(50)', null: 'NO', key: '', default: null, extra: '' },
      { field: 'email', type: 'varchar(100)', null: 'YES', key: '', default: null, extra: '' }
    ])
  },
  {
    url: '/admin/database/recycle',
    method: 'get',
    handler: () => ok([])
  },
  {
    url: '/admin/database/delete',
    method: 'delete',
    handler: ({ isDemoMode }) => {
      if (isDemoMode) return forbidden()
      return ok({ success: true })
    }
  },
  {
    url: '/admin/database/recovery',
    method: 'post',
    handler: ({ isDemoMode }) => {
      if (isDemoMode) return forbidden()
      return ok({ success: true })
    }
  },
  {
    url: '/admin/database/optimize',
    method: 'post',
    handler: () => ok({ success: true, message: '优化完成' })
  },
  {
    url: '/admin/database/fragment',
    method: 'post',
    handler: () => ok({ success: true, message: '碎片清理完成' })
  },

  // ===== 服务监控 =====
  {
    url: '/admin/system/monitor',
    method: 'get',
    handler: () => ok({
      cpu: { usage: 35.6, cores: 4, model: 'ARM Cortex-A76' },
      memory: { total: 8192, used: 4096, free: 4096, usage: 50 },
      disk: { total: 128000, used: 64000, free: 64000, usage: 50 },
      system: { os: 'Linux 5.4.289', arch: 'aarch64', hostname: 'termux', uptime: 86400 },
      jvm: { version: 'N/A', heap_used: 0, heap_max: 0 },
      runtime: { version: 'Rust 1.85', framework: 'Salvo' }
    })
  },

  // ===== 附件管理 =====
  {
    url: '/admin/attachment/index',
    method: 'get',
    handler: ({ params }) => {
      const page = parseInt(params?.page || 1)
      const pageSize = parseInt(params?.page_size || 20)
      const attachments = [
        { id: '1', name: 'logo.png', path: '/upload/image/202403/logo.png', size: 15360, mime_type: 'image/png', create_time: '2024-03-20 10:00:00' },
        { id: '2', name: 'banner.jpg', path: '/upload/image/202403/banner.jpg', size: 51200, mime_type: 'image/jpeg', create_time: '2024-03-20 11:00:00' },
        { id: '3', name: 'doc.pdf', path: '/upload/file/202403/doc.pdf', size: 102400, mime_type: 'application/pdf', create_time: '2024-03-20 12:00:00' }
      ]
      return page(attachments, attachments.length, page, pageSize)
    }
  },
  {
    url: '/admin/attachment/destroy',
    method: 'delete',
    handler: ({ data, isDemoMode }) => {
      if (isDemoMode) return forbidden()
      return ok({ success: true })
    }
  },

  // ===== 邮件日志 =====
  {
    url: '/admin/email/index',
    method: 'get',
    handler: ({ params }) => {
      const page = parseInt(params?.page || 1)
      const pageSize = parseInt(params?.page_size || 20)
      const emails = [
        { id: '1', to: 'user@example.com', subject: '验证码', status: 1, error: null, create_time: '2024-03-20 10:00:00' },
        { id: '2', to: 'admin@example.com', subject: '系统通知', status: 1, error: null, create_time: '2024-03-20 11:00:00' }
      ]
      return page(emails, emails.length, page, pageSize)
    }
  },
  {
    url: '/admin/email/destroy',
    method: 'delete',
    handler: ({ data, isDemoMode }) => {
      if (isDemoMode) return forbidden()
      return ok({ success: true })
    }
  },

  // ===== 操作日志 =====
  {
    url: '/admin/logs/getOperLogPageList',
    method: 'get',
    handler: ({ params }) => {
      const page = parseInt(params?.page || 1)
      const pageSize = parseInt(params?.page_size || 20)
      const logs = [
        { id: '1', username: 'admin', method: 'POST', url: '/admin/user/add', ip: '127.0.0.1', status: 1, message: '添加用户成功', create_time: '2024-03-20 10:00:00' },
        { id: '2', username: 'admin', method: 'PUT', url: '/admin/user/edit', ip: '127.0.0.1', status: 1, message: '修改用户成功', create_time: '2024-03-20 11:00:00' }
      ]
      return page(logs, logs.length, page, pageSize)
    }
  },
  {
    url: '/admin/logs/deleteOperLog',
    method: 'delete',
    handler: ({ data, isDemoMode }) => {
      if (isDemoMode) return forbidden()
      return ok({ success: true })
    }
  },

  // ===== 登录日志 =====
  {
    url: '/admin/logs/getLoginLogPageList',
    method: 'get',
    handler: ({ params }) => {
      const page = parseInt(params?.page || 1)
      const pageSize = parseInt(params?.page_size || 20)
      const result = store.list('loginLogs', null, page, pageSize)
      return page(result.data, result.total, page, pageSize)
    }
  },
  {
    url: '/admin/logs/deleteLoginLog',
    method: 'delete',
    handler: ({ data, isDemoMode }) => {
      if (isDemoMode) return forbidden()
      const ids = Array.isArray(data?.ids) ? data.ids : [data?.id]
      store.deleteMany('loginLogs', ids)
      return ok({ success: true })
    }
  },

  // ===== 公告 =====
  {
    url: '/admin/system/notice',
    method: 'get',
    handler: () => ok([{ id: '1', title: '系统公告', content: '欢迎使用 Mock 模式', type: 1, status: 1, create_time: '2024-03-20' }])
  },

  // ===== 缓存 =====
  {
    url: '/admin/system/clearAllCache',
    method: 'get',
    handler: () => ok({ success: true })
  },

  // ===== 文件上传 =====
  {
    url: '/admin/upload/img',
    method: 'post',
    handler: () => {
      // 模拟返回相对路径，与后端格式一致
      const dateStr = new Date().toISOString().slice(0, 7).replace('-', '')
      const filename = generateId() + '.jpg'
      return ok({ url: `/upload/image/${dateStr}/${filename}` })
    }
  },
  {
    url: '/admin/system/uploadImage',
    method: 'post',
    handler: () => ok({ id: generateId(), url: 'https://picsum.photos/200/200', name: 'mock_image.jpg' })
  },
  {
    url: '/admin/system/uploadFile',
    method: 'post',
    handler: () => ok({ id: generateId(), url: 'https://picsum.photos/200/200', name: 'mock_file.jpg', size: 1024 })
  },

  // ===== 用户列表 =====
  {
    url: '/admin/system/getUserList',
    method: 'get',
    handler: () => {
      const users = store.getCollection('users')
      return ok(users.map(u => ({ id: u.id, username: u.username, nickname: u.nickname })))
    }
  },

  // ===== 设置 =====
  {
    url: '/admin/system/saveSetting',
    method: 'post',
    handler: ({ data }) => {
      localStorage.setItem('setting', JSON.stringify(data))
      return ok({ success: true })
    }
  },
  {
    url: '/admin/system/getSetting',
    method: 'get',
    handler: () => {
      const setting = localStorage.getItem('setting')
      return ok(setting ? JSON.parse(setting) : {})
    }
  },

  // ===== 应用管理 =====
  {
    url: '/admin/app/list',
    method: 'post',
    handler: ({ data }) => {
      const page = parseInt(data?.pg || data?.page || 1)
      const pageSize = 12
      let list = store.getCollection('apps') || []
      
      // 搜索条件过滤
      const so = data?.so || {}
      if (so.type) {
        list = list.filter(item => item.type === so.type)
      }
      if (so.keyword) {
        const keyword = String(so.keyword).toLowerCase()
        list = list.filter(item => 
          String(item.id).includes(keyword) || 
          (item.name || '').toLowerCase().includes(keyword)
        )
      }
      
      const total = list.length
      const start = (page - 1) * pageSize
      const pageList = list.slice(start, start + pageSize)
      
      return ok({
        list: pageList.map(a => ({
          id: a.id,
          app_key: a.app_key || a.id,
          app_type: a.type,
          app_name: a.name,
          app_logo: a.logo || '',
          app_state: a.status === 1 ? 'on' : 'off'
        })),
        currentPage: page,
        pageTotal: Math.ceil(total / pageSize),
        dataTotal: total
      })
    }
  },
  {
    url: '/admin/app/all',
    method: 'get',
    handler: () => {
      const apps = store.getCollection('apps') || []
      return ok({
        currentPage: 1,
        dataTotal: apps.length,
        list: apps.map(a => ({ 
          id: a.id, 
          app_key: a.app_key || a.id,
          app_type: a.type, 
          app_name: a.name,
          app_logo: a.logo || '',
          app_state: a.status === 1 ? 'on' : 'off'
        })),
        pageTotal: 1
      })
    }
  },
  {
    url: '/admin/app/get',
    method: 'post',
    handler: ({ data }) => {
      const item = store.getById('apps', data?.id)
      return item ? ok({
        id: item.id,
        app_key: item.app_key || item.id,
        app_type: item.type,
        app_name: item.name,
        app_logo: item.logo || '',
        app_state: item.status === 1 ? 'on' : 'off'
      }) : fail('应用不存在')
    }
  },
  {
    url: '/admin/app/add',
    method: 'post',
    handler: ({ data, isDemoMode }) => {
      if (isDemoMode) return forbidden()
      const newItem = {
        id: generateId(),
        app_key: 'key_' + Date.now(),
        app_type: data?.app_type || 'user',
        name: data?.app_name,
        type: data?.app_type || 'user',
        logo: data?.app_logo || '',
        status: 1,
        version: '1.0.0'
      }
      return ok(store.add('apps', newItem))
    }
  },
  {
    url: '/admin/app/edit',
    method: 'post',
    handler: ({ data, isDemoMode, headers }) => {
      if (isDemoMode) return forbidden()
      const appid = headers?.appid || data?.id
      
      // 保存所有字段
      const updates = {
        name: data?.app_name,
        type: data?.app_type,
        logo: data?.app_logo,
        status: data?.app_state === 'on' ? 1 : 0,
        app_mode: data?.app_mode,
        app_off_msg: data?.app_off_msg,
        // 注册控制
        reg_state: data?.reg_state,
        reg_off_msg: data?.reg_off_msg,
        reg_way: data?.reg_way,
        reg_is_inviter: data?.reg_is_inviter,
        reg_time_sn: data?.reg_time_sn,
        reg_time_ip: data?.reg_time_ip,
        // 登录控制
        logon_state: data?.logon_state,
        logon_off_msg: data?.logon_off_msg,
        logon_token_exp: data?.logon_token_exp,
        logon_sn_dk: data?.logon_sn_dk,
        logon_ban_expire: data?.logon_ban_expire,
        logon_sn_num: data?.logon_sn_num,
        logon_sn_over_ban: data?.logon_sn_over_ban,
        login_prevent_brute_force: data?.login_prevent_brute_force,
        logon_sn_unbde_auto: data?.logon_sn_unbde_auto,
        logon_sn_unbde_type: data?.logon_sn_unbde_type,
        logon_sn_unbde_val: data?.logon_sn_unbde_val,
        // 第三方登录
        logon_open_wxconfig: data?.logon_open_wxconfig,
        logon_open_qqconfig: data?.logon_open_qqconfig
      }
      
      // 过滤掉 undefined 值
      Object.keys(updates).forEach(key => {
        if (updates[key] === undefined) delete updates[key]
      })
      
      const item = store.update('apps', appid, updates)
      return item ? ok({ success: true, msg: '保存成功' }) : fail('应用不存在')
    }
  },
  {
    url: '/admin/app/del',
    method: 'post',
    handler: ({ data, isDemoMode }) => {
      if (isDemoMode) return forbidden()
      store.delete('apps', data?.id)
      return ok({ success: true })
    }
  },
  {
    url: '/admin/app/getInherit',
    method: 'post',
    handler: () => {
      const apps = store.getCollection('apps') || []
      const kami = apps.filter(a => a.type === 'kami').map(a => ({ id: a.id, app_name: a.name }))
      const user = apps.filter(a => a.type === 'user').map(a => ({ id: a.id, app_name: a.name }))
      return ok({ kami, user })
    }
  },
  {
    url: '/admin/app/getUrl',
    method: 'post',
    handler: () => ok({ url: 'http://127.0.0.1:8080' })
  },
  {
    url: '/admin/app/getInfo',
    method: 'post',
    handler: ({ data, headers }) => {
      // 从 headers 获取 appid，后端逻辑也是如此
      const appid = headers?.appid || data?.id
      const item = appid ? store.getById('apps', appid) : null
      
      if (!item) {
        // 返回默认配置
        return ok({
          id: appid || 'app001',
          app_name: '测试应用',
          app_key: 'test_key_' + Date.now(),
          app_logo: '',
          app_mode: 'y',
          app_state: 'on',
          app_off_msg: '',
          reg_state: 'on',
          reg_off_msg: '',
          reg_way: 'email',
          reg_is_inviter: 'n',
          reg_time_sn: 24,
          reg_time_ip: 24,
          logon_state: 'on',
          logon_off_msg: '',
          logon_token_exp: 86400,
          logon_sn_dk: 'n',
          logon_ban_expire: 'y',
          logon_sn_num: 1,
          logon_sn_over_ban: true,
          login_prevent_brute_force: true,
          logon_sn_unbde_auto: false,
          logon_sn_unbde_type: 'fen',
          logon_sn_unbde_val: 100,
          logon_open_wxconfig: { state: 'on', appID: '', appSecret: '' },
          logon_open_qqconfig: { state: 'on', appID: '', appKey: '' }
        })
      }
      
      const result = {
        id: item.id,
        app_key: item.app_key || item.id,
        app_type: item.type,
        app_name: item.name,
        app_logo: item.logo || '',
        app_state: item.status === 1 ? 'on' : 'off',
        app_mode: item.app_mode || 'y',
        app_off_msg: item.app_off_msg || '',
        // 注册控制
        reg_state: item.reg_state || 'on',
        reg_off_msg: item.reg_off_msg || '',
        reg_way: item.reg_way || 'email',
        reg_is_inviter: item.reg_is_inviter || 'n',
        reg_time_sn: item.reg_time_sn || 24,
        reg_time_ip: item.reg_time_ip || 24,
        // 登录控制
        logon_state: item.logon_state || 'on',
        logon_off_msg: item.logon_off_msg || '',
        logon_token_exp: item.logon_token_exp || 86400,
        logon_sn_dk: item.logon_sn_dk || 'n',
        logon_ban_expire: item.logon_ban_expire || 'y',
        logon_sn_num: item.logon_sn_num ?? 1,
        logon_sn_over_ban: item.logon_sn_over_ban ?? true,
        login_prevent_brute_force: item.login_prevent_brute_force ?? true,
        logon_sn_unbde_auto: item.logon_sn_unbde_auto ?? false,
        logon_sn_unbde_type: item.logon_sn_unbde_type || 'fen',
        logon_sn_unbde_val: item.logon_sn_unbde_val || 100,
        // 第三方登录
        logon_open_wxconfig: item.logon_open_wxconfig || { state: 'on', appID: '', appSecret: '' },
        logon_open_qqconfig: item.logon_open_qqconfig || { state: 'on', appID: '', appKey: '' }
      }
      
      // 根据请求的 field 过滤返回字段
      if (data?.field && Array.isArray(data.field)) {
        const filtered = { id: result.id }
        data.field.forEach(f => {
          if (result[f] !== undefined) {
            filtered[f] = result[f]
          }
        })
        return ok(filtered)
      }
      
      return ok(result)
    }
  },

  // ===== 版本管理 =====
  {
    url: '/admin/ver/list',
    method: 'post',
    handler: ({ data }) => {
      const page = parseInt(data?.page || 1)
      const pageSize = parseInt(data?.page_size || 20)
      const result = store.list('versions', null, page, pageSize)
      return page(result.data, result.total, page, pageSize)
    }
  },
  {
    url: '/admin/ver/group',
    method: 'get',
    handler: () => ok(store.getCollection('versionGroups'))
  },
  {
    url: '/admin/ver/add',
    method: 'post',
    handler: ({ data, isDemoMode }) => {
      if (isDemoMode) return forbidden()
      return ok(store.add('versions', data))
    }
  },
  {
    url: '/admin/ver/edit',
    method: 'post',
    handler: ({ data, isDemoMode }) => {
      if (isDemoMode) return forbidden()
      const item = store.update('versions', data?.id, data)
      return item ? ok(item) : fail('版本不存在')
    }
  },
  {
    url: '/admin/ver/del',
    method: 'post',
    handler: ({ data, isDemoMode }) => {
      if (isDemoMode) return forbidden()
      store.delete('versions', data?.id)
      return ok({ success: true })
    }
  },

  // ===== 卡密管理 =====
  {
    url: '/admin/cdkKami/list',
    method: 'post',
    handler: ({ data }) => {
      const page = parseInt(data?.page || 1)
      const pageSize = parseInt(data?.page_size || 20)
      const result = store.list('cdkKamis', null, page, pageSize)
      return page(result.data, result.total, page, pageSize)
    }
  },
  {
    url: '/admin/cdkKami/add',
    method: 'post',
    handler: ({ data, isDemoMode }) => {
      if (isDemoMode) return forbidden()
      const count = data?.count || 1
      const items = []
      for (let i = 0; i < count; i++) {
        items.push(store.add('cdkKamis', {
          ...data,
          code: generateCdkCode(),
          status: 0
        }))
      }
      return ok({ count: items.length, items })
    }
  },
  {
    url: '/admin/cdkKami/edit',
    method: 'post',
    handler: ({ data, isDemoMode }) => {
      if (isDemoMode) return forbidden()
      const item = store.update('cdkKamis', data?.id, data)
      return item ? ok(item) : fail('卡密不存在')
    }
  },
  {
    url: '/admin/cdkKami/del',
    method: 'post',
    handler: ({ data, isDemoMode }) => {
      if (isDemoMode) return forbidden()
      store.delete('cdkKamis', data?.id)
      return ok({ success: true })
    }
  },

  // ===== 卡密分组 =====
  {
    url: '/admin/cdkGroup/list',
    method: 'post',
    handler: ({ data }) => {
      const page = parseInt(data?.page || 1)
      const pageSize = parseInt(data?.size || 20)
      const keyword = data?.so?.keyword || ''
      let result = store.list('cdkGroups', null, page, pageSize)
      
      // 搜索过滤
      if (keyword) {
        const filtered = result.list.filter(item => 
          item.name?.includes(keyword)
        )
        result = {
          list: filtered,
          total: filtered.length,
          currentPage: page,
          pageTotal: Math.ceil(filtered.length / pageSize) || 1,
          dataTotal: filtered.length
        }
      }
      
      return page(result.list, result.total, page, pageSize)
    }
  },
  {
    url: '/admin/cdkGroup/add',
    method: 'post',
    handler: ({ data, isDemoMode }) => {
      if (isDemoMode) return forbidden()
      return ok(store.add('cdkGroups', data))
    }
  },
  {
    url: '/admin/cdkGroup/del',
    method: 'post',
    handler: ({ data, isDemoMode }) => {
      if (isDemoMode) return forbidden()
      store.delete('cdkGroups', data?.id)
      return ok({ success: true })
    }
  },

  // ===== 代理管理 =====
  {
    url: '/admin/agentList/list',
    method: 'post',
    handler: ({ data }) => {
      const page = parseInt(data?.page || 1)
      const pageSize = parseInt(data?.page_size || 20)
      const result = store.list('agents', null, page, pageSize)
      return page(result.data, result.total, page, pageSize)
    }
  },
  {
    url: '/admin/agentList/add',
    method: 'post',
    handler: ({ data, isDemoMode }) => {
      if (isDemoMode) return forbidden()
      return ok(store.add('agents', data))
    }
  },
  {
    url: '/admin/agentList/edit',
    method: 'post',
    handler: ({ data, isDemoMode }) => {
      if (isDemoMode) return forbidden()
      const item = store.update('agents', data?.id, data)
      return item ? ok(item) : fail('代理不存在')
    }
  },
  {
    url: '/admin/agentList/del',
    method: 'post',
    handler: ({ data, isDemoMode }) => {
      if (isDemoMode) return forbidden()
      store.delete('agents', data?.id)
      return ok({ success: true })
    }
  },

  // ===== 订单管理 =====
  {
    url: '/admin/order/list',
    method: 'post',
    handler: ({ data }) => {
      const page = parseInt(data?.page || 1)
      const pageSize = parseInt(data?.page_size || 20)
      const result = store.list('orders', null, page, pageSize)
      return page(result.data, result.total, page, pageSize)
    }
  },
  {
    url: '/admin/order/statistics',
    method: 'post',
    handler: () => ok({
      count: { total: 1580, success_total: 126 },
      money: { total: 25800.00, ali_total: 15600.00, wx_total: 10200.00 }
    })
  },
  {
    url: '/admin/order/edit',
    method: 'post',
    handler: ({ data, isDemoMode }) => {
      if (isDemoMode) return forbidden()
      const item = store.update('orders', data?.id, data)
      return item ? ok(item) : fail('订单不存在')
    }
  },
  {
    url: '/admin/order/close',
    method: 'post',
    handler: ({ data, isDemoMode }) => {
      if (isDemoMode) return forbidden()
      const item = store.update('orders', data?.id, { status: -1 })
      return item ? ok(item) : fail('订单不存在')
    }
  },

  // ===== 支付配置 =====
  {
    url: '/admin/app/pay',
    method: 'get',
    handler: () => ok({
      info: {
        id: 1,
        pay_ali_state: 'off',
        pay_ali_type: 'jie',
        pay_ali_config: {},
        pay_wx_state: 'off',
        pay_wx_type: 'jie',
        pay_wx_config: {}
      },
      plug: [
        {
          id: 'jie',
          name: '皆网支付',
          type: 'all',
          extra: '申请地址: http://pay.jienet.com 邀请码: NzMy',
          form: {
            "accessid": { "name": "AccessID", "type": "input", "placeholder": "用户中心->个人信息->密钥信息" },
            "accesskey": { "name": "AccessKey", "type": "input", "placeholder": "用户中心->个人信息->密钥信息" },
            "pid": { "name": "支付PID", "type": "input", "placeholder": "用户中心->支付渠道->PID(可空)", "extra": "如果空则会根据支付方式轮询随机使用您的支付渠道" }
          }
        },
        {
          id: 'wx',
          name: '微信官方',
          type: 'wx',
          form: {
            "AppID": { "name": "开发者ID", "type": "input", "placeholder": "微信公众号AppID" },
            "AppSecret": { "name": "开发者密码", "type": "input", "placeholder": "微信公众号AppSecret (JSAPI支付必填)", "service": "jsapi" },
            "MchID": { "name": "商户ID", "type": "input", "placeholder": "微信支付商户ID" },
            "ApiV3Key": { "name": "APIv3密钥", "type": "input", "placeholder": "商户APIv3密钥" },
            "ApiCertSerialNo": { "name": "API证书序列号", "type": "input", "placeholder": "商户API证书序列号" },
            "ApiCertPrivateKey": { "name": "API证书私钥", "type": "textarea", "placeholder": "API证书apiclient_key.pem内容" },
            "service": { "name": "支付服务", "type": "select", "multiple": true, "placeholder": "请选择您已开通的服务", "option": { "app": "APP支付", "h5": "H5支付", "jsapi": "JSAPI支付", "qr": "Native支付" } }
          }
        },
        {
          id: 'ali',
          name: '支付宝官方',
          type: 'ali',
          form: {
            "AppID": { "name": "APPID", "type": "input", "placeholder": "支付宝应用ID" },
            "AppPrivateKey": { "name": "应用私钥", "type": "textarea", "placeholder": "应用RSA私钥" },
            "AliPublicKey": { "name": "支付宝公钥", "type": "textarea", "placeholder": "支付宝RSA公钥" },
            "service": { "name": "支付服务", "type": "select", "multiple": true, "placeholder": "请选择您已开通的服务", "option": { "app": "APP支付", "h5": "H5支付", "pc": "电脑支付", "qr": "当面付" } }
          }
        }
      ]
    })
  },
  {
    url: '/admin/app/pay/edit',
    method: 'post',
    handler: ({ data, isDemoMode }) => {
      if (isDemoMode) return forbidden()
      return ok({ success: true })
    }
  },

  // ===== 加密方案 =====
  {
    url: '/admin/encryption/list',
    method: 'post',
    handler: ({ data }) => {
      const page = parseInt(data?.page || 1)
      const pageSize = parseInt(data?.page_size || 20)
      const result = store.list('encryptions', null, page, pageSize)
      return page(result.data, result.total, page, pageSize)
    }
  },
  {
    url: '/admin/encryption/add',
    method: 'post',
    handler: ({ data, isDemoMode }) => {
      if (isDemoMode) return forbidden()
      return ok(store.add('encryptions', data))
    }
  },
  {
    url: '/admin/encryption/edit',
    method: 'post',
    handler: ({ data, isDemoMode }) => {
      if (isDemoMode) return forbidden()
      const item = store.update('encryptions', data?.id, data)
      return item ? ok(item) : fail('加密方案不存在')
    }
  },
  {
    url: '/admin/encryption/del',
    method: 'post',
    handler: ({ data, isDemoMode }) => {
      if (isDemoMode) return forbidden()
      store.delete('encryptions', data?.id)
      return ok({ success: true })
    }
  },
  {
    url: '/admin/encryption/plug',
    method: 'get',
    handler: () => ok([
      { id: 'rc4', name: 'RC4', type: 'stream' },
      { id: 'aes', name: 'AES', type: 'block' },
      { id: 'des', name: 'DES', type: 'block' },
      { id: 'rsa', name: 'RSA', type: 'asymmetric' }
    ])
  },

  // ===== 云函数 =====
  {
    url: '/admin/functions/list',
    method: 'post',
    handler: ({ data }) => {
      const page = parseInt(data?.page || 1)
      const pageSize = parseInt(data?.page_size || 20)
      const result = store.list('functions', null, page, pageSize)
      return page(result.data, result.total, page, pageSize)
    }
  },
  {
    url: '/admin/functions/add',
    method: 'post',
    handler: ({ data, isDemoMode }) => {
      if (isDemoMode) return forbidden()
      return ok(store.add('functions', { ...data, code: data?.code || '' }))
    }
  },
  {
    url: '/admin/functions/edit',
    method: 'post',
    handler: ({ data, isDemoMode }) => {
      if (isDemoMode) return forbidden()
      const item = store.update('functions', data?.id, data)
      return item ? ok(item) : fail('函数不存在')
    }
  },
  {
    url: '/admin/functions/del',
    method: 'post',
    handler: ({ data, isDemoMode }) => {
      if (isDemoMode) return forbidden()
      store.delete('functions', data?.id)
      return ok({ success: true })
    }
  },
  {
    url: '/admin/functions/getCode',
    method: 'post',
    handler: ({ data }) => {
      const item = store.getById('functions', data?.id)
      return item ? ok({ code: item.code || '' }) : fail('函数不存在')
    }
  },

  // ===== 黑名单 =====
  {
    url: '/admin/blocklist/list',
    method: 'post',
    handler: ({ data }) => {
      const page = parseInt(data?.page || 1)
      const pageSize = parseInt(data?.page_size || 20)
      const result = store.list('blocklist', null, page, pageSize)
      return page(result.data, result.total, page, pageSize)
    }
  },
  {
    url: '/admin/blocklist/add',
    method: 'post',
    handler: ({ data, isDemoMode }) => {
      if (isDemoMode) return forbidden()
      return ok(store.add('blocklist', data))
    }
  },
  {
    url: '/admin/blocklist/del',
    method: 'post',
    handler: ({ data, isDemoMode }) => {
      if (isDemoMode) return forbidden()
      store.delete('blocklist', data?.id)
      return ok({ success: true })
    }
  },

  // ===== 商品管理 =====
  {
    url: '/admin/goods/list',
    method: 'post',
    handler: ({ data }) => {
      const page = parseInt(data?.page || 1)
      const pageSize = parseInt(data?.page_size || 20)
      const result = store.list('goods', null, page, pageSize)
      return page(result.data, result.total, page, pageSize)
    }
  },
  {
    url: '/admin/goods/add',
    method: 'post',
    handler: ({ data, isDemoMode }) => {
      if (isDemoMode) return forbidden()
      return ok(store.add('goods', data))
    }
  },
  {
    url: '/admin/goods/edit',
    method: 'post',
    handler: ({ data, isDemoMode }) => {
      if (isDemoMode) return forbidden()
      const item = store.update('goods', data?.id, data)
      return item ? ok(item) : fail('商品不存在')
    }
  },
  {
    url: '/admin/goods/del',
    method: 'post',
    handler: ({ data, isDemoMode }) => {
      if (isDemoMode) return forbidden()
      store.delete('goods', data?.id)
      return ok({ success: true })
    }
  },

  // ===== 业务用户管理 =====
  {
    url: '/admin/user/list',
    method: 'post',
    handler: ({ data }) => {
      const page = parseInt(data?.page || 1)
      const pageSize = parseInt(data?.size || data?.page_size || 20)
      let list = store.getCollection('bizUsers') || []
      
      // 搜索条件过滤
      const so = data?.so || {}
      const now = Math.floor(Date.now() / 1000)
      
      // 状态过滤: 'n' 表示被封禁
      if (so.status === 'n') {
        list = list.filter(item => item.ban > now)
      }
      
      // 用户等级过滤: 1=普通, 2=VIP, 3=永久VIP
      if (so.ug) {
        const ug = parseInt(so.ug)
        if (ug === 1) {
          list = list.filter(item => !item.vip || item.vip < now)
        } else if (ug === 2) {
          list = list.filter(item => item.vip > now && item.vip < 9999999999)
        } else if (ug === 3) {
          list = list.filter(item => item.vip >= 9999999999)
        }
      }
      
      // 关键词搜索
      if (so.keyword && so.keywordType) {
        const keyword = String(so.keyword).toLowerCase()
        list = list.filter(item => {
          const field = so.keywordType
          if (field === 'id') return String(item.id).includes(keyword)
          if (field === 'acctno') return (item.acctno || '').toLowerCase().includes(keyword)
          if (field === 'phone') return (item.phone || '').includes(keyword)
          if (field === 'email') return (item.email || '').toLowerCase().includes(keyword)
          if (field === 'nickname') return (item.nickname || '').toLowerCase().includes(keyword)
          if (field === 'reg_ip') return (item.reg_ip || '').includes(keyword)
          if (field === 'reg_sn') return (item.reg_sn || '').toLowerCase().includes(keyword)
          return true
        })
      }
      
      const total = list.length
      const start = (page - 1) * pageSize
      const pageList = list.slice(start, start + pageSize)
      
      return ok({
        list: pageList.map(u => ({
          id: u.id,
          acctno: u.acctno,
          nickname: u.nickname,
          email: u.email || null,
          phone: u.phone || null,
          avatars: u.avatars || null,
          vip: u.vip || null,
          fen: u.fen || 0,
          ban: u.ban || null,
          ban_msg: u.ban_msg || null,
          reg_time: u.reg_time || 0,
          reg_ip: u.reg_ip || '',
          reg_sn: u.reg_sn || null,
          sn_list: u.sn_list || [],
          sn_max: u.sn_max || 0,
          online: u.online || 0,
          inviter_id: u.inviter_id || 0,
          extend: u.extend || null,
          open_wx: u.open_wx || null,
          open_qq: u.open_qq || null
        })),
        currentPage: page,
        pageTotal: Math.ceil(total / pageSize),
        dataTotal: total
      })
    }
  },
  {
    url: '/admin/user/get',
    method: 'post',
    handler: ({ data }) => {
      const item = store.getById('bizUsers', data?.id)
      if (!item) return fail('用户不存在')
      
      return ok({
        info: {
          id: item.id,
          acctno: item.acctno,
          nickname: item.nickname,
          email: item.email || null,
          phone: item.phone || null,
          avatars: item.avatars || null,
          vip: item.vip || null,
          fen: item.fen || 0,
          ban: item.ban || null,
          ban_msg: item.ban_msg || null,
          reg_time: item.reg_time || 0,
          reg_ip: item.reg_ip || '',
          reg_sn: item.reg_sn || null,
          sn_list: item.sn_list || [],
          sn_max: item.sn_max || 0,
          online: item.online || 0,
          inviter_id: item.inviter_id || 0,
          extend: item.extend || null,
          open_wx: item.open_wx || null,
          open_qq: item.open_qq || null
        },
        log: [
          { type: '登录', time: item.last_login_info?.time || item.reg_time, ip: item.last_login_info?.ip || item.reg_ip, asset_changes: null },
          { type: '注册', time: item.reg_time, ip: item.reg_ip, asset_changes: null }
        ]
      })
    }
  },
  {
    url: '/admin/user/add',
    method: 'post',
    handler: ({ data, isDemoMode }) => {
      if (isDemoMode) return forbidden()
      
      const users = store.getCollection('bizUsers') || []
      const maxId = users.reduce((max, u) => Math.max(max, Number(u.id) || 0), 10000)
      
      const newUser = {
        id: maxId + 1,
        acctno: data?.acctno,
        nickname: data?.acctno || '新用户',
        email: null,
        phone: null,
        avatars: null,
        open_wx: null,
        open_qq: null,
        vip: null,
        fen: 0,
        ban: null,
        ban_msg: null,
        sn_list: [],
        sn_max: 0,
        reg_sn: null,
        reg_time: Math.floor(Date.now() / 1000),
        reg_ip: '127.0.0.1',
        last_login_info: null,
        online: 0,
        inviter_id: 0,
        extend: null
      }
      
      return ok(store.add('bizUsers', newUser))
    }
  },
  {
    url: '/admin/user/edit',
    method: 'post',
    handler: ({ data, isDemoMode }) => {
      if (isDemoMode) return forbidden()
      const item = store.update('bizUsers', data?.id, data)
      return item ? ok({ success: true }) : fail('用户不存在')
    }
  },
  {
    url: '/admin/user/editExtend',
    method: 'post',
    handler: ({ data, isDemoMode }) => {
      if (isDemoMode) return forbidden()
      const item = store.update('bizUsers', data?.id, { extend: data?.extend })
      return item ? ok({ success: true }) : fail('用户不存在')
    }
  },
  {
    url: '/admin/user/del',
    method: 'post',
    handler: ({ data, isDemoMode }) => {
      if (isDemoMode) return forbidden()
      store.delete('bizUsers', data?.id)
      return ok({ success: true })
    }
  },
  {
    url: '/admin/user/delall',
    method: 'post',
    handler: ({ data, isDemoMode }) => {
      if (isDemoMode) return forbidden()
      store.deleteMany('bizUsers', data?.ids || [])
      return ok({ success: true })
    }
  },
  {
    url: '/admin/user/unbindSn',
    method: 'post',
    handler: ({ data, isDemoMode }) => {
      if (isDemoMode) return forbidden()
      const item = store.getById('bizUsers', data?.id)
      if (!item) return fail('用户不存在')
      
      const snList = item.sn_list || []
      const newSnList = snList.filter(sn => sn.udid !== data?.udid)
      store.update('bizUsers', data?.id, { sn_list: newSnList })
      
      return ok({ success: true })
    }
  },
  {
    url: '/admin/user/award',
    method: 'post',
    handler: ({ isDemoMode }) => {
      if (isDemoMode) return forbidden()
      return ok({ success: true })
    }
  },

  // ===== 统计数据 =====
  {
    url: '/admin/statistics/get',
    method: 'get',
    handler: () => ok({
      user: {
        count: 12580,
        onLine: 256,
        onLine_token: 180,
        sign_in: 126,
        sign_in_yesterday: 118,
        census: [120, 150, 180, 200, 160, 220, 280]
      },
      order: {
        count: 8960,
        money_sum: 158900.00,
        today_money: 1580.00,
        yesterday_money: 1420.00,
        today_count: 89,
        yesterday_count: 76,
        today_success_rate: 95.5,
        yesterday_success_rate: 93.2,
        census: [1200, 1800, 1500, 2200, 1900, 2500, 3200]
      },
      kami: {
        count: 5200,
        use_count: 3800,
        census: [50, 80, 65, 90, 75, 100, 120]
      }
    })
  },

  // ===== 可视化分析 API =====
  {
    url: '/api/public-opinion-analysis',
    method: 'post',
    handler: ({ data }) => {
      const quota = data?.quota || 'visitors'
      const baseCount = Math.floor(Math.random() * 50000) + 10000
      const baseGrowth = Math.floor(Math.random() * 30) + 1
      const chartData = []
      for (let i = 0; i < 7; i++) {
        chartData.push({
          x: `Day${i + 1}`,
          y: Math.floor(Math.random() * 1000) + 100,
          name: i % 2 ? '2021' : '2022'
        })
      }
      return ok({
        count: baseCount,
        growth: baseGrowth,
        chartData
      })
    }
  },
  {
    url: '/api/content-period-analysis',
    method: 'post',
    handler: () => {
      const xAxis = ['00:00', '02:00', '04:00', '06:00', '08:00', '10:00', '12:00', '14:00', '16:00', '18:00', '20:00', '22:00']
      return ok({
        xAxis,
        data: [
          { name: '纯文本', value: [120, 80, 50, 30, 60, 180, 320, 450, 380, 520, 480, 200] },
          { name: '图文类', value: [80, 50, 30, 20, 40, 120, 280, 380, 320, 450, 380, 150] },
          { name: '视频类', value: [40, 20, 15, 10, 25, 80, 180, 280, 220, 350, 280, 100] }
        ]
      })
    }
  },
  {
    url: '/api/content-publish',
    method: 'get',
    handler: () => {
      const xAxis = ['周一', '周二', '周三', '周四', '周五', '周六', '周日']
      return ok({
        xAxis,
        data: [
          { name: '纯文本', value: [1200, 1400, 1100, 1600, 1300, 800, 600] },
          { name: '图文类', value: [1800, 2000, 1600, 2200, 1900, 1200, 900] },
          { name: '视频类', value: [800, 1000, 700, 1200, 900, 500, 300] }
        ]
      })
    }
  },
  {
    url: '/api/popular-author/list',
    method: 'get',
    handler: () => ok({
      list: [
        { ranking: 1, author: '张三', contentCount: 1580, clickCount: 25600 },
        { ranking: 2, author: '李四', contentCount: 1420, clickCount: 22300 },
        { ranking: 3, author: '王五', contentCount: 1280, clickCount: 19800 },
        { ranking: 4, author: '赵六', contentCount: 1150, clickCount: 17500 },
        { ranking: 5, author: '孙七', contentCount: 980, clickCount: 15200 }
      ]
    })
  },
  {
    url: '/api/data-chain-growth',
    method: 'post',
    handler: ({ data }) => {
      const quota = data?.quota || 'retentionTrends'
      const baseCount = Math.floor(Math.random() * 10000) + 1000
      const baseGrowth = Math.floor(Math.random() * 100)
      const chartDataValue = []
      for (let i = 0; i < 7; i++) {
        chartDataValue.push(Math.floor(Math.random() * 500) + 100)
      }
      return ok({
        count: baseCount,
        growth: baseGrowth,
        chartData: { data: { value: chartDataValue } }
      })
    }
  },
  {
    url: '/api/data-overview',
    method: 'post',
    handler: () => {
      const xAxis = ['12.10', '12.11', '12.12', '12.13', '12.14', '12.15', '12.16', '12.17']
      return ok({
        xAxis,
        data: [
          { name: '内容生产量', value: [1200, 1400, 1100, 1600, 1300, 1500, 1700, 1902] },
          { name: '内容点击量', value: [1800, 2000, 1600, 2200, 1900, 2100, 2300, 2445] },
          { name: '内容曝光量', value: [2500, 2800, 2200, 3100, 2700, 2900, 3200, 3034] },
          { name: '活跃用户数', value: [1000, 1100, 900, 1300, 1100, 1200, 1400, 1275] }
        ]
      })
    }
  },

  // ===== 日志管理 =====
  {
    url: '/admin/logs/list',
    method: 'post',
    handler: ({ data }) => {
      const page = parseInt(data?.page || 1)
      const pageSize = parseInt(data?.page_size || 20)
      const result = store.list('logs', null, page, pageSize)
      return page(result.data, result.total, page, pageSize)
    }
  },
  {
    url: '/admin/logs/list/user',
    method: 'post',
    handler: ({ data }) => {
      const page = parseInt(data?.page || 1)
      const pageSize = parseInt(data?.page_size || 20)
      const result = store.list('userLogs', null, page, pageSize)
      return page(result.data, result.total, page, pageSize)
    }
  },
  {
    url: '/admin/logs/list/admin',
    method: 'post',
    handler: ({ data }) => {
      const page = parseInt(data?.page || 1)
      const pageSize = parseInt(data?.page_size || 20)
      const result = store.list('adminLogs', null, page, pageSize)
      return page(result.data, result.total, page, pageSize)
    }
  },
  {
    url: '/admin/logs/type/user',
    method: 'get',
    handler: () => ok([
      { label: '登录', value: 'login' },
      { label: '注册', value: 'register' },
      { label: '充值', value: 'recharge' },
      { label: '签到', value: 'signin' }
    ])
  },
  {
    url: '/admin/logs/type/admin',
    method: 'get',
    handler: () => ok([
      { label: '登录', value: 'login' },
      { label: '用户管理', value: 'user' },
      { label: '订单管理', value: 'order' },
      { label: '系统设置', value: 'system' }
    ])
  },
  {
    url: '/admin/logs/del',
    method: 'post',
    handler: ({ data, isDemoMode }) => {
      if (isDemoMode) return forbidden()
      store.delete('logs', data?.id)
      return ok({ success: true })
    }
  },
  {
    url: '/admin/logs/clean',
    method: 'post',
    handler: ({ isDemoMode }) => {
      if (isDemoMode) return forbidden()
      return ok({ success: true })
    }
  },

  // ===== 公告管理 =====
  {
    url: '/admin/notice/list',
    method: 'post',
    handler: ({ data }) => {
      const page = parseInt(data?.page || 1)
      const pageSize = parseInt(data?.page_size || 20)
      const result = store.list('notices', null, page, pageSize)
      return page(result.data, result.total, page, pageSize)
    }
  },
  {
    url: '/admin/notice/add',
    method: 'post',
    handler: ({ data, isDemoMode }) => {
      if (isDemoMode) return forbidden()
      return ok(store.add('notices', data))
    }
  },
  {
    url: '/admin/notice/edit',
    method: 'post',
    handler: ({ data, isDemoMode }) => {
      if (isDemoMode) return forbidden()
      const item = store.update('notices', data?.id, data)
      return item ? ok(item) : fail('公告不存在')
    }
  },
  {
    url: '/admin/notice/del',
    method: 'post',
    handler: ({ data, isDemoMode }) => {
      if (isDemoMode) return forbidden()
      store.delete('notices', data?.id)
      return ok({ success: true })
    }
  },

  // ===== 留言管理 =====
  {
    url: '/admin/message/list',
    method: 'post',
    handler: ({ data }) => {
      const page = parseInt(data?.page || 1)
      const pageSize = parseInt(data?.page_size || 20)
      const result = store.list('messages', null, page, pageSize)
      return page(result.data, result.total, page, pageSize)
    }
  },
  {
    url: '/admin/message/edit',
    method: 'post',
    handler: ({ data, isDemoMode }) => {
      if (isDemoMode) return forbidden()
      const item = store.update('messages', data?.id, data)
      return item ? ok(item) : fail('留言不存在')
    }
  },
  {
    url: '/admin/message/del',
    method: 'post',
    handler: ({ data, isDemoMode }) => {
      if (isDemoMode) return forbidden()
      store.delete('messages', data?.id)
      return ok({ success: true })
    }
  },

  // ===== 积分事件 =====
  {
    url: '/admin/fenEvent/list',
    method: 'post',
    handler: ({ data }) => {
      const page = parseInt(data?.page || 1)
      const pageSize = parseInt(data?.page_size || 20)
      const result = store.list('fenEvents', null, page, pageSize)
      return page(result.data, result.total, page, pageSize)
    }
  },
  {
    url: '/admin/fenEvent/add',
    method: 'post',
    handler: ({ data, isDemoMode }) => {
      if (isDemoMode) return forbidden()
      return ok(store.add('fenEvents', data))
    }
  },
  {
    url: '/admin/fenEvent/edit',
    method: 'post',
    handler: ({ data, isDemoMode }) => {
      if (isDemoMode) return forbidden()
      const item = store.update('fenEvents', data?.id, data)
      return item ? ok(item) : fail('事件不存在')
    }
  },
  {
    url: '/admin/fenEvent/editState',
    method: 'post',
    handler: ({ data, isDemoMode }) => {
      if (isDemoMode) return forbidden()
      const item = store.update('fenEvents', data?.id, { status: data?.status })
      return item ? ok(item) : fail('事件不存在')
    }
  },
  {
    url: '/admin/fenEvent/del',
    method: 'post',
    handler: ({ data, isDemoMode }) => {
      if (isDemoMode) return forbidden()
      store.delete('fenEvents', data?.id)
      return ok({ success: true })
    }
  },

  // ===== 积分订单 =====
  {
    url: '/admin/fenOrder/list',
    method: 'post',
    handler: ({ data }) => {
      const page = parseInt(data?.page || 1)
      const pageSize = parseInt(data?.page_size || 20)
      const result = store.list('fenOrders', null, page, pageSize)
      return page(result.data, result.total, page, pageSize)
    }
  },
  {
    url: '/admin/fenOrder/del',
    method: 'post',
    handler: ({ data, isDemoMode }) => {
      if (isDemoMode) return forbidden()
      store.delete('fenOrders', data?.id)
      return ok({ success: true })
    }
  },

  // ===== 扩展字段 =====
  {
    url: '/admin/extend/list',
    method: 'post',
    handler: ({ data }) => {
      const page = parseInt(data?.page || 1)
      const pageSize = parseInt(data?.page_size || 20)
      const result = store.list('extends', null, page, pageSize)
      return page(result.data, result.total, page, pageSize)
    }
  },
  {
    url: '/admin/extend/add',
    method: 'post',
    handler: ({ data, isDemoMode }) => {
      if (isDemoMode) return forbidden()
      return ok(store.add('extends', data))
    }
  },
  {
    url: '/admin/extend/edit',
    method: 'post',
    handler: ({ data, isDemoMode }) => {
      if (isDemoMode) return forbidden()
      const item = store.update('extends', data?.id, data)
      return item ? ok(item) : fail('字段不存在')
    }
  },
  {
    url: '/admin/extend/del',
    method: 'post',
    handler: ({ data, isDemoMode }) => {
      if (isDemoMode) return forbidden()
      store.delete('extends', data?.id)
      return ok({ success: true })
    }
  },

  // ===== 系统设置 =====
  {
    url: '/admin/system/getSet',
    method: 'post',
    handler: () => ok({
      site_name: 'U验证系统',
      site_logo: '',
      qq: '123456789',
      wechat: 'wechat_id',
      allow_register: true,
      register_audit: false,
      device_limit: 1,
      safe_mode: false,
      signin_reward: 10,
      expire_notice: 7
    })
  },
  {
    url: '/admin/system/editSet',
    method: 'post',
    handler: ({ data, isDemoMode }) => {
      if (isDemoMode) return forbidden()
      return ok({ success: true })
    }
  },
  {
    url: '/admin/system/clearCache',
    method: 'post',
    handler: () => ok({ success: true })
  },
  {
    url: '/admin/system/getUserApiRouter',
    method: 'post',
    handler: () => ok([])
  },
  {
    url: '/admin/system/editUserApiRouter',
    method: 'post',
    handler: ({ isDemoMode }) => {
      if (isDemoMode) return forbidden()
      return ok({ success: true })
    }
  },
  {
    url: '/admin/system/switchUserApiRouter',
    method: 'post',
    handler: ({ isDemoMode }) => {
      if (isDemoMode) return forbidden()
      return ok({ success: true })
    }
  },
  {
    url: '/admin/system/getUserApiCode',
    method: 'post',
    handler: () => ok([])
  },
  {
    url: '/admin/system/editUserApiCode',
    method: 'post',
    handler: ({ isDemoMode }) => {
      if (isDemoMode) return forbidden()
      return ok({ success: true })
    }
  },

  // ===== 管理员管理 =====
  {
    url: '/admin/admList/list',
    method: 'post',
    handler: ({ data }) => {
      const page = parseInt(data?.page || 1)
      const pageSize = parseInt(data?.page_size || 20)
      const result = store.list('adminList', null, page, pageSize)
      return page(result.data, result.total, page, pageSize)
    }
  },
  {
    url: '/admin/admList/add',
    method: 'post',
    handler: ({ data, isDemoMode }) => {
      if (isDemoMode) return forbidden()
      return ok(store.add('adminList', data))
    }
  },
  {
    url: '/admin/admList/edit',
    method: 'post',
    handler: ({ data, isDemoMode }) => {
      if (isDemoMode) return forbidden()
      const item = store.update('adminList', data?.id, data)
      return item ? ok(item) : fail('管理员不存在')
    }
  },
  {
    url: '/admin/admList/del',
    method: 'post',
    handler: ({ data, isDemoMode }) => {
      if (isDemoMode) return forbidden()
      store.delete('adminList', data?.id)
      return ok({ success: true })
    }
  },

  // ===== 发送配置 =====
  {
    url: '/admin/send',
    method: 'get',
    handler: () => ok({
      email: { enabled: false, host: '', port: 25, user: '', pass: '' },
      sms: { enabled: false, accessKeyId: '', accessKeySecret: '', signName: '', templateCode: '' }
    })
  },
  {
    url: '/admin/send/edit',
    method: 'post',
    handler: ({ isDemoMode }) => {
      if (isDemoMode) return forbidden()
      return ok({ success: true })
    }
  },

  // ===== 更新日志 =====
  {
    url: '/admin/uplog',
    method: 'get',
    handler: () => ok([
      { ver: 3.3, revision: '19', type: 'official', content: '<ol><li>修复微信APP支付返回参数不完整</li><li>修复邮箱/手机号找回密码非注册用户不发验证码</li><li>修复统计成交率不准确</li><li>新增用户模糊搜索模式</li><li>修复用户昵称搜索无结果</li><li>禁止后台路径修改为纯数字，避免报错访问应用有误</li><li>优化批量创建卡密导出校验</li><li>卡密长度最短缩短至8位</li><li>后台删除绑定设备后立即退出已登录的token</li><li>安全优化</li><li>代理端卡密版充值卡密优化</li></ol>', time: 1765611627 },
      { ver: 3.3, revision: '18', type: 'official', content: '<p>1.修复代理下级在线支付后上级代理无返利</p><p>2.在线支付新增token下单字段</p><p>3.云函数支持免登录调用</p><p>4.修复专业版代理面板重复加载问题</p><p>5.修复未读留言标记无法消除</p><p>6.修复代理面板订单页面收益错误统计显示</p><p>7.优化细节若干</p>', time: 1764116803 },
      { ver: 3.3, revision: '17', type: 'official', content: '<ol><li>修复管理后台和代理后台机器码过长导致解绑按钮消失</li><li>修复卡密按管理员搜索无效</li><li>修复微信openid有误</li><li>修正统计错误标识</li><li>修复微信和QQ绑定时昵称和头像未正确写入</li><li>修复禁用代理账号后代理还能正常登录</li><li>优化代理后台和管理员后台在同一个浏览器内只能同时登录一个后台</li></ol>', time: 1760367477 },
      { ver: 3.3, revision: '16', type: 'official', content: '<ol><li>管理员/代理端新增安全登录模式</li><li>修复json提交时time字段必须是字符串</li><li>修复高并发时redis缓存出现数据混乱</li><li>修复微信登录access_token不规范</li><li>调用info接口强制刷新最新用户信息</li><li>后台统计优化</li><li>新增未读留言标记</li><li>卡密版新增清理卡密功能</li><li>卡密版新增卡密分组、过期状态筛选</li><li>修复代理端充值提示未开通产品</li></ol>', time: 1759047366 },
      { ver: 3.3, revision: '15', type: 'official', content: '<ol><li>卡密版代理端搜索卡密有问题</li><li>修复QQSDK登录报错access_token不规范</li><li>新增允许自动解绑离线设备开关</li><li>修复订单列表页面数据统计小数点过长</li><li>专业版自动加载代理面板</li><li>自定义代理面板充值金额</li><li>修复积分事件缓存错误空</li><li>修复QQ/微信SDK登录access_token长度校验有误</li><li>新增App支付</li><li>修复api路由重命名后无法清空</li><li>优化在线支付接口</li><li>新增防爆破登录开关</li></ol>', time: 1757425456 },
    ])
  },
]

// 生成卡密码
function generateCdkCode() {
  const chars = 'ABCDEFGHJKLMNPQRSTUVWXYZ23456789'
  let code = ''
  for (let i = 0; i < 16; i++) {
    if (i > 0 && i % 4 === 0) code += '-'
    code += chars.charAt(Math.floor(Math.random() * chars.length))
  }
  return code
}

export default { allMocks, initMockData }

// 导出菜单数据供路由使用
export { menuSeed, buildMenuTree }