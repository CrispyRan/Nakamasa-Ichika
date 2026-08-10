<template>
  <div>
    <!-- 面包屑 -->
    <div class="w-full mb-5">
      <a-breadcrumb>
        <a-breadcrumb-item>
          <router-link :to="{ name: 'userList' }">用户列表</router-link>
        </a-breadcrumb-item>
        <a-breadcrumb-item>编辑用户</a-breadcrumb-item>
      </a-breadcrumb>
    </div>

    <div class="grid gap-x-5 gap-y-5 grid-cols-1">
      <!-- 用户信息卡片 -->
      <div>
        <div class="bg-[--color-bg-1] p-5 rounded">
          <div class="flex justify-between items-center">
            <div class="grid gap-x-5 gap-y-5 grid-cols-[48px_1fr]">
              <!-- 头像 -->
              <a-avatar :size="48">
                <img v-if="state.form.avatars" :src="state.form.avatars" alt="avatar" />
                <icon-user v-else />
              </a-avatar>
              <!-- 用户信息 -->
              <div class="md:ml-3">
                <div class="flex items-center">
                  <div class="text-base mr-2">{{ state.form.nickname || '匿名用户' }}</div>
                  <div class="flex gap-2">
                    <icon-wechat :class="state.form.open_wx ? 'text-green-600' : 'text-[var(--color-text-3)]'" />
                    <icon-qq :class="state.form.open_qq ? 'text-blue-400' : 'text-[var(--color-text-3)]'" />
                  </div>
                </div>
                <div class="text-[var(--color-text-2)] flex">
                  <div v-if="state.form.acctno">
                    <span>账号：{{ state.form.acctno }}</span>
                    <a-divider direction="vertical" class="max-md:hidden" />
                  </div>
                  <div v-if="state.form.inviter_id" class="hidden md:block">
                    <span>邀请人：{{ state.form.inviter_id }}</span>
                    <a-divider direction="vertical" />
                  </div>
                  <div v-if="state.form.reg_time" class="hidden md:block">
                    <span>注册IP：{{ state.form.reg_ip }}</span>
                    <a-divider direction="vertical" />
                    <span>注册时间：{{ time.toDate(state.form.reg_time) }}</span>
                  </div>
                  <div v-if="state.form.reg_sn" class="hidden md:block">
                    <a-divider direction="vertical" />
                    <span>注册设备：{{ state.form.reg_sn }}</span>
                  </div>
                </div>
              </div>
            </div>
            <!-- 移动端展开按钮 -->
            <a-button @click="toggleExpand" size="mini" class="md:hidden block">
              <template #icon>
                <icon-down v-if="expanded" />
                <icon-right v-else />
              </template>
            </a-button>
          </div>

          <!-- 移动端展开内容 -->
          <div v-if="expanded" class="w-full md:hidden block">
            <hr class="mb-2 mt-5 border-[--color-fill-2]" />
            <a-form :model="state.form" class="py-2 pl-1" auto-label-width>
              <a-form-item label="注册IP">
                <span class="text-[var(--color-text-2)]">{{ state.form.reg_ip }}</span>
              </a-form-item>
              <a-form-item label="注册时间">
                <span class="text-[var(--color-text-2)]">{{ time.toDate(state.form.reg_time) }}</span>
              </a-form-item>
              <a-form-item label="注册设备" class="!mb-0">
                <span :class="state.form.reg_sn ? 'text-[var(--color-text-2)]' : 'text-[var(--color-text-3)]'">
                  {{ state.form.reg_sn || '未绑定' }}
                </span>
              </a-form-item>
            </a-form>
          </div>
        </div>
      </div>

      <!-- 标签页内容 -->
      <div class="bg-[--color-bg-1] p-5 rounded">
        <a-tabs default-active-key="info">
          <!-- 基本信息 -->
          <a-tab-pane key="info" title="基本信息">
            <a-form :model="state.form" class="py-2 pl-1" layout="vertical">
              <div class="grid grid-cols-1 lg:grid-cols-2 gap-x-5">
                <a-form-item label="手机号">
                  <a-input v-model="state.form.phone" placeholder="11位手机号" />
                </a-form-item>
                <a-form-item label="邮箱">
                  <a-input v-model="state.form.email" placeholder="邮箱账号" />
                </a-form-item>
                
                <!-- 会员到期时间 -->
                <a-form-item class="edit-w-full" label="会员到期时间">
                  <a-date-picker
                    v-model="state.form.vip"
                    show-time
                    value-format="timestamp"
                    style="width: 100%"
                    :shortcuts="vipShortcuts"
                    @change="handleDateChange('vip')"
                  >
                    <a-input v-model="timestamp.vip.timestamp" placeholder="请选择日期时间" readonly>
                      <template #suffix>
                        <span class="arco-icon-hover arco-input-icon-hover arco-input-clear-btn" @click.stop="clearDate('vip')">
                          <icon-close />
                        </span>
                      </template>
                    </a-input>
                  </a-date-picker>
                </a-form-item>
                
                <a-form-item label="积分">
                  <a-input-number v-model="state.form.fen" placeholder="0" style="width: 100%" />
                </a-form-item>
                
                <a-form-item label="额外绑定设备数量" tooltip="基于系统设置额外增加可绑定设备数" class="lg:col-span-2">
                  <a-input-number v-model="state.form.sn_max" placeholder="0" style="width: 100%" />
                </a-form-item>
                
                <a-form-item label="密码" class="lg:col-span-2">
                  <a-input v-model="state.form.password" placeholder="空则不修改密码" />
                </a-form-item>
                
                <!-- 禁用用户期限 -->
                <a-form-item class="lg:col-span-2 edit-w-full">
                  <template #label>
                    <div class="flex items-center justify-between w-full">
                      <div>
                        <a-switch v-model="timestamp.ban.status" size="small" @change="handleBanSwitch" />
                        <span class="ml-2">禁用用户期限</span>
                      </div>
                    </div>
                  </template>
                  <a-date-picker
                    v-model="state.form.ban"
                    show-time
                    value-format="timestamp"
                    style="width: 100%"
                    :shortcuts="banShortcuts"
                    :disabled="!timestamp.ban.status"
                    @change="handleDateChange('ban')"
                  >
                    <a-input v-model="timestamp.ban.timestamp" placeholder="请选择日期时间" :disabled="!timestamp.ban.status" readonly>
                      <template #suffix>
                        <span class="arco-icon-hover arco-input-icon-hover arco-input-clear-btn" @click.stop="clearDate('ban')">
                          <icon-close />
                        </span>
                      </template>
                    </a-input>
                  </a-date-picker>
                </a-form-item>
                
                <!-- 禁用原因 -->
                <a-form-item v-if="timestamp.ban.status" label="禁用原因" class="lg:col-span-2">
                  <a-textarea v-model="state.form.ban_msg" placeholder="如：违反用户使用协议，禁用中" allow-clear />
                </a-form-item>
              </div>
              
              <a-button type="primary" :loading="submitLoading" @click="handleSubmit">
                提交
              </a-button>
            </a-form>
          </a-tab-pane>

          <!-- 扩展信息 -->
          <a-tab-pane key="extend" title="扩展信息">
            <table class="min-w-full divide-y divide-[--color-border-1]">
              <thead class="bg-[--color-fill-2] text-[var(--color-text-1)]">
                <tr>
                  <td class="text-left py-2 px-2 w-32">变量名</td>
                  <td class="text-left py-2 px-2">变量值</td>
                  <td class="text-sm font-semibold w-5">
                    <a-button @click="addExtend">
                      <template #icon><icon-plus :size="15" /></template>
                    </a-button>
                  </td>
                </tr>
              </thead>
              <tbody class="divide-y divide-[--color-border-1]">
                <tr v-for="(item, index) in extend.list" :key="index">
                  <td class="text-left py-2">
                    <a-input v-model="item.key" placeholder="如：name" @blur="checkExtendChange" />
                  </td>
                  <td class="text-left py-2">
                    <a-input v-model="item.val" placeholder="如：张三" @blur="checkExtendChange" />
                  </td>
                  <td>
                    <a-button @click="removeExtend(index)">
                      <template #icon><icon-delete /></template>
                    </a-button>
                  </td>
                </tr>
              </tbody>
            </table>
            <a-empty v-if="extend.list.length < 1" description="暂无扩展信息" />
            <a-button v-if="extend.btnVisible" type="primary" :loading="extend.btnLoading" long @click="saveExtend">
              提交保存
            </a-button>
          </a-tab-pane>

          <!-- 绑定设备 -->
          <a-tab-pane key="snlist" title="绑定设备">
            <template v-if="snList.list.length < 1">
              <a-empty description="暂无登录记录" class="lg:mt-10" />
            </template>
            <template v-else>
              <table class="min-w-full divide-y divide-[--color-border-1]">
                <thead class="bg-[--color-fill-2] text-[var(--color-text-1)]">
                  <tr>
                    <td class="text-left py-2 px-2">机器码</td>
                    <td class="w-16 py-2 px-2">时间</td>
                    <td class="w-16 py-2 px-2">操作</td>
                  </tr>
                </thead>
                <tbody class="divide-y divide-[--color-border-1] text-[var(--color-text-1)]">
                  <tr v-for="(item, index) in snList.list" :key="index">
                    <td class="text-left py-2 px-2">{{ item.udid }}</td>
                    <td class="py-2 px-2">{{ time.toDate(item.time) }}</td>
                    <td class="py-2 px-2">
                      <a-popconfirm type="warning" position="tr" @before-ok="unbindSn">
                        <template #content>确认解绑：{{ item.udid }} ？</template>
                        <a-button status="danger" @click="setUnbindTarget(item.udid, index)">删除</a-button>
                      </a-popconfirm>
                    </td>
                  </tr>
                </tbody>
              </table>
            </template>
          </a-tab-pane>

          <!-- 人脸与登录 -->
          <a-tab-pane key="face" title="人脸与登录">
            <a-form :model="state.form" class="py-2 pl-1" layout="vertical">
              <div class="grid grid-cols-1 lg:grid-cols-2 gap-x-5">
                <a-form-item label="人脸注册时间">
                  <span v-if="state.form.face_time" class="text-[var(--color-text-2)]">
                    {{ time.toDate(state.form.face_time) }}
                  </span>
                  <span v-else class="text-[var(--color-text-3)]">未注册</span>
                </a-form-item>
                <a-form-item label="最近登录地">
                  <span v-if="state.form.last_location" class="text-[var(--color-text-2)]">
                    {{ state.form.last_location }}
                  </span>
                  <span v-else class="text-[var(--color-text-3)]">暂无记录</span>
                </a-form-item>
                <a-form-item label="最近登录IP">
                  <span v-if="state.form.last_login_ip" class="text-[var(--color-text-2)]">
                    {{ state.form.last_login_ip }}
                  </span>
                  <span v-else class="text-[var(--color-text-3)]">暂无记录</span>
                </a-form-item>
                <a-form-item label="人脸底图">
                  <div class="flex items-center gap-4 flex-wrap">
                    <template v-if="state.form.face_image">
                      <a-image
                        :src="state.form.face_image"
                        :preview="true"
                        width="96"
                        height="96"
                        :preview-props="{ wrapStyle: { maxWidth: '60vw' } }"
                      />
                    </template>
                    <span v-else class="text-[var(--color-text-3)]">
                      {{
                        state.form.face_time
                          ? '仅存特征向量（未开启 app.face_store_image 保留底图）'
                          : '未注册人脸'
                      }}
                    </span>
                    <div class="flex items-center gap-2">
                      <a-button :loading="faceUploading" @click="pickFaceFile">
                        <template #icon><icon-upload /></template>
                        上传新脸覆盖特征
                      </a-button>
                      <a-popconfirm
                        v-if="state.form.face_time"
                        type="danger"
                        position="tr"
                        @before-ok="handleDelFace"
                      >
                        <template #content>确认删除该用户的人脸信息？删除后用户需重新注册人脸。</template>
                        <a-button status="danger">删除人脸</a-button>
                      </a-popconfirm>
                    </div>
                    <input
                      ref="faceFileInput"
                      type="file"
                      accept="image/*"
                      class="hidden"
                      @change="handleFaceUpload"
                    />
                  </div>
                </a-form-item>
              </div>
            </a-form>
          </a-tab-pane>

          <!-- 用户日志 -->
          <a-tab-pane key="logs" title="用户日志">
            <template v-if="logList.list.length < 1">
              <a-empty description="暂无用户日志" class="lg:mt-10" />
            </template>
            <template v-else>
              <table class="min-w-full divide-y divide-[--color-border-1] text-[var(--color-text-1)]">
                <thead class="bg-[--color-fill-2]">
                  <tr>
                    <td class="text-left w-16 py-2 px-2">类型</td>
                    <td class="text-left py-2 px-2"></td>
                    <td class="w-16 py-2 px-2">IP</td>
                  </tr>
                </thead>
                <tbody class="divide-y divide-[--color-border-1] text-[var(--color-text-1)]">
                  <tr v-for="(item, index) in logList.list" :key="index">
                    <td class="text-left py-2 px-2">
                      <p>{{ item.type }}</p>
                      <span class="text-[--color-text-3] text-[0.75rem]">{{ time.toDate(item.time) }}</span>
                    </td>
                    <td class="py-2 px-2 text-left">
                      <div v-if="item.asset_changes">
                        <span v-for="(val, key) in item.asset_changes" :key="key" class="mr-2">
                          {{ key === 'fen' ? '积分' : key === 'vip' ? 'VIP' : key === 'money' ? '余额' : key }}:{{ val }}
                        </span>
                      </div>
                    </td>
                    <td class="py-2 px-2">
                      <p>{{ item.ip_address }}</p>
                      <span class="text-[--color-text-3] text-[0.75rem]">{{ item.ip }}</span>
                    </td>
                  </tr>
                </tbody>
              </table>
            </template>
          </a-tab-pane>
        </a-tabs>
      </div>
    </div>
  </div>
</template>

<script setup>
import { ref, onMounted } from 'vue'
import { useRouter } from 'vue-router'
import { Message } from '@arco-design/web-vue'
import dayjs from 'dayjs'
import userApi from '@/api/system/userMgmt'
import time from '@/utils/time'

const router = useRouter()

// 响应式状态 - 严格按照静态文件结构
const state = ref({
  form: {
    id: null,
    email: null,
    phone: null,
    acctno: null,
    nickname: null,
    avatars: null,
    password: null,
    inviter_id: 0,
    vip: 0,
    fen: 0,
    extend: null,
    open_wx: null,
    open_qq: null,
    reg_time: null,
    reg_ip: '',
    reg_sn: '',
    sn_list: [],
    sn_max: 0,
    ban: 0,
    ban_msg: null,
    appid: 0,
    face_time: null,
    face_image: null,
    last_location: null,
    last_login_ip: null
  }
})

// 日志列表
const logList = ref({ list: [] })

// 设备列表
const snList = ref({ list: [] })

// 扩展信息
const extend = ref({
  list: [],
  originaldata: null,
  btnVisible: false,
  btnLoading: false
})

// 时间戳显示
const timestamp = ref({
  vip: { timestamp: '' },
  ban: { timestamp: '', status: false }
})

// UI状态
const expanded = ref(false)
const submitLoading = ref(false)

// 解绑目标
const unbindTarget = ref({ index: 0, udid: '' })

// VIP 快捷选项 - 与静态文件一致
const vipShortcuts = [
  { label: '此刻', value: () => dayjs().add(1, 'second').valueOf() },
  { label: '1个月', value: () => dayjs().add(1, 'month').valueOf() },
  { label: '3个月', value: () => dayjs().add(3, 'month').valueOf() },
  { label: '一年', value: () => dayjs().add(1, 'year').valueOf() },
  { label: '永久', value: () => 9999999999000 }
]

// 禁用快捷选项 - 与静态文件一致
const banShortcuts = [
  { label: '此刻', value: () => dayjs().add(1, 'second').valueOf() },
  { label: '一周', value: () => dayjs().add(1, 'week').valueOf() },
  { label: '一月', value: () => dayjs().add(1, 'month').valueOf() },
  { label: '一年', value: () => dayjs().add(1, 'year').valueOf() },
  { label: '永久', value: () => 9999999999000 }
]

// 展开/收起
const toggleExpand = () => {
  expanded.value = !expanded.value
}

// 处理时间变化 - 与静态文件一致
const handleDateChange = (type) => {
  if (type === 'vip') {
    if (state.value.form.vip >= 9999999999000) {
      timestamp.value.vip.timestamp = '9999-99-99 99:99:99'
    } else {
      timestamp.value.vip.timestamp = time.toDate(state.value.form.vip, true)
    }
  } else {
    if (state.value.form.ban >= 9999999999000) {
      timestamp.value.ban.timestamp = '9999-99-99 99:99:99'
    } else {
      timestamp.value.ban.timestamp = time.toDate(state.value.form.ban, true)
    }
  }
}

// 清除时间 - 与静态文件一致
const clearDate = (type) => {
  if (type === 'vip') {
    state.value.form.vip = ''
    timestamp.value.vip.timestamp = ''
  } else {
    state.value.form.ban = ''
    timestamp.value.ban.timestamp = ''
  }
}

// 处理禁用开关 - 与静态文件一致
const handleBanSwitch = (val) => {
  if (!val) {
    timestamp.value.ban.timestamp = ''
    state.value.form.ban = 0
  }
}

// 加载用户数据 - 与静态文件逻辑一致
onMounted(() => {
  userApi.get(Number(router.currentRoute.value.params.uid)).then(res => {
    if (res.code !== 200) {
      Message.error(res.msg)
      return
    }
    
    // 日志列表
    logList.value.list = res.data.log || []
    
    // 填充表单
    state.value.form = res.data.info
    
    // 处理 VIP 时间 - 与静态文件一致
    if (state.value.form.vip) {
      timestamp.value.vip.timestamp = time.toDate(state.value.form.vip, true)
      if (state.value.form.vip >= 9999999999) {
        timestamp.value.vip.timestamp = '9999-99-99 99:99:99'
      }
      state.value.form.vip = state.value.form.vip * 1000 // 转换为毫秒
    }
    
    // 处理禁用状态 - 与静态文件一致
    if (state.value.form.ban && state.value.form.ban > time.get()) {
      timestamp.value.ban.status = true
      timestamp.value.ban.timestamp = time.toDate(state.value.form.ban, true)
      if (state.value.form.ban >= 9999999999) {
        timestamp.value.ban.status = true
        timestamp.value.ban.timestamp = '9999-99-99 99:99:99'
      }
      state.value.form.ban = state.value.form.ban * 1000
    }
    
    // 设备列表
    if (state.value.form.sn_list) {
      snList.value.list = state.value.form.sn_list
    }
    
    // 扩展信息
    if (state.value.form.extend) {
      parseExtend(state.value.form.extend)
    }
    
    // 清空密码
    state.value.form.password = null
  }).catch(e => {
    Message.error('出错了-1：' + e)
  })
})

// 解析扩展信息 - 与静态文件一致
const parseExtend = (extendData) => {
  extend.value.list = Object.entries(extendData).map(([key, val]) => ({ key, val }))
  let json = JSON.stringify(extend.value.list)
  extend.value.originaldata = json === '[]' ? null : json
}

// 添加扩展字段
const addExtend = () => {
  extend.value.list.push({ key: '', val: '' })
  checkExtendChange()
}

// 删除扩展字段
const removeExtend = (index) => {
  extend.value.list.splice(index, 1)
  checkExtendChange()
}

// 检查扩展信息变化 - 与静态文件一致
const checkExtendChange = () => {
  let json = JSON.stringify(extend.value.list)
  json = json === '[]' ? null : json
  extend.value.btnVisible = json !== extend.value.originaldata
}

// 保存扩展信息 - 与静态文件一致
const saveExtend = async () => {
  const extendData = extend.value.list.reduce((acc, { key, val }) => {
    if (key) acc[key] = val
    return acc
  }, {})
  
  extend.value.btnLoading = true
  try {
    const res = await userApi.editExtend(state.value.form.id, extendData)
    extend.value.btnLoading = false
    if (res.code !== 200) {
      Message.error(res.msg)
      return
    }
    Message.success(res.msg || '保存成功')
    parseExtend(extendData)
    checkExtendChange()
  } catch (e) {
    extend.value.btnLoading = false
    Message.error('出错了：' + e)
  }
}

// 设置解绑目标
const setUnbindTarget = (udid, index) => {
  unbindTarget.value.udid = udid
  unbindTarget.value.index = index
}

// 解绑设备 - 与静态文件一致
const unbindSn = async () => {
  try {
    const res = await userApi.unbindSn(state.value.form.id, unbindTarget.value.udid)
    if (res.code !== 200) {
      Message.error(res.msg)
      return false
    }
    Message.success(res.msg || '解绑成功')
    snList.value.list.splice(unbindTarget.value.index, 1)
    state.value.form.sn_list = JSON.stringify(snList.value.list)
    return true
  } catch (e) {
    Message.error('出错了：' + e)
    return false
  }
}

// 删除人脸
const handleDelFace = async () => {
  try {
    const res = await userApi.delFace(state.value.form.id)
    if (res.code !== 200) {
      Message.error(res.msg)
      return false
    }
    Message.success(res.msg || '人脸已清除')
    state.value.form.face_image = null
    state.value.form.face_time = null
    return true
  } catch (e) {
    Message.error('出错了：' + e)
    return false
  }
}

// 人脸文件上传状态与引用
const faceUploading = ref(false)
const faceFileInput = ref(null)

// 触发文件选择
const pickFaceFile = () => {
  faceFileInput.value?.click()
}

// 上传新脸覆盖特征（强制覆盖，不比对旧脸）
const handleFaceUpload = async (e) => {
  const file = e.target.files?.[0]
  if (!file) return
  // 允许执行同文件再次上传
  e.target.value = ''

  faceUploading.value = true
  try {
    const res = await userApi.faceReg(state.value.form.id, file)
    if (res.code !== 200) {
      Message.error(res.msg)
      return
    }
    Message.success(res.msg || '人脸特征已覆盖')
    state.value.form.face_time = Math.floor(Date.now() / 1000)
    state.value.form.face_image = res.data?.faceImage || null
  } catch (err) {
    Message.error('出错了：' + err)
  } finally {
    faceUploading.value = false
  }
}

// 提交表单 - 与静态文件一致
const handleSubmit = async () => {
  submitLoading.value = true
  
  const data = {
    id: state.value.form.id,
    phone: state.value.form.phone,
    email: state.value.form.email,
    vip: null,
    fen: state.value.form.fen,
    password: state.value.form.password,
    sn_max: state.value.form.sn_max,
    ban: null,
    ban_msg: state.value.form.ban_msg
  }
  
  // 处理 VIP 时间 - 与静态文件一致
  if (state.value.form.vip >= 9999999999000) {
    data.vip = 9999999999
  } else if (state.value.form.vip) {
    data.vip = Math.floor(state.value.form.vip / 1000)
  }
  
  // 处理禁用时间 - 与静态文件一致
  if (state.value.form.ban >= 9999999999000) {
    data.ban = 9999999999
  } else if (state.value.form.ban) {
    data.ban = Math.floor(state.value.form.ban / 1000)
  }
  
  try {
    const res = await userApi.edit(data)
    submitLoading.value = false
    if (res.code !== 200) {
      Message.error(res.msg)
      return
    }
    Message.success(res.msg || '修改成功')
  } catch (e) {
    submitLoading.value = false
    Message.error('出错了：' + e)
  }
}
</script>

<script>
export default { name: 'UserEdit' }
</script>

<style scoped>
.edit-w-full {
  width: 100%;
}

.edit-w-full :deep(.arco-form-item-content) {
  width: 100%;
}
</style>
