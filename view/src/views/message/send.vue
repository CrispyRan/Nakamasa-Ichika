<template>
  <div class="send-control">
    <a-spin :loading="pageLoading" style="width: 100%;">
      <a-row :gutter="16">
        <!-- 邮箱发信设置 -->
        <a-col :span="12">
          <a-card :bordered="false" class="mb-4">
            <template #title>
              <div class="card-title-row">
                <span>邮箱发信</span>
                <a-switch 
                  v-model="formData.smtp_state" 
                  checked-value="on" 
                  unchecked-value="off"
                  size="small"
                  @change="onStateChange('smtp')"
                />
              </div>
            </template>
            
            <!-- 开启状态时显示配置表单 -->
            <div v-if="formData.smtp_state === 'on'" class="config-section">
              <a-form :model="formData" layout="vertical">
                <a-form-item label="发信服务器">
                  <a-input v-model="formData.smtp_host" placeholder="如：smtp.qq.com" />
                </a-form-item>
                <a-form-item label="端口">
                  <a-input-number v-model="formData.smtp_port" :min="1" :max="9999" style="width: 100%;" />
                </a-form-item>
                <a-form-item label="发信账号">
                  <a-input v-model="formData.smtp_user" placeholder="发信邮箱账号" />
                </a-form-item>
                <a-form-item label="发信密码">
                  <a-input-password v-model="formData.smtp_pass" placeholder="授权码或密码" />
                </a-form-item>
              </a-form>
            </div>
            
            <!-- 未开启状态 -->
            <div v-else class="disabled-section">
              <a-empty description="邮箱发信未开启" :style="{ padding: '20px 0' }">
                <template #image>
                  <icon-email :size="40" style="color: var(--color-text-3)" />
                </template>
              </a-empty>
            </div>
          </a-card>
        </a-col>

        <!-- 短信发信设置 -->
        <a-col :span="12">
          <a-card :bordered="false" class="mb-4">
            <template #title>
              <div class="card-title-row">
                <span>短信发信</span>
                <a-switch 
                  v-model="formData.sms_state" 
                  checked-value="on" 
                  unchecked-value="off"
                  size="small"
                  @change="onStateChange('sms')"
                />
              </div>
            </template>
            
            <!-- 开启状态时显示配置表单 -->
            <div v-if="formData.sms_state === 'on'" class="config-section">
              <a-form :model="formData" layout="vertical">
                <a-form-item label="短信平台">
                  <a-select v-model="formData.sms_type" @change="onSmsTypeChange">
                    <a-option v-for="p in plugList" :key="p.id" :value="p.id">
                      {{ p.name }}
                    </a-option>
                  </a-select>
                </a-form-item>
                
                <!-- 动态渲染平台配置表单 -->
                <template v-if="currentPlug && currentPlug.form">
                  <a-form-item v-for="(field, key) in currentPlug.form" :key="key" :label="field.name">
                    <a-input 
                      v-model="smsConfig[key]" 
                      :placeholder="field.placeholder"
                    />
                    <div v-if="field.tooltip" class="field-tooltip">
                      <icon-question-circle style="margin-right: 4px;" />
                      {{ field.tooltip }}
                    </div>
                  </a-form-item>
                </template>
                
                <!-- 平台额外信息 -->
                <div v-if="currentPlug && currentPlug.extra" class="plug-extra">
                  {{ currentPlug.extra }}
                </div>
              </a-form>
            </div>
            
            <!-- 未开启状态 -->
            <div v-else class="disabled-section">
              <a-empty description="短信发信未开启" :style="{ padding: '20px 0' }">
                <template #image>
                  <icon-message :size="40" style="color: var(--color-text-3)" />
                </template>
              </a-empty>
            </div>
          </a-card>
        </a-col>
      </a-row>

      <!-- 验证码设置 -->
      <a-card title="验证码设置" :bordered="false" class="mb-4">
        <a-form :model="formData" layout="inline">
          <a-form-item label="验证码长度">
            <a-input-number v-model="formData.vc_length" :min="4" :max="6" style="width: 120px;">
              <template #append>
                <span>位</span>
              </template>
            </a-input-number>
          </a-form-item>
          <a-form-item label="有效时间">
            <a-input-number v-model="formData.vc_time" :min="1" :max="30" style="width: 120px;">
              <template #append>
                <span>分钟</span>
              </template>
            </a-input-number>
          </a-form-item>
          <a-form-item label="发送频率">
            <a-input-number v-model="formData.vc_frequency" :min="60" :max="300" style="width: 120px;" disabled>
              <template #append>
                <span>秒</span>
              </template>
            </a-input-number>
          </a-form-item>
          <a-form-item label="每日上限">
            <a-input-number v-model="formData.vc_maximum" :min="1" :max="20" style="width: 120px;" disabled>
              <template #append>
                <span>条</span>
              </template>
            </a-input-number>
          </a-form-item>
        </a-form>
      </a-card>

      <!-- 保存按钮 -->
      <div class="action-bar">
        <a-button type="primary" :loading="saveLoading" @click="handleSave">
          保存设置
        </a-button>
      </div>
    </a-spin>
  </div>
</template>

<script setup>
import { ref, reactive, computed, onMounted } from 'vue'
import { Message } from '@arco-design/web-vue'
import sendApi from '@/api/system/send'

const pageLoading = ref(false)
const saveLoading = ref(false)
const plugList = ref([])

const formData = reactive({
  id: null,
  smtp_state: 'off',
  smtp_host: '',
  smtp_port: 465,
  smtp_user: '',
  smtp_pass: '',
  sms_state: 'off',
  sms_type: 'jie',
  vc_length: 4,
  vc_time: 10,
  vc_frequency: 120,
  vc_maximum: 10
})

const smsConfig = reactive({})

const currentPlug = computed(() => {
  return plugList.value.find(p => p.id === formData.sms_type)
})

async function loadData() {
  pageLoading.value = true
  try {
    const res = await sendApi.getInfo()
    if (res.code === 200) {
      const info = res.data.info || {}
      plugList.value = res.data.plug || []
      
      // 填充表单数据
      formData.id = info.id
      formData.smtp_state = info.smtp_state || 'off'
      formData.smtp_host = info.smtp_host || ''
      formData.smtp_port = info.smtp_port || 465
      formData.smtp_user = info.smtp_user || ''
      formData.smtp_pass = info.smtp_pass || ''
      formData.sms_state = info.sms_state || 'off'
      formData.sms_type = info.sms_type || 'jie'
      formData.vc_length = info.vc_length || 4
      formData.vc_time = info.vc_time || 10
      formData.vc_frequency = info.vc_frequency || 120
      formData.vc_maximum = info.vc_maximum || 10
      
      // 解析短信配置
      if (info.sms_config) {
        // 如果是对象格式
        if (typeof info.sms_config === 'object' && !Array.isArray(info.sms_config)) {
          Object.assign(smsConfig, info.sms_config)
        }
        // 如果是数组格式 [{key, value}, ...]
        else if (Array.isArray(info.sms_config)) {
          info.sms_config.forEach(item => {
            if (item.key && item.value !== undefined) {
              smsConfig[item.key] = item.value
            }
          })
        }
      }
    }
  } catch (e) {
    Message.error('加载数据失败')
  } finally {
    pageLoading.value = false
  }
}

function onStateChange(type) {
  // 状态切换时的处理
}

function onSmsTypeChange() {
  // 切换平台时清空配置
  Object.keys(smsConfig).forEach(key => delete smsConfig[key])
}

async function handleSave() {
  saveLoading.value = true
  try {
    // 将 smsConfig 转换为数组格式（后端期望数组）
    const configArray = Object.entries(smsConfig).map(([key, value]) => ({
      key,
      value
    }))
    
    const submitData = {
      id: formData.id,
      smtp_state: formData.smtp_state,
      smtp_host: formData.smtp_host || null,
      smtp_port: formData.smtp_port,
      smtp_user: formData.smtp_user || null,
      smtp_pass: formData.smtp_pass || null,
      sms_state: formData.sms_state,
      sms_type: formData.sms_type,
      sms_config: configArray,
      vc_length: formData.vc_length,
      vc_time: formData.vc_time
    }
    
    const res = await sendApi.edit(submitData)
    if (res.code === 200) {
      Message.success('保存成功')
    } else {
      Message.error(res.msg || '保存失败')
    }
  } catch (e) {
    Message.error('保存失败')
  } finally {
    saveLoading.value = false
  }
}

onMounted(() => {
  loadData()
})
</script>

<script>
export default { name: 'SendControl' }
</script>

<style scoped>
.send-control {
  padding: 16px;
}

.mb-4 {
  margin-bottom: 16px;
}

.card-title-row {
  display: flex;
  align-items: center;
  justify-content: space-between;
  width: 100%;
}

.config-section {
  padding-top: 8px;
}

.disabled-section {
  text-align: center;
  padding: 16px 0;
}

.field-tooltip {
  font-size: 12px;
  color: var(--color-text-3);
  margin-top: 4px;
  display: flex;
  align-items: flex-start;
}

.plug-extra {
  font-size: 12px;
  color: var(--color-text-3);
  margin-top: 8px;
  padding: 8px;
  background: var(--color-fill-1);
  border-radius: 4px;
}

.action-bar {
  display: flex;
  justify-content: flex-end;
  padding: 16px 0;
}
</style>