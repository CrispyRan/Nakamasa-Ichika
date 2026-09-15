/**
 * 管理员权限分组定义（RBAC）
 *
 * value 必须与后端 admin_auth.rs 的 AUTH_RULES 分组名严格一致，
 * 否则 auth_allows 匹配不上，表现为"给了权限但接口仍 403"。
 *
 * 为什么单独成文件：
 * 这些常量原本是 export 在 store/modules/user.js 里，但 admin/index.vue 需要
 * 引用，而 user.js 自己又 import 了 @/store（barrel），barrel 再 import user.js，
 * 构成循环依赖。循环下 AUTH_GROUPS 在首次求值时是 undefined，
 * admin/index.vue 里 `AUTH_GROUPS.filter(...)` 直接抛错，导致权限配置区不渲染。
 * 抽到独立常量模块（不 import 任何项目内部模块）即可打破该环。
 */

// 全部权限分组，供管理员编辑页勾选使用
export const AUTH_GROUPS = [
  { value: 'adm', label: '管理员管理' },
  { value: 'cdk', label: '卡密管理' },
  { value: 'agent', label: '代理管理' },
  { value: 'finance', label: '积分财务' },
  { value: 'goods', label: '商品管理' },
  { value: 'order', label: '订单管理' },
  { value: 'statistics', label: '数据统计' },
  { value: 'blocklist', label: '黑名单' },
  { value: 'function', label: '云函数' },
  { value: 'system', label: '系统设置' },
  { value: 'upload', label: '上传管理' },
  { value: 'send', label: '发信控制' },
  { value: 'content', label: '公告留言' },
  { value: 'ver', label: '版本管理' },
  { value: 'logs', label: '日志管理' },
  { value: 'user', label: '用户管理' },
  { value: 'app', label: '应用管理' }
]

// 菜单 name → 所需权限组。
// 仪表盘/个人中心/插件市场任何已登录管理员都可见，不在表内即默认放行；
// adm 组不映射菜单：管理员列表只给超管用（后端 get_list 写死 WHERE id > 1），
// 给受限管理员显示反而会出现空表。
export const MENU_AUTH = {
  app: 'app',
  appInfo: 'app',
  appRegLogin: 'app',
  userList: 'user',
  kami: 'cdk',
  kamiList: 'cdk',
  kamiGroup: 'cdk',
  verList: 'ver',
  agentList: 'agent',
  orderList: 'order',
  payConfig: 'finance',
  functionList: 'function',
  encryptionList: 'system',
  blocklistList: 'blocklist',
  logsList: 'logs',
  noticeList: 'content',
  messageList: 'content',
  sendControl: 'send',
  fenIndex: 'finance',
  goodsList: 'goods',
  extendList: 'system',
  system: 'system',
  systemSet: 'system',
  systemRouter: 'system',
  systemCode: 'system',
  statistics: 'statistics',
  statisticsIndex: 'statistics',
  dataAnalysis: 'statistics',
  multiDimensionDataAnalysis: 'statistics'
}

// 判断一组权限码是否包含指定权限组
export function hasAuth(codes, group) {
  if (!Array.isArray(codes) || codes.length === 0) return false
  if (codes.includes('*') || codes.includes('all')) return true
  return codes.includes(group)
}

// 是否为全部权限（null/undefined 视为老账号的全量权限）
export function isFullAuth(auth) {
  if (auth == null) return true
  if (!Array.isArray(auth)) return true
  return auth.some(a => a === '*' || a === 'all')
}
