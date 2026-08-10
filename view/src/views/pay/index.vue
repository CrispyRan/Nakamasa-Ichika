<template>
  <div class="pay-config">
    <a-spin :loading="loading" class="w-full">
      <!-- 应用选择提示 -->
      <a-alert v-if="!appid" type="warning" class="mb-4">
        <template #title>请先在顶部选择要配置的应用</template>
      </a-alert>

      <!-- 批量操作栏 -->
      <div v-if="appid && dirtyCount > 0" class="batch-bar">
        <span class="batch-info">共 {{ dirtyCount }} 个通道有未保存的修改</span>
        <a-button type="primary" size="small" :loading="batchSaving" @click="saveAll">
          全部保存
        </a-button>
        <a-button size="small" @click="resetAll">重置</a-button>
      </div>

      <!-- 支付通道卡片 -->
      <a-row :gutter="[16, 16]">
        <a-col v-for="ch in channels" :key="ch.id" :xs="24" :sm="24" :md="12" :lg="12">
          <a-card :bordered="false" class="pay-card" :class="{ 'is-dirty': ch._dirty }">
            <template #title>
              <div class="card-header">
                <div class="card-title-row">
                  <span class="card-label">{{ ch.label }}</span>
                  <a-tag v-if="ch._configured" color="green" size="small">已配置</a-tag>
                  <a-tag v-else color="gray" size="small">未配置</a-tag>
                  <span v-if="ch._dirty" class="dirty-dot" />
                </div>
                <a-switch
                  v-model="ch.state"
                  checked-value="on"
                  unchecked-value="off"
                  size="small"
                  @change="onStateChange(ch)"
                >
                  <template #checked>启用</template>
                  <template #unchecked>关闭</template>
                </a-switch>
              </div>
            </template>

            <!-- 启用时显示配置表单 -->
            <div v-if="ch.state === 'on'" class="config-body">
              <a-form layout="vertical">
                <a-form-item label="支付引擎">
                  <a-select
                    v-model="ch.type"
                    placeholder="请选择支付引擎"
                    @change="(val) => onEngineChange(ch, val)"
                  >
                    <a-option v-for="p in availablePlugins(ch.id)" :key="p.id" :value="p.id">
                      {{ p.name }}
                    </a-option>
                  </a-select>
                  <div v-if="currentPlugin(ch)?.extra" class="field-extra">
                    {{ currentPlugin(ch)?.extra }}
                  </div>
                </a-form-item>

                <template v-if="currentPluginFormFields(ch).length > 0">
                  <template v-for="field in currentPluginFormFields(ch)" :key="field.key">
                    <a-form-item v-if="field.config.type === 'select'" :label="field.config.name">
                      <a-select
                        v-model="ch.config[field.key]"
                        :placeholder="field.config.placeholder"
                        :multiple="field.config.multiple"
                        allow-clear
                      >
                        <a-option
                          v-for="(label, value) in field.config.option"
                          :key="value"
                          :value="value"
                        >
                          {{ label }}
                        </a-option>
                      </a-select>
                      <div v-if="field.config.extra" class="field-extra">
                        {{ field.config.extra }}
                      </div>
                    </a-form-item>

                    <a-form-item v-else-if="field.config.type === 'textarea'" :label="field.config.name">
                      <a-textarea
                        v-model="ch.config[field.key]"
                        :placeholder="field.config.placeholder"
                        :auto-size="{ minRows: 2, maxRows: 4 }"
                      />
                    </a-form-item>

                    <a-form-item v-else :label="field.config.name">
                      <a-input
                        v-model="ch.config[field.key]"
                        :placeholder="field.config.placeholder"
                      />
                      <div v-if="field.config.extra" class="field-extra">
                        {{ field.config.extra }}
                      </div>
                    </a-form-item>
                  </template>
                </template>

                <div v-else class="no-fields-tip">该引擎无需额外配置</div>

                <a-form-item>
                  <a-button
                    type="primary"
                    :loading="ch._saving"
                    :disabled="!ch._dirty"
                    long
                    @click="saveChannel(ch)"
                  >
                    {{ ch._dirty ? '保存更改' : '已是最新' }}
                  </a-button>
                </a-form-item>
              </a-form>
            </div>

            <!-- 关闭状态 -->
            <div v-else class="disabled-body">
              <icon-credit-card :size="40" style="color: var(--color-text-4)" />
              <div class="disabled-text">支付通道已关闭，开启后可配置</div>
            </div>
          </a-card>
        </a-col>
      </a-row>

      <!-- 支付引擎说明（折叠） -->
      <a-collapse :default-active-key="[]" class="mt-4 engine-info" :bordered="false">
        <a-collapse-item key="plugins" header="支付引擎说明">
          <div class="plugin-list">
            <div v-for="plugin in plugins" :key="plugin.id" class="plugin-item">
              <div class="plugin-name">{{ plugin.name }}</div>
              <div v-if="plugin.extra" class="plugin-extra">{{ plugin.extra }}</div>
              <div class="plugin-fields">
                <a-tag
                  v-for="field in getFormFields(plugin.form)"
                  :key="field.key"
                  color="arcoblue"
                  size="small"
                >
                  {{ field.config.name }}
                </a-tag>
              </div>
            </div>
          </div>
        </a-collapse-item>
      </a-collapse>
    </a-spin>
  </div>
</template>

<script setup>
import { ref, reactive, computed, onMounted } from 'vue'
import { Message, Modal } from '@arco-design/web-vue'
import payApi from '@/api/system/pay'
import tool from '@/utils/tool'

const loading = ref(false)
const batchSaving = ref(false)
const appid = computed(() => tool.local.get('currentAppId'))

const plugins = ref([])
const channels = reactive([])

/** 脏通道数量 */
const dirtyCount = computed(() => channels.filter(c => c._dirty).length)

/**
 * 获取当前通道可用的支付引擎
 * 引擎 type: "all" = 通用, "ali" = 仅支付宝, "wx" = 仅微信
 */
const availablePlugins = (channelId) => {
  return plugins.value.filter(p => p.type === 'all' || p.type === channelId)
}

/** 获取当前通道选中的引擎对象 */
const currentPlugin = (ch) => {
  return plugins.value.find(p => p.id === ch.type)
}

/** 将 form 对象转为数组 */
const getFormFields = (form) => {
  if (!form || typeof form !== 'object') return []
  return Object.entries(form).map(([key, config]) => ({ key, config }))
}

/** 获取当前通道选中引擎的表单字段 */
const currentPluginFormFields = (ch) => {
  return getFormFields(currentPlugin(ch)?.form)
}

/** 初始化配置对象：为每个表单字段设默认值 */
const initConfig = (form) => {
  const config = {}
  if (form && typeof form === 'object') {
    Object.keys(form).forEach(key => {
      config[key] = form[key].type === 'select' && form[key].multiple ? [] : ''
    })
  }
  return config
}

/** 标记通道为脏 */
const markDirty = (ch) => {
  ch._dirty = true
}

/** 切换状态时标记脏 */
const onStateChange = (ch) => {
  markDirty(ch)
}

/** 切换引擎时重置配置 */
const onEngineChange = (ch, newType) => {
  const plugin = plugins.value.find(p => p.id === newType)
  if (!plugin) return

  // 如果当前有未保存修改，询问确认
  if (ch._dirty) {
    Modal.confirm({
      title: '切换支付引擎',
      content: '切换引擎将重置配置表单，未保存的更改将丢失，是否继续？',
      okText: '继续切换',
      cancelText: '取消',
      onOk: () => doSwitchEngine(ch, plugin)
    })
  } else {
    doSwitchEngine(ch, plugin)
  }
}

/** 实际执行引擎切换 */
const doSwitchEngine = (ch, plugin) => {
  ch.type = plugin.id
  ch.config = initConfig(plugin.form)
  ch._dirty = true
}

/** 检查通道是否已配置（有有效配置值） */
const isConfigured = (ch) => {
  if (ch.state !== 'on') return false
  const form = currentPlugin(ch)?.form
  if (!form) return false
  return Object.keys(form).some(key => {
    const val = ch.config[key]
    return val !== '' && val !== null && val !== undefined &&
      !(Array.isArray(val) && val.length === 0)
  })
}

/** 加载配置 */
const loadConfig = async () => {
  loading.value = true
  try {
    const res = await payApi.getInfo()
    if (res.code === 200 && res.data) {
      plugins.value = res.data.plugins || []

      channels.splice(0, channels.length)
      const raw = res.data.channels || []
      raw.forEach(item => {
        channels.push({
          id: item.id,
          label: item.label,
          icon: item.icon || '',
          state: item.state || 'off',
          type: item.type || '',
          config: item.config || {},
          // 内部状态
          _saving: false,
          _dirty: false,
          _configured: false
        })
      })
      // 计算已配置状态
      channels.forEach(c => { c._configured = isConfigured(c) })
    }
  } catch (e) {
    console.error('加载配置失败', e)
  } finally {
    loading.value = false
  }
}

/** 保存单个通道 */
const saveChannel = async (ch) => {
  if (!appid.value) {
    Message.warning('请先选择应用')
    return
  }

  ch._saving = true
  try {
    const res = await payApi.edit({
      id: parseInt(appid.value),
      channels: [{
        id: ch.id,
        state: ch.state,
        type: ch.type,
        config: ch.config
      }]
    })
    if (res.code === 200) {
      ch._dirty = false
      ch._configured = isConfigured(ch)
      Message.success(`${ch.label} 已保存`)
    } else {
      Message.error(res.msg || '保存失败')
    }
  } catch (e) {
    Message.error('保存失败，请检查网络')
  } finally {
    ch._saving = false
  }
}

/** 保存所有脏通道 */
const saveAll = async () => {
  if (!appid.value) {
    Message.warning('请先选择应用')
    return
  }

  const dirtyList = channels.filter(c => c._dirty)
  if (dirtyList.length === 0) {
    Message.info('没有需要保存的更改')
    return
  }

  batchSaving.value = true
  let ok = 0, fail = 0

  for (const ch of dirtyList) {
    try {
      const res = await payApi.edit({
        id: parseInt(appid.value),
        channels: [{
          id: ch.id,
          state: ch.state,
          type: ch.type,
          config: ch.config
        }]
      })
      if (res.code === 200) {
        ch._dirty = false
        ch._configured = isConfigured(ch)
        ok++
      } else {
        fail++
      }
    } catch (e) {
      fail++
    }
  }

  if (fail === 0) {
    Message.success(`全部保存成功（${ok} 个通道）`)
  } else {
    Message.warning(`保存完成：成功 ${ok}，失败 ${fail}`)
  }
  batchSaving.value = false
}

/** 重置所有脏通道到初始状态（重新加载） */
const resetAll = () => {
  Modal.confirm({
    title: '重置更改',
    content: '将放弃所有未保存的修改，重新加载配置，是否继续？',
    okText: '确认重置',
    cancelText: '取消',
    onOk: loadConfig
  })
}

onMounted(() => {
  loadConfig()
})
</script>

<script>
export default { name: 'PayConfig' }
</script>

<style scoped>
/* ========== 布局 ========== */
.pay-config {
  padding: 20px;
  min-height: 100%;
}
.mb-4 { margin-bottom: 16px; }
.mt-4 { margin-top: 16px; }

/* ========== 批量操作栏 ========== */
.batch-bar {
  display: flex;
  align-items: center;
  gap: 12px;
  padding: 10px 16px;
  margin-bottom: 16px;
  background: rgba(var(--primary-6), 0.06);
  border: 1px solid rgba(var(--primary-6), 0.15);
  border-radius: 10px;
  flex-wrap: wrap;
}
.batch-info {
  font-size: 13px;
  color: var(--color-text-2);
  flex: 1;
}

/* ========== 卡片 ========== */
.pay-card {
  border-radius: 16px;
  background: var(--color-bg-2);
  border: 1px solid var(--color-border-2);
  transition: box-shadow 0.2s, border-color 0.2s;
}
.pay-card:hover {
  box-shadow: 0 4px 20px rgba(0, 0, 0, 0.06);
}
.pay-card.is-dirty {
  border-color: rgb(var(--warning-6));
  box-shadow: 0 0 0 1px rgba(var(--warning-6), 0.15);
}

.pay-card :deep(.arco-card-header) {
  padding: 14px 18px;
  border-bottom: 1px solid var(--color-border-2);
}
.pay-card :deep(.arco-card-body) {
  padding: 18px;
}
.pay-card :deep(.arco-card-header-title) {
  flex: 1;
  overflow: visible;
}

/* ========== 卡片标题 ========== */
.card-header {
  display: flex;
  align-items: center;
  justify-content: space-between;
  width: 100%;
  gap: 8px;
}
.card-title-row {
  display: flex;
  align-items: center;
  gap: 6px;
  min-width: 0;
}
.card-label {
  font-weight: 600;
  font-size: 15px;
  white-space: nowrap;
}

/* 脏状态红点 */
.dirty-dot {
  width: 7px;
  height: 7px;
  border-radius: 50%;
  background: rgb(var(--warning-6));
  flex-shrink: 0;
}

/* ========== 配置表单 ========== */
.config-body {
  padding-top: 4px;
}
.field-extra {
  font-size: 12px;
  color: var(--color-text-3);
  margin-top: 4px;
  line-height: 1.4;
}
.no-fields-tip {
  text-align: center;
  color: var(--color-text-3);
  font-size: 13px;
  padding: 12px 0;
}

/* ========== 关闭状态 ========== */
.disabled-body {
  display: flex;
  flex-direction: column;
  align-items: center;
  gap: 8px;
  padding: 24px 0;
}
.disabled-text {
  font-size: 13px;
  color: var(--color-text-3);
}

/* ========== 引擎说明折叠面板 ========== */
.engine-info :deep(.arco-collapse-item-header) {
  font-weight: 600;
  font-size: 14px;
}
.plugin-list {
  display: grid;
  grid-template-columns: repeat(auto-fit, minmax(260px, 1fr));
  gap: 12px;
}
.plugin-item {
  padding: 12px 14px;
  background: rgba(var(--primary-6), 0.04);
  border-radius: 10px;
  border: 1px solid rgba(var(--primary-6), 0.08);
}
.plugin-name {
  font-weight: 600;
  font-size: 14px;
  color: var(--color-text-1);
  margin-bottom: 4px;
}
.plugin-extra {
  font-size: 12px;
  color: var(--color-text-3);
  margin-bottom: 8px;
  word-break: break-all;
  line-height: 1.4;
}
.plugin-fields {
  display: flex;
  flex-wrap: wrap;
  gap: 4px;
}

/* ========== 移动端适配 ========== */
@media screen and (max-width: 768px) {
  .pay-config {
    padding: 12px;
  }

  .pay-card {
    border-radius: 12px;
  }
  .pay-card :deep(.arco-card-header) {
    padding: 12px 14px;
  }
  .pay-card :deep(.arco-card-body) {
    padding: 14px;
  }

  .card-label {
    font-size: 14px;
  }

  .batch-bar {
    padding: 8px 12px;
    gap: 8px;
  }
  .batch-info {
    font-size: 12px;
    width: 100%;
  }

  .plugin-list {
    grid-template-columns: 1fr;
    gap: 10px;
  }
}
</style>