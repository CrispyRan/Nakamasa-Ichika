<template>
  <div class="reglogin-page">
    <!-- 注册控制 -->
    <a-card class="mb-4" :bordered="false">
      <div class="flex justify-between items-center mb-4">
        <p class="font-semibold text-lg">注册控制</p>
        <a-switch v-model="formData.reg_state" checked-value="on" unchecked-value="off">
          <template #checked>开</template>
          <template #unchecked>关</template>
        </a-switch>
      </div>
      <a-divider />
      
      <a-form v-if="formData.reg_state === 'on'" :model="formData" layout="vertical">
        <a-row :gutter="16">
          <a-col :span="12">
            <a-form-item label="注册方式" tooltip="注册方式为手机号或邮箱时，需在发信控制中进行配置后用户才可以正常注册">
              <a-radio-group v-model="formData.reg_way">
                <a-radio value="phone">手机号</a-radio>
                <a-radio value="email">邮箱</a-radio>
                <a-radio value="wordnum">自定义账号</a-radio>
              </a-radio-group>
            </a-form-item>
          </a-col>
          <a-col :span="12">
            <a-form-item label="邀请人是否必填" tooltip="当邀请人必填时，未填写邀请人ID则无法注册">
              <a-radio-group v-model="formData.reg_is_inviter">
                <a-radio value="y">是</a-radio>
                <a-radio value="n">否</a-radio>
              </a-radio-group>
            </a-form-item>
          </a-col>
        </a-row>
        
        <a-row :gutter="16">
          <a-col :span="12">
            <a-form-item label="机器码注册间隔" tooltip="该参数可防止用户恶意注册，如设置为24小时间隔，那么每台设备24小时内只能注册一个账号">
              <a-input-number v-model="formData.reg_time_sn" :min="0" placeholder="默认值：24">
                <template #append>小时</template>
              </a-input-number>
            </a-form-item>
          </a-col>
          <a-col :span="12">
            <a-form-item label="IP注册间隔" tooltip="该参数可防止用户恶意注册，如设置为24小时间隔，那么每个IP24小时内只能注册一个账号">
              <a-input-number v-model="formData.reg_time_ip" :min="0" placeholder="默认值：24">
                <template #append>小时</template>
              </a-input-number>
            </a-form-item>
          </a-col>
        </a-row>
      </a-form>
      
      <a-form v-else :model="formData" layout="vertical">
        <a-form-item label="关闭注册提示">
          <a-textarea v-model="formData.reg_off_msg" placeholder="关闭注册提示内容，如：软件维护中，暂时关闭注册功能" :auto-size="{ minRows: 2, maxRows: 4 }" />
        </a-form-item>
      </a-form>
    </a-card>

    <!-- 登录控制 -->
    <a-card class="mb-4" :bordered="false">
      <div class="flex justify-between items-center mb-4">
        <p class="font-semibold text-lg">登录控制</p>
        <a-switch v-model="formData.logon_state" checked-value="on" unchecked-value="off">
          <template #checked>开</template>
          <template #unchecked>关</template>
        </a-switch>
      </div>
      <a-divider />
      
      <a-form v-if="formData.logon_state === 'on'" :model="formData" layout="vertical">
        <a-row :gutter="16">
          <a-col :span="12">
            <a-form-item label="Token过期时间" tooltip="用户登录后如需保持在线状态，则需要在过期token过期前使用心跳接口保持在线">
              <a-input-number v-model="formData.logon_token_exp" :min="60" :max="2592000" placeholder="默认值：86400">
                <template #append>秒</template>
              </a-input-number>
            </a-form-item>
          </a-col>
          <a-col :span="12">
            <a-form-item label="防爆登录" tooltip="开启防爆登录开关用户登录失败超过5次,则需等待5分钟后才可以再次登录">
              <a-radio-group v-model="formData.login_prevent_brute_force">
                <a-radio :value="true">开启</a-radio>
                <a-radio :value="false">关闭</a-radio>
              </a-radio-group>
            </a-form-item>
          </a-col>
        </a-row>
        
        <a-row :gutter="16">
          <a-col :span="12">
            <a-form-item label="应用多开" tooltip="如果允许应用多开，那么用户就可以在已绑定的设备上无限制登录数量">
              <a-radio-group v-model="formData.logon_sn_dk" :disabled="formData.logon_sn_num <= 0">
                <a-radio value="y">允许</a-radio>
                <a-radio value="n">不允许</a-radio>
              </a-radio-group>
            </a-form-item>
          </a-col>
          <a-col :span="12">
            <a-form-item label="登录设备数">
              <div class="flex items-center gap-4">
                <a-input-number 
                  v-model="formData.logon_sn_num" 
                  :min="0" 
                  placeholder="默认值：1"
                  @input="handleDeviceNumChange"
                >
                  <template #append>台</template>
                </a-input-number>
                <a-checkbox 
                  v-model="formData.logon_sn_over_ban" 
                  :disabled="formData.logon_sn_num <= 0"
                >
                  禁止超限登录
                </a-checkbox>
              </div>
              <div class="text-xs text-gray-500 mt-1">
                设置0则不限制登录设备，但是只能同时保持一台设备在线
              </div>
            </a-form-item>
          </a-col>
        </a-row>
        
        <a-row :gutter="16">
          <a-col :span="12">
            <a-form-item label="自动解绑离线设备" tooltip="在不同设备上登录时,当已绑定设备上限,则自动解绑离线设备,由新设备顶替登录">
              <a-radio-group v-model="formData.logon_sn_unbde_auto" :disabled="formData.logon_sn_over_ban || formData.logon_sn_num <= 0">
                <a-radio :value="true">允许</a-radio>
                <a-radio :value="false">不允许</a-radio>
              </a-radio-group>
            </a-form-item>
          </a-col>
          <a-col :span="12">
            <a-form-item label="解绑惩罚" tooltip="用户换绑登录设备时可选设置扣除积分或VIP时间作为惩罚">
              <div class="flex gap-2">
                <a-select v-model="formData.logon_sn_unbde_type" style="width: 100px">
                  <a-option value="fen">积分</a-option>
                  <a-option value="vip">会员</a-option>
                </a-select>
                <a-input-number v-model="formData.logon_sn_unbde_val" :min="0" placeholder="0则不惩罚">
                  <template #append>{{ formData.logon_sn_unbde_type === 'fen' ? '积分' : '秒' }}</template>
                </a-input-number>
              </div>
            </a-form-item>
          </a-col>
        </a-row>
      </a-form>
      
      <a-form v-else :model="formData" layout="vertical">
        <a-form-item label="关闭登录提示">
          <a-textarea v-model="formData.logon_off_msg" placeholder="关闭登录提示内容，如：软件维护中，暂时关闭登录功能" :auto-size="{ minRows: 2, maxRows: 4 }" />
        </a-form-item>
      </a-form>
    </a-card>

    <!-- 微信登录控制 -->
    <a-card v-if="formData.logon_state === 'on'" class="mb-4" :bordered="false">
      <div class="flex justify-between items-center mb-4">
        <div>
          <p class="font-semibold text-lg">微信登录控制</p>
          <span class="text-xs text-gray-500">回调域名：{{ callbackDomain }}</span>
        </div>
        <a-switch v-model="formData.logon_open_wxconfig.state" checked-value="on" unchecked-value="off">
          <template #checked>开</template>
          <template #unchecked>关</template>
        </a-switch>
      </div>
      <a-divider />
      
      <a-form v-if="formData.logon_open_wxconfig.state === 'on'" :model="formData" layout="vertical">
        <a-row :gutter="16">
          <a-col :span="12">
            <a-form-item label="微信appID">
              <a-input v-model="formData.logon_open_wxconfig.appID" placeholder="如：wx6462627a7*****" />
            </a-form-item>
          </a-col>
          <a-col :span="12">
            <a-form-item label="微信appSecret">
              <a-input-password v-model="formData.logon_open_wxconfig.appSecret" placeholder="如：a57af4eb64f89fc3d00e9b16*****" />
            </a-form-item>
          </a-col>
        </a-row>
      </a-form>
    </a-card>

    <!-- QQ登录控制 -->
    <a-card v-if="formData.logon_state === 'on'" class="mb-4" :bordered="false">
      <div class="flex justify-between items-center mb-4">
        <div>
          <p class="font-semibold text-lg">QQ登录控制</p>
          <span class="text-xs text-gray-500">网站回调域：{{ callbackDomain }}/api/oauth2.0/qqlogon/callback</span>
        </div>
        <a-switch v-model="formData.logon_open_qqconfig.state" checked-value="on" unchecked-value="off">
          <template #checked>开</template>
          <template #unchecked>关</template>
        </a-switch>
      </div>
      <a-divider />
      
      <a-form v-if="formData.logon_open_qqconfig.state === 'on'" :model="formData" layout="vertical">
        <a-row :gutter="16">
          <a-col :span="12">
            <a-form-item label="QQappID">
              <a-input v-model="formData.logon_open_qqconfig.appID" placeholder="如：102354***" />
            </a-form-item>
          </a-col>
          <a-col :span="12">
            <a-form-item label="QQappKey">
              <a-input-password v-model="formData.logon_open_qqconfig.appKey" placeholder="如：UwGNHySilhU4R***" />
            </a-form-item>
          </a-col>
        </a-row>
      </a-form>
    </a-card>

    <!-- 异地登录安全验证 -->
    <a-card v-if="formData.logon_state === 'on'" class="mb-4" :bordered="false">
      <div class="flex justify-between items-center mb-4">
        <div>
          <p class="font-semibold text-lg">异地登录安全验证</p>
          <span class="text-xs text-gray-500">异地登录（IP 地域变化）时，要求用户通过人脸识别或验证码完成二次验证</span>
        </div>
        <a-switch v-model="formData.face_enable" checked-value="on" unchecked-value="off">
          <template #checked>开</template>
          <template #unchecked>关</template>
        </a-switch>
      </div>
      <a-divider />

      <a-form v-if="formData.face_enable === 'on'" :model="formData" layout="vertical">
        <a-row :gutter="16">
          <a-col :span="12">
            <a-form-item label="人脸识别" tooltip="需在服务端放置人脸模型文件（models/face_detection_yunet_2023mar.onnx 与 arcfaceresnet100-8.onnx）后方可生效。开启后异地登录优先走人脸校验">
              <a-radio-group v-model="faceEngineEnabled" :disabled="true">
                <a-radio :value="true">可用（模型已加载）</a-radio>
                <a-radio :value="false">不可用（缺少模型文件）</a-radio>
              </a-radio-group>
            </a-form-item>
          </a-col>
          <a-col :span="12">
            <a-form-item label="验证码兜底" tooltip="未配置邮件/短信验证码服务，且账号未提交人脸时，异地登录不会锁定账号，仅提醒提交人脸">
              <span class="text-xs text-gray-500">
                {{ hasCodeService ? '已配置（邮件或短信验证码）' : '未配置（仅依赖人脸）' }}
              </span>
            </a-form-item>
          </a-col>
        </a-row>
        <div class="text-xs text-gray-500">
          当账号已提交人脸时，异地登录将要求人脸校验；账号无人脸时，将提醒用户先提交人脸。
          开启人脸识别请务必先在服务端放置人脸模型文件，否则功能不生效。
        </div>
      </a-form>
    </a-card>

    <!-- 提交按钮 -->
    <a-button type="primary" long :loading="loading" @click="handleSubmit">
      提交
    </a-button>
  </div>
</template>

<script setup>
import { ref, reactive, onMounted, computed } from 'vue'
import { Message } from '@arco-design/web-vue'
import appApi from '@/api/system/app'

const loading = ref(false)
const callbackDomain = computed(() => window.location.protocol + '//' + window.location.host)

// 人脸/验证码可用性展示（只读，不参与提交）
const faceEngineEnabled = ref(false)
const codeServiceState = reactive({ smtp_state: 'off', sms_state: 'off' })
const hasCodeService = computed(
  () => codeServiceState.smtp_state === 'on' || codeServiceState.sms_state === 'on'
)

const formData = reactive({
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
  face_enable: 'off',
  logon_open_wxconfig: {
    state: 'on',
    appID: '',
    appSecret: ''
  },
  logon_open_qqconfig: {
    state: 'on',
    appID: '',
    appKey: ''
  }
})

// 加载配置
const loadConfig = async () => {
  try {
    const res = await appApi.getInfo([
      'reg_state', 'reg_off_msg', 'reg_way', 'reg_is_inviter', 'reg_time_sn', 'reg_time_ip',
      'logon_state', 'logon_off_msg', 'logon_token_exp', 'logon_ban_expire', 'logon_sn_dk',
      'login_prevent_brute_force', 'logon_sn_num', 'logon_sn_over_ban', 'logon_sn_unbde_auto',
      'logon_sn_unbde_type', 'logon_sn_unbde_val', 'face_enable',
      'logon_open_wxconfig', 'logon_open_qqconfig',
      'smtp_state', 'sms_state', 'face_available'
    ])
    
    if (res.code === 200) {
      Object.keys(res.data).forEach(key => {
        if (key === 'face_available') {
          faceEngineEnabled.value = res.data[key] === true
        } else if (key === 'smtp_state' || key === 'sms_state') {
          // 只读展示，不参与提交，避免覆盖发信控制的配置
          codeServiceState[key] = res.data[key] || 'off'
        } else if (key === 'logon_open_wxconfig' || key === 'logon_open_qqconfig') {
          if (res.data[key]) {
            Object.assign(formData[key], res.data[key])
          }
        } else {
          formData[key] = res.data[key]
        }
      })
    } else {
      Message.error(res.msg)
    }
  } catch (e) {
    Message.error('加载配置失败：' + e)
  }
}

// 设备数量变化处理
const handleDeviceNumChange = (val) => {
  if (val <= 0) {
    formData.logon_sn_dk = 'n'
    formData.logon_sn_over_ban = false
    formData.logon_sn_unbde_auto = false
  }
}

// 提交
const handleSubmit = async () => {
  if (formData.logon_sn_num < 0) {
    Message.error('登录设备数不可小于0')
    return
  }
  
  loading.value = true
  try {
    const res = await appApi.edit(formData)
    if (res.code === 200) {
      Message.success(res.msg || '保存成功')
    } else {
      Message.error(res.msg)
    }
  } catch (e) {
    Message.error('保存失败：' + e)
  } finally {
    loading.value = false
  }
}

onMounted(() => {
  loadConfig()
})
</script>

<script>
export default { name: 'AppRegLogin' }
</script>

<style scoped>
.reglogin-page {
  padding: 16px;
}
</style>
