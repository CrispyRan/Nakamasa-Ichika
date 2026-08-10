<template>
  <div class="message-management">
    <!-- 搜索栏 -->
    <a-card class="mb-4 ma-content-block" :bordered="false">
      <a-form :model="searchForm" layout="inline" auto-label-width>
        <a-form-item label="用户">
          <a-input v-model="searchForm.username" placeholder="请输入用户名" allow-clear style="width: 150px" />
        </a-form-item>
        <a-form-item label="状态">
          <a-select v-model="searchForm.status" placeholder="请选择" allow-clear style="width: 120px">
            <a-option :value="0">待处理</a-option>
            <a-option :value="1">已回复</a-option>
            <a-option :value="2">已关闭</a-option>
          </a-select>
        </a-form-item>
        <a-form-item>
          <a-space>
            <a-button type="primary" @click="handleSearch">搜索</a-button>
            <a-button @click="handleReset">重置</a-button>
          </a-space>
        </a-form-item>
      </a-form>
    </a-card>

    <!-- 数据表格 -->
    <a-card class="ma-content-block" :bordered="false">
      <a-table :columns="columns" :data="tableData" :loading="loading" :pagination="pagination" row-key="id" @page-change="handlePageChange">
        <template #avatar="{ record }">
          <a-avatar :size="32">
            <img v-if="record.avatar" :src="record.avatar" alt="avatar" />
            <span v-else>{{ (record.username || 'U').charAt(0).toUpperCase() }}</span>
          </a-avatar>
        </template>
        <template #content="{ record }">
          <div class="content-preview" @click="handleViewDetail(record)">
            {{ record.content }}
          </div>
        </template>
        <template #images="{ record }">
          <a-image-preview-group v-if="record.images && record.images.length">
            <a-image v-for="(img, idx) in record.images.slice(0, 3)" :key="idx" :src="img" width="32" height="32" fit="cover" class="mr-1 rounded" />
            <span v-if="record.images.length > 3" class="text-xs text-gray-500">+{{ record.images.length - 3 }}</span>
          </a-image-preview-group>
          <span v-else class="text-gray-400">-</span>
        </template>
        <template #status="{ record }">
          <a-tag v-if="record.status === 0" color="orange">待处理</a-tag>
          <a-tag v-else-if="record.status === 1" color="green">已回复</a-tag>
          <a-tag v-else color="gray">已关闭</a-tag>
        </template>
        <template #actions="{ record }">
          <a-space>
            <a-button type="text" size="small" @click="handleViewDetail(record)">详情</a-button>
            <a-button type="text" size="small" status="danger" @click="handleDelete(record)">删除</a-button>
          </a-space>
        </template>
      </a-table>
    </a-card>

    <!-- 聊天式详情弹窗 -->
    <a-modal v-model:visible="detailVisible" :title="`留言详情 - ${currentMessage.username || ''}`" :width="700" :footer="false" unmount-on-close>
      <div class="chat-container">
        <!-- 聊天记录区域 -->
        <div class="chat-messages" ref="chatRef">
          <div v-for="(msg, idx) in chatMessages" :key="idx" :class="['chat-item', msg.type === 'admin' ? 'admin' : 'user']">
            <div class="chat-avatar">
              <a-avatar :size="36">
                <img v-if="msg.avatar" :src="msg.avatar" />
                <span v-else>{{ (msg.username || (msg.type === 'admin' ? '管理' : 'U')).charAt(0).toUpperCase() }}</span>
              </a-avatar>
            </div>
            <div class="chat-content">
              <div class="chat-header">
                <span class="chat-username">{{ msg.username || (msg.type === 'admin' ? '管理员' : '用户') }}</span>
                <span class="chat-time">{{ msg.time }}</span>
              </div>
              <div class="chat-text">{{ msg.content }}</div>
              <div v-if="msg.images && msg.images.length" class="chat-images">
                <a-image-preview-group>
                  <a-image v-for="(img, imgIdx) in msg.images" :key="imgIdx" :src="img" width="80" height="80" fit="cover" class="mr-2 mb-2 rounded" />
                </a-image-preview-group>
              </div>
            </div>
          </div>
          <a-empty v-if="!chatMessages.length" description="暂无消息" />
        </div>

        <!-- 回复区域 -->
        <div class="chat-reply" v-if="currentMessage.status !== 2">
          <a-textarea v-model="replyContent" placeholder="输入回复内容..." :auto-size="{ minRows: 2, maxRows: 4 }" class="reply-input" />
          <div class="reply-actions">
            <a-upload :custom-request="handleUpload" :show-file-list="false" accept="image/*">
              <a-button type="text" size="small">
                <template #icon><icon-image /></template>
                图片
              </a-button>
            </a-upload>
            <div class="reply-images" v-if="replyImages.length">
              <div v-for="(img, idx) in replyImages" :key="idx" class="reply-image-item">
                <img :src="img" />
                <div class="remove-btn" @click="removeImage(idx)"><icon-close /></div>
              </div>
            </div>
            <a-button type="primary" size="small" :loading="replyLoading" @click="handleSendReply">
              发送
            </a-button>
          </div>
        </div>
        <div v-else class="chat-closed">
          <a-tag color="gray">该留言已关闭</a-tag>
        </div>
      </div>
    </a-modal>
  </div>
</template>

<script setup>
import { ref, reactive, nextTick, onMounted } from 'vue'
import { Message } from '@arco-design/web-vue'
import messageApi from '@/api/system/message'

const searchForm = reactive({ username: '', status: undefined })
const tableData = ref([])
const loading = ref(false)
const pagination = reactive({ current: 1, pageSize: 20, total: 0, showTotal: true })

const detailVisible = ref(false)
const currentMessage = ref({})
const chatMessages = ref([])
const chatRef = ref(null)
const replyContent = ref('')
const replyImages = ref([])
const replyLoading = ref(false)

const columns = [
  { title: '', dataIndex: 'avatar', slotName: 'avatar', width: 60 },
  { title: '用户', dataIndex: 'username', width: 100 },
  { title: '留言内容', dataIndex: 'content', slotName: 'content', ellipsis: true },
  { title: '图片', dataIndex: 'images', slotName: 'images', width: 120 },
  { title: '状态', dataIndex: 'status', slotName: 'status', width: 90 },
  { title: '创建时间', dataIndex: 'create_time', width: 160 },
  { title: '操作', slotName: 'actions', width: 120 }
]

const loadData = async () => {
  loading.value = true
  try {
    const res = await messageApi.getList({ ...searchForm, page: pagination.current, size: pagination.pageSize })
    if (res.code === 200) {
      tableData.value = res.data.list || []
      pagination.total = res.data.dataTotal || 0
    }
  } finally {
    loading.value = false
  }
}

const handleSearch = () => { pagination.current = 1; loadData() }
const handleReset = () => { Object.assign(searchForm, { username: '', status: undefined }); handleSearch() }
const handlePageChange = (page) => { pagination.current = page; loadData() }

const handleViewDetail = async (record) => {
  currentMessage.value = record
  detailVisible.value = true
  // 模拟加载聊天记录
  chatMessages.value = [
    {
      type: 'user',
      username: record.username,
      avatar: record.avatar,
      content: record.content,
      images: record.images || [],
      time: record.create_time
    }
  ]
  if (record.reply) {
    chatMessages.value.push({
      type: 'admin',
      username: '管理员',
      content: record.reply,
      time: record.reply_time || ''
    })
  }
  scrollToBottom()
}

const scrollToBottom = () => {
  nextTick(() => {
    if (chatRef.value) {
      chatRef.value.scrollTop = chatRef.value.scrollHeight
    }
  })
}

const handleUpload = async (options) => {
  const { fileItem, onSuccess, onError } = options
  // 这里模拟上传，实际需要调用上传接口
  const reader = new FileReader()
  reader.onload = (e) => {
    replyImages.value.push(e.target.result)
    onSuccess()
  }
  reader.onerror = () => onError()
  reader.readAsDataURL(fileItem.file)
}

const removeImage = (idx) => {
  replyImages.value.splice(idx, 1)
}

const handleSendReply = async () => {
  if (!replyContent.value.trim() && !replyImages.value.length) {
    Message.warning('请输入回复内容')
    return
  }
  replyLoading.value = true
  try {
    const res = await messageApi.edit({
      id: currentMessage.value.id,
      reply: replyContent.value,
      images: replyImages.value,
      status: 1
    })
    if (res.code === 200) {
      chatMessages.value.push({
        type: 'admin',
        username: '管理员',
        content: replyContent.value,
        images: [...replyImages.value],
        time: new Date().toLocaleString()
      })
      replyContent.value = ''
      replyImages.value = []
      scrollToBottom()
      Message.success('回复成功')
      loadData()
    }
  } finally {
    replyLoading.value = false
  }
}

const handleDelete = async (record) => {
  try {
    const res = await messageApi.del(record.id)
    if (res.code === 200) {
      Message.success('删除成功')
      loadData()
    }
  } catch (e) {
    Message.error('删除失败')
  }
}

onMounted(() => { loadData() })
</script>

<script>
export default { name: 'MessageList' }
</script>

<style scoped>
.message-management { padding: 16px; }

.content-preview {
  cursor: pointer;
  color: var(--color-text-1);
}
.content-preview:hover {
  color: rgb(var(--primary-6));
}

.chat-container {
  display: flex;
  flex-direction: column;
  height: 500px;
}

.chat-messages {
  flex: 1;
  overflow-y: auto;
  padding: 16px;
  background: transparent;
  border-radius: 8px;
}

.chat-item {
  display: flex;
  margin-bottom: 16px;
}

.chat-item.admin {
  flex-direction: row-reverse;
}

.chat-item.admin .chat-content {
  align-items: flex-end;
}

.chat-item.admin .chat-text {
  background: rgba(var(--primary-1), 0.5);
  border-radius: 12px 12px 0 12px;
}

.chat-avatar {
  flex-shrink: 0;
}

.chat-content {
  display: flex;
  flex-direction: column;
  margin: 0 12px;
  max-width: 70%;
}

.chat-header {
  display: flex;
  align-items: center;
  gap: 8px;
  margin-bottom: 4px;
}

.chat-item.admin .chat-header {
  flex-direction: row-reverse;
}

.chat-username {
  font-size: 13px;
  font-weight: 500;
  color: var(--color-text-1);
}

.chat-time {
  font-size: 12px;
  color: var(--color-text-3);
}

.chat-text {
  padding: 10px 14px;
  background: var(--color-fill-2);
  border-radius: 12px 12px 12px 0;
  font-size: 14px;
  line-height: 1.5;
  word-break: break-word;
}

.chat-images {
  margin-top: 8px;
}

.chat-reply {
  padding: 16px;
  background: transparent;
  border-top: 1px solid var(--color-border);
}

.reply-input {
  margin-bottom: 12px;
}

.reply-actions {
  display: flex;
  align-items: center;
  justify-content: space-between;
}

.reply-images {
  display: flex;
  gap: 8px;
  flex: 1;
  margin: 0 12px;
}

.reply-image-item {
  position: relative;
  width: 48px;
  height: 48px;
}

.reply-image-item img {
  width: 100%;
  height: 100%;
  object-fit: cover;
  border-radius: 4px;
}

.reply-image-item .remove-btn {
  position: absolute;
  top: -6px;
  right: -6px;
  width: 18px;
  height: 18px;
  background: var(--color-danger);
  color: #fff;
  border-radius: 50%;
  display: flex;
  align-items: center;
  justify-content: center;
  cursor: pointer;
  font-size: 10px;
}

.chat-closed {
  padding: 16px;
  text-align: center;
  background: transparent;
}
</style>