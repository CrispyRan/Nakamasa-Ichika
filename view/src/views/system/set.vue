<template>
  <div class="system-settings">
    <a-row :gutter="16">
      <!-- 基础设置 -->
      <a-col :span="12">
        <a-card title="基础设置" class="mb-4" :bordered="false">
          <a-form :model="basicForm" layout="vertical">
            <a-form-item label="网站名称">
              <a-input v-model="basicForm.site_name" placeholder="请输入网站名称" />
            </a-form-item>
            <a-form-item label="网站Logo">
              <a-input v-model="basicForm.site_logo" placeholder="Logo URL" />
            </a-form-item>
            <a-form-item label="客服QQ">
              <a-input v-model="basicForm.qq" placeholder="请输入客服QQ" />
            </a-form-item>
            <a-form-item label="客服微信">
              <a-input v-model="basicForm.wechat" placeholder="请输入客服微信" />
            </a-form-item>
            <a-form-item>
              <a-button type="primary" @click="saveBasic">保存设置</a-button>
            </a-form-item>
          </a-form>
        </a-card>

        <!-- 登录设置 -->
        <a-card title="登录设置" :bordered="false">
          <a-form :model="loginForm" layout="vertical">
            <a-form-item label="允许注册">
              <a-switch v-model="loginForm.allow_register" />
            </a-form-item>
            <a-form-item label="注册需审核">
              <a-switch v-model="loginForm.register_audit" />
            </a-form-item>
            <a-form-item label="设备绑定数量">
              <a-input-number v-model="loginForm.device_limit" :min="0" style="width: 100%" />
            </a-form-item>
            <a-form-item label="安全登录模式">
              <a-switch v-model="loginForm.safe_mode" />
            </a-form-item>
            <a-form-item>
              <a-button type="primary" @click="saveLogin">保存设置</a-button>
            </a-form-item>
          </a-form>
        </a-card>
      </a-col>

      <!-- VIP设置 -->
      <a-col :span="12">
        <a-card title="VIP设置" class="mb-4" :bordered="false">
          <a-form :model="vipForm" layout="vertical">
            <a-form-item label="签到奖励积分">
              <a-input-number v-model="vipForm.signin_reward" :min="0" style="width: 100%" />
            </a-form-item>
            <a-form-item label="VIP到期提醒(天)">
              <a-input-number v-model="vipForm.expire_notice" :min="0" style="width: 100%" />
            </a-form-item>
            <a-form-item>
              <a-button type="primary" @click="saveVip">保存设置</a-button>
            </a-form-item>
          </a-form>
        </a-card>

        <!-- 缓存管理 -->
        <a-card title="系统缓存" :bordered="false">
          <a-space direction="vertical" fill>
            <div class="flex justify-between items-center">
              <span>数据缓存</span>
              <a-button size="small" @click="clearCache">清理缓存</a-button>
            </div>
            <div class="flex justify-between items-center">
              <span>模板缓存</span>
              <a-button size="small" @click="clearCache">清理缓存</a-button>
            </div>
            <div class="flex justify-between items-center">
              <span>日志缓存</span>
              <a-button size="small" status="danger" @click="clearCache">清理全部</a-button>
            </div>
          </a-space>
        </a-card>
      </a-col>
    </a-row>
  </div>
</template>

<script setup>
import { ref, reactive, onMounted } from 'vue'
import { Message } from '@arco-design/web-vue'
import systemApi from '@/api/system/system'

const basicForm = reactive({ site_name: '', site_logo: '', qq: '', wechat: '' })
const loginForm = reactive({ allow_register: true, register_audit: false, device_limit: 1, safe_mode: false })
const vipForm = reactive({ signin_reward: 10, expire_notice: 7 })

const loadSettings = async () => {
  try {
    const res = await systemApi.getSet()
    if (res.code === 200 && res.data) {
      // 合并设置数据
    }
  } catch (e) {}
}

const saveBasic = async () => {
  try {
    const res = await systemApi.editSet(basicForm)
    if (res.code === 200) Message.success('保存成功')
  } catch (e) { Message.error('保存失败') }
}

const saveLogin = async () => {
  try {
    const res = await systemApi.editSet(loginForm)
    if (res.code === 200) Message.success('保存成功')
  } catch (e) { Message.error('保存失败') }
}

const saveVip = async () => {
  try {
    const res = await systemApi.editSet(vipForm)
    if (res.code === 200) Message.success('保存成功')
  } catch (e) { Message.error('保存失败') }
}

const clearCache = async () => {
  try {
    const res = await systemApi.clearCache()
    if (res.code === 200) Message.success('缓存清理成功')
  } catch (e) { Message.error('清理失败') }
}

onMounted(() => { loadSettings() })
</script>

<script>
export default { name: 'SystemSet' }
</script>

<style scoped>
.system-settings { padding: 16px; }
</style>
