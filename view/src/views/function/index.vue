<template>
  <div class="cloud-function">
    <!-- 操作栏 -->
    <a-card class="mb-4 ma-content-block" :bordered="false">
      <a-space>
        <a-button type="primary" @click="handleAdd">
          <template #icon><icon-plus /></template>
          添加函数
        </a-button>
      </a-space>
    </a-card>

    <!-- 函数列表 -->
    <a-card class="ma-content-block" :bordered="false">
      <a-table :columns="columns" :data="tableData" :loading="loading" :pagination="pagination" row-key="id" @page-change="handlePageChange">
        <template #allow="{ record }">
          <a-tag v-if="record.allow === 0" color="green" size="small">全部用户</a-tag>
          <a-tag v-else-if="record.allow === 1" color="blue" size="small">VIP用户</a-tag>
          <a-tag v-else color="orange" size="small">VIP{{ record.allow }}+</a-tag>
        </template>
        <template #state="{ record }">
          <a-switch
            :model-value="record.state === 'y'"
            size="small"
            @change="handleStateChange(record, $event)"
          />
        </template>
        <template #actions="{ record }">
          <a-space>
            <a-button type="text" size="small" @click="handleEdit(record)">编辑</a-button>
            <a-button type="text" size="small" @click="handleViewCode(record)">查看代码</a-button>
            <a-popconfirm content="确定删除该函数吗？" @ok="handleDelete(record)">
              <a-button type="text" size="small" status="danger">删除</a-button>
            </a-popconfirm>
          </a-space>
        </template>
      </a-table>
    </a-card>

    <!-- 编辑弹窗 -->
    <a-modal v-model:visible="modalVisible" :title="modalTitle" :width="'80%'" :footer="false" unmount-on-close>
      <a-form ref="formRef" :model="form" :rules="rules" layout="vertical">
        <a-row :gutter="16">
          <a-col :span="8">
            <a-form-item field="name" label="函数名称" required>
              <a-input v-model="form.name" placeholder="字母开头，3-64位字母数字" />
            </a-form-item>
          </a-col>
          <a-col :span="8">
            <a-form-item field="allow" label="VIP权限">
              <a-select v-model="form.allow" placeholder="请选择权限要求">
                <a-option :value="0">全部用户</a-option>
                <a-option :value="1">VIP用户</a-option>
                <a-option :value="2">VIP2+</a-option>
                <a-option :value="3">VIP3+</a-option>
              </a-select>
            </a-form-item>
          </a-col>
          <a-col :span="8">
            <a-form-item field="fen" label="积分消耗">
              <a-input-number v-model="form.fen" :min="0" placeholder="消耗积分" style="width: 100%" />
            </a-form-item>
          </a-col>
        </a-row>
        <a-row :gutter="16">
          <a-col :span="12">
            <a-form-item field="state" label="状态">
              <a-radio-group v-model="form.state">
                <a-radio value="y">启用</a-radio>
                <a-radio value="n">禁用</a-radio>
              </a-radio-group>
            </a-form-item>
          </a-col>
          <a-col :span="12">
            <a-form-item field="notes" label="备注说明">
              <a-input v-model="form.notes" placeholder="请输入备注说明" />
            </a-form-item>
          </a-col>
        </a-row>
        <a-form-item field="code" label="函数代码">
          <div class="code-editor-wrapper">
            <div class="editor-toolbar">
              <a-select v-model="editorTheme" size="small" style="width: 120px">
                <a-option value="vs-dark">深色主题</a-option>
                <a-option value="vs-light">浅色主题</a-option>
              </a-select>
              <a-tooltip content="开启/关闭智能提示">
                <a-button size="small" :type="editorSuggestions ? 'primary' : 'secondary'" @click="editorSuggestions = !editorSuggestions">
                  <template #icon><icon-bulb /></template>
                  智能提示
                </a-button>
              </a-tooltip>
              <a-button size="small" @click="insertTemplate">
                <template #icon><icon-file /></template>
                插入模板
              </a-button>
              <a-button size="small" @click="formatCode">
                <template #icon><icon-code /></template>
                格式化
              </a-button>
            </div>
            <ma-code-editor
              ref="editorRef"
              v-model="form.code"
              :height="450"
              language="javascript"
              :theme="editorTheme"
              :suggestions="editorSuggestions"
            />
          </div>
        </a-form-item>
        <div class="form-footer">
          <a-space>
            <a-button @click="handleCancel">取消</a-button>
            <a-button type="primary" :loading="submitLoading" @click="handleSubmit">保存</a-button>
          </a-space>
        </div>
      </a-form>
    </a-modal>

    <!-- 查看代码弹窗 -->
    <a-modal v-model:visible="codeModalVisible" title="函数代码" :width="'70%'" :footer="false" unmount-on-close>
      <div class="code-viewer-wrapper">
        <div class="viewer-toolbar">
          <a-select v-model="viewerTheme" size="small" style="width: 120px">
            <a-option value="vs-dark">深色主题</a-option>
            <a-option value="vs-light">浅色主题</a-option>
          </a-select>
          <a-button size="small" @click="copyCode">
            <template #icon><icon-copy /></template>
            复制代码
          </a-button>
        </div>
        <ma-code-editor
          v-model="currentCode"
          :height="500"
          language="javascript"
          :theme="viewerTheme"
          :readonly="true"
          :suggestions="false"
        />
      </div>
    </a-modal>
  </div>
</template>

<script setup>
import { ref, reactive, computed, onMounted } from 'vue'
import { Message } from '@arco-design/web-vue'
import functionApi from '@/api/system/function'
import MaCodeEditor from '@/components/ma-codeEditor/index.vue'

// 表格数据
const tableData = ref([])
const loading = ref(false)

// 分页
const pagination = reactive({
  current: 1,
  pageSize: 10,
  total: 0,
  showTotal: true
})

// 弹窗
const modalVisible = ref(false)
const modalTitle = computed(() => form.id ? '编辑函数' : '添加函数')
const formRef = ref(null)
const editorRef = ref(null)
const submitLoading = ref(false)
const form = reactive({
  id: null,
  name: '',
  code: '',
  notes: '',
  allow: 0,
  fen: 0,
  state: 'y'
})

const rules = {
  name: [{ required: true, message: '请输入函数名称' }]
}

// 编辑器配置
const editorTheme = ref('vs-dark')
const editorSuggestions = ref(true)

// 代码查看器
const codeModalVisible = ref(false)
const viewerTheme = ref('vs-dark')
const currentCode = ref('')

// 函数模板
const functionTemplate = `/**
 * 云函数示例
 * @param {Object} data - 传入的数据
 * @param {Object} user - 用户信息
 * @returns {Object} 返回处理结果
 */
async function handler(data, user) {
  // 在这里编写你的业务逻辑
  
  return {
    success: true,
    message: '处理成功',
    data: {}
  }
}`

// 表格列
const columns = [
  { title: 'ID', dataIndex: 'id', width: 80 },
  { title: '函数名称', dataIndex: 'name', width: 150 },
  { title: '备注', dataIndex: 'notes', ellipsis: true },
  { title: 'VIP权限', slotName: 'allow', width: 100 },
  { title: '积分消耗', dataIndex: 'fen', width: 80 },
  { title: '状态', slotName: 'state', width: 80 },
  { title: '操作', slotName: 'actions', width: 200 }
]

// 插入模板
const insertTemplate = () => {
  form.code = functionTemplate
  Message.success('已插入模板')
}

// 格式化代码
const formatCode = () => {
  editorRef.value?.formatCode()
  Message.success('代码已格式化')
}

// 复制代码
const copyCode = async () => {
  try {
    await navigator.clipboard.writeText(currentCode.value)
    Message.success('代码已复制到剪贴板')
  } catch (e) {
    Message.error('复制失败')
  }
}

// 加载数据
const loadData = async () => {
  loading.value = true
  try {
    const res = await functionApi.getList(pagination.current, pagination.pageSize)
    if (res.code === 200) {
      tableData.value = res.data.list || []
      pagination.total = res.data.dataTotal || 0
      pagination.current = res.data.currentPage || 1
    }
  } catch (e) {
    Message.error('加载数据失败')
  } finally {
    loading.value = false
  }
}

// 分页变化
const handlePageChange = (page) => {
  pagination.current = page
  loadData()
}

// 添加
const handleAdd = () => {
  Object.assign(form, { id: null, name: '', code: functionTemplate, notes: '', allow: 0, fen: 0, state: 'y' })
  modalVisible.value = true
}

// 编辑
const handleEdit = async (record) => {
  try {
    const res = await functionApi.getInfo(record.id)
    if (res.code === 200) {
      // 解码 Base64 编码的代码
      let decodedCode = ''
      if (res.data.code) {
        try {
          // 使用 TextDecoder 正确解码 UTF-8
          const binaryString = atob(res.data.code)
          const bytes = new Uint8Array(binaryString.length)
          for (let i = 0; i < binaryString.length; i++) {
            bytes[i] = binaryString.charCodeAt(i)
          }
          decodedCode = new TextDecoder('utf-8').decode(bytes)
        } catch (e) {
          decodedCode = res.data.code || ''
        }
      }
      Object.assign(form, {
        id: res.data.id,
        name: res.data.name,
        code: decodedCode,
        notes: res.data.notes || '',
        allow: res.data.allow || 0,
        fen: res.data.fen || 0,
        state: res.data.state || 'y'
      })
      modalVisible.value = true
    }
  } catch (e) {
    Message.error('获取函数信息失败')
  }
}

// 查看代码
const handleViewCode = async (record) => {
  try {
    const res = await functionApi.getCode(record.id)
    if (res.code === 200) {
      // Base64 解码 (UTF-8)
      try {
        const binaryString = atob(res.data.code || '')
        const bytes = new Uint8Array(binaryString.length)
        for (let i = 0; i < binaryString.length; i++) {
          bytes[i] = binaryString.charCodeAt(i)
        }
        currentCode.value = new TextDecoder('utf-8').decode(bytes)
      } catch (e) {
        currentCode.value = res.data.code || ''
      }
      codeModalVisible.value = true
    }
  } catch (e) {
    Message.error('获取代码失败')
  }
}

// 提交
const handleSubmit = async () => {
  const valid = await formRef.value?.validate()
  if (valid) return

  // 验证函数名称
  const name = form.name
  if (!/^[a-zA-Z]/.test(name)) {
    Message.warning('函数名称必须以字母开头')
    return
  }
  if (name.length < 3 || name.length > 64) {
    Message.warning('函数名称长度必须为3-64位')
    return
  }
  if (!/^[a-zA-Z0-9]+$/.test(name)) {
    Message.warning('函数名称只能包含字母和数字')
    return
  }

  submitLoading.value = true
  try {
    const res = await functionApi.submit(form)
    if (res.code === 200) {
      Message.success(form.id ? '编辑成功' : '添加成功')
      modalVisible.value = false
      loadData()
    } else {
      Message.error(res.msg || '操作失败')
    }
  } catch (e) {
    Message.error('操作失败')
  } finally {
    submitLoading.value = false
  }
}

// 取消
const handleCancel = () => {
  modalVisible.value = false
}

// 删除
const handleDelete = async (record) => {
  try {
    const res = await functionApi.del(record.id)
    if (res.code === 200) {
      Message.success('删除成功')
      loadData()
    } else {
      Message.error(res.msg || '删除失败')
    }
  } catch (e) {
    Message.error('删除失败')
  }
}

// 状态切换
const handleStateChange = async (record, checked) => {
  const newState = checked ? 'y' : 'n'
  try {
    const res = await functionApi.editState({ id: record.id, state: newState })
    if (res.code === 200) {
      record.state = newState
      Message.success('状态更新成功')
    } else {
      Message.error(res.msg || '操作失败')
    }
  } catch (e) {
    Message.error('操作失败')
  }
}

onMounted(() => {
  loadData()
})
</script>

<script>
export default { name: 'CloudFunction' }
</script>

<style scoped>
.cloud-function {
  padding: 16px;
}

.code-editor-wrapper {
  border: 1px solid var(--color-border);
  border-radius: 6px;
  overflow: hidden;
  width: 100%;
}

.editor-toolbar {
  padding: 8px 12px;
  background: transparent;
  border-bottom: 1px solid var(--color-border);
  display: flex;
  flex-wrap: wrap;
  gap: 8px;
}

.form-footer {
  padding-top: 16px;
  text-align: right;
  border-top: 1px solid var(--color-border);
  margin-top: 16px;
}

.code-viewer-wrapper {
  border: 1px solid var(--color-border);
  border-radius: 6px;
  overflow: hidden;
  width: 100%;
}

.viewer-toolbar {
  padding: 8px 12px;
  background: transparent;
  border-bottom: 1px solid var(--color-border);
  display: flex;
  flex-wrap: wrap;
  gap: 8px;
}
</style>
