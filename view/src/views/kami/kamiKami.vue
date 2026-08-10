<template>
  <div class="p-5 bg-[--color-bg-1]">
    <div class="w-full mx-auto">
      <!-- 搜索栏 -->
      <div class="w-full">
        <a-form :model="searchForm.form" auto-label-width>
          <div class="grid grid-cols-1 md:grid-cols-2 lg:grid-cols-5 gap-x-5">
            <a-form-item field="state" label="卡密状态">
              <a-select v-model="searchForm.form.state" placeholder="全部" allow-clear class="w-full" @change="handleSearch">
                <a-option value="y">正常</a-option>
                <a-option value="n">禁用</a-option>
              </a-select>
            </a-form-item>
            <a-form-item field="use_state" label="使用状态">
              <a-select v-model="searchForm.form.use_state" placeholder="全部" allow-clear class="w-full" @change="handleSearch">
                <a-option value="y">已使用</a-option>
                <a-option value="n">未使用</a-option>
              </a-select>
            </a-form-item>
            <a-form-item field="out_state" label="导出状态">
              <a-select v-model="searchForm.form.out_state" placeholder="全部" allow-clear class="w-full" @change="handleSearch">
                <a-option value="y">已导出</a-option>
                <a-option value="n">未导出</a-option>
              </a-select>
            </a-form-item>
            <a-form-item field="add_role" label="创建人">
              <a-select v-model="searchForm.form.add_role" placeholder="全部" allow-clear class="w-full" @change="handleSearch">
                <a-option value="admin">管理员</a-option>
                <a-option value="agent">代理</a-option>
              </a-select>
            </a-form-item>
            <a-form-item field="type" label="卡密类型">
              <a-select v-model="searchForm.form.type" placeholder="全部" allow-clear class="w-full" @change="handleSearch">
                <a-option value="vip">会员</a-option>
                <a-option value="fen">积分</a-option>
                <a-option value="addsn">设备增绑卡</a-option>
              </a-select>
            </a-form-item>
            <a-form-item field="gid" label="分组">
              <a-select v-model="searchForm.form.gid" placeholder="全部" allow-clear class="w-full" @change="handleSearch">
                <a-option v-for="group in addModal.group" :key="group.id" :value="group.id">{{ group.name }}</a-option>
              </a-select>
            </a-form-item>
            <a-form-item field="expire" label="过期">
              <a-select v-model="searchForm.form.expire" placeholder="全部" allow-clear class="w-full" @change="handleSearch">
                <a-option value="y">已过期</a-option>
                <a-option value="n">未过期</a-option>
              </a-select>
            </a-form-item>
            <a-form-item field="add_time" label="创建时间">
              <a-range-picker v-model="searchForm.form.add_time" style="width: 100%" @change="handleSearch" />
            </a-form-item>
            <a-form-item field="use_time" label="使用时间">
              <a-range-picker v-model="searchForm.form.use_time" style="width: 100%" @change="handleSearch" />
            </a-form-item>
            <a-form-item field="keyword" label="关键词">
              <a-input-group>
                <a-select v-model="searchForm.form.keywordType" style="width: 105px">
                  <a-option value="cdk">卡号</a-option>
                  <a-option value="user">使用者</a-option>
                  <a-option value="note">备注</a-option>
                  <a-option value="use_ip">使用IP</a-option>
                  <a-option value="phone">手机号</a-option>
                  <a-option value="email">邮箱</a-option>
                </a-select>
                <a-input-search
                  v-model="searchForm.form.keyword"
                  :placeholder="keywordPlaceholder"
                  :loading="searchForm.btnLoading"
                  allow-clear
                  @search="handleSearch"
                  @press-enter="handleSearch"
                  @clear="handleSearch"
                />
              </a-input-group>
            </a-form-item>
          </div>
        </a-form>
      </div>

      <!-- 操作栏 -->
      <div class="lg:flex items-center justify-between">
        <div class="w-full lg:w-auto">
          <a-button type="primary" class="mb-5" @click="handleAdd" :disabled="!auth('cdkKami')">
            <template #icon><icon-plus /></template>
            创建卡密
          </a-button>
          <a-button class="mb-5 ml-4" status="danger" @click="handleClear" :disabled="!auth('cdkKami')">
            <template #icon><icon-delete /></template>
            清理卡密
          </a-button>
          <a-button class="mb-5 ml-4" @click="handleAward" :disabled="!auth('cdkKami')">
            <template #icon><icon-gift /></template>
            发送奖励
          </a-button>
        </div>
      </div>

      <!-- 数据表格 -->
      <a-table
        :columns="tableConfig.columns"
        :data="tableConfig.list"
        :loading="tableConfig.loading"
        :pagination="false"
        :row-selection="tableConfig.rowSelection"
        v-model:selectedKeys="tableConfig.selectedKeys"
        row-key="id"
        :bordered="false"
      >
        <template #cardNo="{ record }">
          <div class="flex items-center">
            <span class="mr-2">{{ record.cdk }}</span>
            <a-tag :size="'small'" :color="record.type === 'vip' ? 'red' : (record.type === 'fen' ? 'orange' : '')">
              {{ record.type === 'vip' ? '会员卡' : (record.type === 'fen' ? '积分卡' : '设备增绑卡') }}
            </a-tag>
          </div>
          <p class="text-[--color-text-3] text-[0.75rem]">{{ record.note }}</p>
        </template>
        <template #add_role="{ record }">
          <a-tooltip :content="record.Gname">
            <span>{{ formatVal(record) }}</span>
          </a-tooltip>
          <p class="text-[--color-text-3] text-[0.75rem]">{{ record.add_user || '' }}</p>
        </template>
        <template #use="{ record }">
          <a-tooltip :content="`使用IP：${record.use_ip || ''}`">
            <span>{{ record.use_user ? record.use_user : '未使用' }}</span>
          </a-tooltip>
          <p class="text-[--color-text-3] text-[0.75rem]">{{ record.use_time ? formatTime(record.use_time) : '未使用' }}</p>
        </template>
        <template #add_time="{ record }">
          <span>{{ formatTime(record.add_time) }}</span>
          <p class="text-[--color-text-3] text-[0.75rem]">{{ record.out_time ? formatTime(record.out_time) : '未导出' }}</p>
        </template>
        <template #state="{ record, rowIndex }">
          <a-switch
            v-model="record.state"
            checked-value="y"
            unchecked-value="n"
            checked-color="#23C343"
            unchecked-color="#F53F3F"
            size="small"
            @change="(val) => handleStateChange(record, val, rowIndex)"
          />
        </template>
        <template #operate="{ record }">
          <div>
            <a-button type="text" size="small" @click="handleEdit(record)" :disabled="!auth('cdkKami')">
              编辑
            </a-button>
            <a-popconfirm type="warning" position="tr" :content="`确认删除：${record.cdk} ？`" @before-ok="() => handleDelete(record.id)">
              <a-button type="text" size="small" status="danger" @click="delId = record.id" :disabled="!auth('cdkKami')">
                删除
              </a-button>
            </a-popconfirm>
          </div>
        </template>
      </a-table>

      <!-- 分页和批量操作 -->
      <div class="w-full md:flex items-center justify-between mt-4">
        <div class="mb-5 md:mb-0 text-center">
          <span v-if="tableConfig.selectedKeys.length > 0" class="text-gray-500 text-sm">
            <a-button type="text" size="small" @click="exportModal.visible = true">导出</a-button>
            <a-popconfirm type="warning" content="确定删除选中数据？" @before-ok="handleBatchDelete">
              <a-button type="text" size="small" status="danger" :disabled="!auth('cdkKami')">删除</a-button>
            </a-popconfirm>
            当前选中的 {{ tableConfig.selectedKeys.length }} 条数据
          </span>
          <span v-else class="text-gray-500 text-sm">
            当前第 {{ tableConfig.currentPage }} 页 共 {{ tableConfig.pageTotal }} 页 {{ tableConfig.dataTotal }} 条结果
          </span>
        </div>
        <div class="flex justify-center">
          <a-pagination
            :total="tableConfig.dataTotal"
            :current="tableConfig.currentPage"
            :page-size="tableConfig.pageSize"
            @change="handlePageChange"
            @page-size-change="handlePageSizeChange"
            show-page-size
          />
        </div>
      </div>
    </div>

    <!-- 发送奖励弹窗 -->
    <a-modal
      v-model:visible="awardModal.visible"
      title="发送奖励"
      :width="400"
      :footer="false"
      title-align="start"
      :mask-closable="false"
    >
      <div class="md:w-80">
        <a-form :model="awardModal.form" auto-label-width @submit="handleAwardSubmit">
          <a-form-item field="object" label="奖励对象">
            <a-radio-group v-model="awardModal.form.object">
              <a-radio value="vip">会员卡</a-radio>
              <a-radio value="fen">积分卡</a-radio>
            </a-radio-group>
          </a-form-item>
          <a-form-item field="val" :label="awardModal.form.object === 'fen' ? '积分值' : '会员值'">
            <a-input-number v-model="awardModal.form.val" placeholder="1" style="width: 100%">
              <template #append>
                <template v-if="awardModal.form.object === 'vip'">
                  <a-select v-model="awardModal.vipType" style="width: 65px">
                    <a-option value="s">秒</a-option>
                    <a-option value="i">分</a-option>
                    <a-option value="h">时</a-option>
                    <a-option value="d">天</a-option>
                  </a-select>
                </template>
                <template v-else>
                  <span> 积分 </span>
                </template>
              </template>
            </a-input-number>
          </a-form-item>
          <a-alert class="mb-2" closable>
            仅会奖励已使用但{{ awardModal.form.object === 'vip' ? '未到期' : '积分未耗尽' }}的卡密
          </a-alert>
          <a-space direction="vertical" fill>
            <a-button type="primary" html-type="submit" :loading="awardModal.btnLoading" long>
              提交
            </a-button>
          </a-space>
        </a-form>
      </div>
    </a-modal>

    <!-- 导出弹窗 -->
    <a-modal
      v-model:visible="exportModal.visible"
      title="导出卡密"
      :width="400"
      :footer="false"
      title-align="start"
      :mask-closable="false"
    >
      <div class="md:w-80">
        <a-form :model="exportModal.data" auto-label-width @submit="handleExport">
          <a-form-item field="out" label="当前选中">
            <span>{{ tableConfig.selectedKeys.length }} 条卡密数据</span>
          </a-form-item>
          <a-form-item field="out" label="导出格式">
            <a-radio-group v-model="exportModal.data.out">
              <a-radio value="txt">文本（txt）</a-radio>
              <a-radio value="csv">表格（csv）</a-radio>
            </a-radio-group>
          </a-form-item>
          <a-space direction="vertical" fill>
            <a-button type="primary" html-type="submit" :loading="exportModal.btnLoading" long>
              确定导出
            </a-button>
          </a-space>
        </a-form>
      </div>
    </a-modal>

    <!-- 编辑弹窗 -->
    <a-drawer
      title="编辑卡密"
      :width="drawerWidth"
      :visible="editModal.visible"
      :footer="false"
      unmount-on-close
      @cancel="editModal.visible = false"
    >
      <!-- 卡密信息头部 -->
      <div class="bg-blue-600 rounded p-5 text-white">
        <div class="mb-5">
          <div class="flex justify-between items-center">
            <p class="text-lg font-bold">{{ editModal.form.cdk }}</p>
            <a-tag :size="'small'" :color="editModal.form.type === 'vip' ? 'red' : (editModal.form.type === 'fen' ? 'orange' : '')">
              {{ editModal.form.Gname }}
            </a-tag>
          </div>
          <p class="text-[rgb(101,149,255)]">卡号</p>
        </div>
        <template v-if="editModal.form.type !== 'addsn'">
          <div class="flex items-center justify-between">
            <p>邮箱</p>
            <p>{{ editModal.form.email || '未绑定' }}</p>
          </div>
          <hr class="my-3 border-blue-500" />
          <div class="flex items-center justify-between">
            <p>手机号</p>
            <p>{{ editModal.form.phone || '未绑定' }}</p>
          </div>
          <hr class="my-3 border-blue-500" />
        </template>
        <div class="flex items-center justify-between">
          <p>创建者</p>
          <p>{{ editModal.form.add_user || '' }}</p>
        </div>
        <hr class="my-3 border-blue-500" />
        <div class="flex items-center justify-between">
          <p>创建时间</p>
          <p>{{ formatTime(editModal.form.add_time) }}</p>
        </div>
        <hr class="my-3 border-blue-500" />
        <div class="flex items-center justify-between">
          <p>创建IP</p>
          <p>{{ editModal.form.add_ip || '' }}</p>
        </div>
      </div>

      <!-- 标签页内容 -->
      <div class="mt-5 bg-[--color-bg-1] rounded-sm">
        <a-tabs default-active-key="info">
          <a-tab-pane key="info" title="基本信息">
            <a-form :model="editModal.form" auto-label-width layout="vertical" class="px-2">
              <!-- 未封禁时可编辑备注 -->
              <template v-if="!editModal.ban.status">
                <a-form-item field="note" label="卡密备注">
                  <a-input v-model="editModal.form.note" placeholder="如：活动卡密（可空）" />
                </a-form-item>
              </template>

              <!-- 已使用且是VIP卡 -->
              <template v-if="!editModal.ban.status && editModal.form.use_time && editModal.form.type === 'vip'">
                <a-form-item label="会员到期时间">
                  <a-date-picker
                    v-model="editModal.form.vip"
                    show-time
                    format="YYYY-MM-DD HH:mm:ss"
                    value-format="timestamp"
                    style="width: 100%"
                    :shortcuts="vipShortcuts"
                  />
                </a-form-item>
              </template>

              <!-- 已使用且是积分卡 -->
              <template v-if="!editModal.ban.status && editModal.form.use_time && editModal.form.type === 'fen'">
                <a-form-item label="积分值">
                  <a-input-number v-model="editModal.form.fen" placeholder="1" style="width: 100%">
                    <template #suffix> 积分 </template>
                  </a-input-number>
                </a-form-item>
              </template>

              <!-- 非增绑卡可设置额外绑定数 -->
              <template v-if="!editModal.ban.status && editModal.form.type !== 'addsn'">
                <a-form-item field="sn_max" label="额外绑定设备数量" tooltip="基于系统设置额外增加可绑定设备数">
                  <a-input-number v-model="editModal.form.sn_max" placeholder="0" style="width: 100%" />
                </a-form-item>
              </template>

              <!-- 已使用时可设置封禁 -->
              <template v-if="editModal.form.use_time">
                <a-form-item label="封禁时间">
                  <div class="flex items-center">
                    <a-switch v-model="editModal.ban.status" size="small" @change="handleBanStatusChange" />
                    <span class="ml-2">禁用用户期限</span>
                  </div>
                  <a-date-picker
                    v-if="editModal.ban.status"
                    v-model="editModal.form.ban"
                    show-time
                    format="YYYY-MM-DD HH:mm:ss"
                    value-format="timestamp"
                    style="width: 100%; margin-top: 8px"
                    :shortcuts="banShortcuts"
                  />
                </a-form-item>
                <a-form-item v-if="editModal.ban.status" field="ban_msg" label="禁用原因">
                  <a-textarea v-model="editModal.form.ban_msg" placeholder="如：违反用户使用协议，禁用中" allow-clear />
                </a-form-item>
              </template>

              <a-button type="primary" :loading="editModal.btnLoading" @click="handleEditSubmit" long>
                提交
              </a-button>
            </a-form>
          </a-tab-pane>

          <!-- 绑定设备 -->
          <a-tab-pane v-if="editModal.form.type !== 'addsn' && editModal.form.use_time" key="snlist" title="绑定设备">
            <template v-if="deviceList.list.length < 1">
              <a-empty description="暂无登录记录" />
            </template>
            <template v-else>
              <table class="min-w-full divide-y divide-[--color-border-1]">
                <thead class="bg-[--color-fill-2]">
                  <tr>
                    <td class="text-left py-2 px-2">机器码</td>
                    <td class="w-16 py-2 px-2">时间</td>
                    <td class="w-16 py-2 px-2">操作</td>
                  </tr>
                </thead>
                <tbody class="divide-y divide-[--color-border-1]">
                  <tr v-for="(device, index) in deviceList.list" :key="index">
                    <td class="text-left py-2 px-2 truncate max-w-20">{{ device.udid || device }}</td>
                    <td class="py-2 px-2">{{ device.time ? formatTime(device.time) : '' }}</td>
                    <td class="py-2 px-2">
                      <a-popconfirm type="warning" position="tr" :content="`确认解绑：${device.udid || device} ？`" @before-ok="() => handleUnbindDevice(device.udid || device, index)">
                        <a-button type="text" size="small" status="danger" @click="unbindDevice.udid = device.udid || device; unbindDevice.index = index">
                          删除
                        </a-button>
                      </a-popconfirm>
                    </td>
                  </tr>
                </tbody>
              </table>
            </template>
          </a-tab-pane>

          <!-- 操作日志 -->
          <a-tab-pane v-if="editModal.form.type !== 'addsn' && editModal.form.use_time" key="logs" title="操作日志">
            <template v-if="logList.list.length < 1">
              <a-empty description="暂无用户日志" />
            </template>
            <template v-else>
              <table class="min-w-full divide-y divide-[--color-border-1]">
                <thead class="bg-[--color-fill-2]">
                  <tr>
                    <td class="text-left w-16 py-2 px-2">类型</td>
                    <td class="text-left py-2 px-2"></td>
                    <td class="w-16 py-2 px-2">IP</td>
                  </tr>
                </thead>
                <tbody class="divide-y divide-[--color-border-1]">
                  <tr v-for="(log, index) in logList.list" :key="index">
                    <td class="text-left py-2 px-2">
                      <p>{{ log.type }}</p>
                      <span class="text-[--color-text-3] text-[0.75rem]">{{ formatTime(log.time) }}</span>
                    </td>
                    <td class="py-2 px-2 text-left">
                      <template v-if="log.asset_changes">
                        <div v-for="(val, key) in log.asset_changes" :key="key">
                          {{ key === 'fen' ? '积分' : key === 'vip' ? 'VIP' : key === 'money' ? '余额' : key }}:{{ val }}
                        </div>
                      </template>
                    </td>
                    <td class="py-2 px-2">
                      <p>{{ log.ip_address || '' }}</p>
                      <span class="text-[--color-text-3] text-[0.75rem]">{{ log.ip || '' }}</span>
                    </td>
                  </tr>
                </tbody>
              </table>
            </template>
          </a-tab-pane>
        </a-tabs>
      </div>
    </a-drawer>

    <!-- 创建卡密弹窗 -->
    <a-modal
      v-model:visible="addModal.visible"
      title="创建卡密"
      :width="400"
      :footer="false"
      title-align="start"
      :mask-closable="false"
    >
      <div class="md:w-80">
        <a-form :model="addModal.data" auto-label-width @submit="handleAddSubmit">
          <a-form-item field="gid" label="卡密组">
            <a-select v-model="addModal.data.gid" placeholder="请选择卡密组">
              <template #empty>
                <div class="w-full flex flex-col items-center justify-center py-5">
                  <icon-empty />
                  <p class="text-[--color-text-3]">暂无卡密组</p>
                </div>
              </template>
              <a-option v-for="group in addModal.group" :key="group.id" :value="group.id">
                {{ group.name }}
              </a-option>
            </a-select>
          </a-form-item>
          <a-form-item field="note" label="备注">
            <a-input v-model="addModal.data.note" placeholder="如：活动卡密（可空）" />
          </a-form-item>
          <a-form-item field="length" label="长度" tooltip="自定义卡密长度，不过为保证卡密唯一性，卡密长度仅可在8~32位字符区间">
            <a-input-number v-model="addModal.data.length" placeholder="13" :min="8" :max="32" style="width: 100%" />
          </a-form-item>
          <a-form-item field="pre" label="前缀" tooltip="卡密前缀有助于区分卡密，支持字母、数字、下划线(_)、横杠(-)">
            <a-input v-model="addModal.data.pre" placeholder="如：TK-（可空）" />
          </a-form-item>
          <a-form-item field="num" label="数量" tooltip="为了避免生成超时，一次性最多生成4000张">
            <a-input-number v-model="addModal.data.num" placeholder="1" :min="1" :max="4000" style="width: 100%">
              <template #suffix> 张 </template>
            </a-input-number>
          </a-form-item>
          <a-form-item field="password" label="密码" tooltip="设置卡密使用密码">
            <a-input v-model="addModal.data.password" placeholder="空则无需密码" />
          </a-form-item>
          <a-form-item field="out" label="导出">
            <a-select v-model="addModal.data.out" placeholder="请选择导出格式（不选择不导出）" allow-clear>
              <a-option value="txt">文本（txt）</a-option>
              <a-option value="csv">表格（csv）</a-option>
            </a-select>
          </a-form-item>
          <a-space direction="vertical" fill>
            <a-button type="primary" html-type="submit" :loading="addModal.btnLoading" long>
              提交
            </a-button>
          </a-space>
        </a-form>
      </div>
    </a-modal>
  </div>
</template>

<script setup>
import { ref, reactive, computed, onMounted, onUnmounted } from 'vue'
import { Message, Modal } from '@arco-design/web-vue'
import dayjs from 'dayjs'
import { auth } from '@/utils/common'
import cdkKamiApi from '@/api/system/cdkKami'
import { formatVipTime, toSeconds } from '@/utils/sun.js'

// 删除ID
const delId = ref(0)

// 抽屉宽度响应式
const drawerWidth = ref(520)
const handleResize = () => {
  drawerWidth.value = window.innerWidth < 765 ? window.innerWidth : 520
}

// 搜索表单
const searchForm = reactive({
  form: {
    state: '',
    out_state: '',
    use_state: '',
    add_role: '',
    type: '',
    gid: null,
    expire: null,
    add_time: [],
    use_time: [],
    keywordType: 'cdk',
    keyword: ''
  },
  btnLoading: false,
  keywordTypePlaceholder: {
    phone: '请输入手机号',
    email: '请输入邮箱账号',
    cdk: '请输入卡密卡号',
    user: '请输入使用者账号',
    note: '请输入备注',
    use_ip: '请输入使用IP'
  }
})

const keywordPlaceholder = computed(() => {
  return searchForm.keywordTypePlaceholder[searchForm.form.keywordType] || '请输入关键词'
})

// 表格配置
const tableConfig = reactive({
  columns: [
    { title: 'ID', dataIndex: 'id', width: 60, align: 'center' },
    { title: '卡密/备注', slotName: 'cardNo' },
    { title: '面值/创建人', slotName: 'add_role', width: 100, align: 'center' },
    { title: '使用者/时间', slotName: 'use', width: 100, align: 'center' },
    { title: '创建/导出时间', slotName: 'add_time', width: 100, align: 'center' },
    { title: '状态', slotName: 'state', width: 60, align: 'center' },
    { title: '操作', slotName: 'operate', width: 100, align: 'center' }
  ],
  list: [],
  loading: true,
  pageSize: 10,
  currentPage: 1,
  pageTotal: 0,
  dataTotal: 0,
  rowSelection: { type: 'checkbox', showCheckedAll: true, onlyCurrent: false },
  selectedKeys: []
})

// 设备列表
const deviceList = reactive({ list: [] })

// 日志列表
const logList = reactive({ list: [] })

// 创建弹窗
const addModal = reactive({
  visible: false,
  btnLoading: false,
  data: { gid: '', note: '', length: 13, pre: '', num: 1, password: '', out: '' },
  group: []
})

// 编辑弹窗
const editModal = reactive({
  visible: false,
  btnLoading: false,
  form: {},
  ban: { timestamp: '', status: false },
  vip: { timestamp: '' }
})

// 解绑设备
const unbindDevice = reactive({ index: 0, udid: '' })

// 导出弹窗
const exportModal = reactive({
  visible: false,
  btnLoading: false,
  data: { out: 'txt' }
})

// 奖励弹窗
const awardModal = reactive({
  visible: false,
  btnLoading: false,
  form: { object: 'vip', val: undefined },
  vipType: 'd'
})

// VIP时间快捷选项
const vipShortcuts = [
  { label: '此刻', value: () => dayjs().add(1, 'second').valueOf() },
  { label: '1个月', value: () => dayjs().add(1, 'month').valueOf() },
  { label: '3个月', value: () => dayjs().add(3, 'month').valueOf() },
  { label: '一年', value: () => dayjs().add(1, 'year').valueOf() },
  { label: '永久', value: () => 9999999999000 }
]

// 封禁时间快捷选项
const banShortcuts = [
  { label: '此刻', value: () => dayjs().add(1, 'second').valueOf() },
  { label: '一周', value: () => dayjs().add(1, 'week').valueOf() },
  { label: '一月', value: () => dayjs().add(1, 'month').valueOf() },
  { label: '一年', value: () => dayjs().add(1, 'year').valueOf() },
  { label: '永久', value: () => 9999999999000 }
]

// 格式化时间
const formatTime = (timestamp) => {
  if (!timestamp) return ''
  return dayjs.unix(timestamp).format('YYYY-MM-DD HH:mm:ss')
}

// 格式化面值
const formatVal = (record) => {
  if (record.type === 'vip') {
    return formatVipTime(Number(record.val))
  }
  return record.val + (record.type === 'fen' ? '积分' : '台')
}

// 加载数据
const loadData = async () => {
  tableConfig.loading = true
  try {
    const params = {
      ...searchForm.form,
      page: tableConfig.currentPage,
      size: tableConfig.pageSize
    }
    // 处理空值
    if (params.gid === '') params.gid = null
    if (params.expire === '') params.expire = null

    const res = await cdkKamiApi.getList(params)
    if (res.code !== 200) {
      Message.error(res.msg)
      return
    }
    tableConfig.list = res.data.list || []
    tableConfig.currentPage = res.data.currentPage
    tableConfig.dataTotal = res.data.dataTotal
    tableConfig.pageTotal = res.data.pageTotal
  } catch (e) {
    Message.error('出错了：' + e)
  } finally {
    tableConfig.loading = false
    searchForm.btnLoading = false
  }
}

// 加载分组列表
const loadGroups = async () => {
  try {
    const res = await cdkKamiApi.getGroupList()
    if (res.code === 200) {
      addModal.group = res.data || []
    }
  } catch (e) {
    console.error('加载分组失败', e)
  }
}

// 搜索
const handleSearch = () => {
  searchForm.btnLoading = true
  tableConfig.currentPage = 1
  loadData()
}

// 分页
const handlePageChange = (page) => {
  tableConfig.currentPage = page
  tableConfig.loading = true
  loadData()
}

const handlePageSizeChange = (size) => {
  tableConfig.pageSize = size
  tableConfig.loading = true
  loadData()
}

// 创建卡密
const handleAdd = () => {
  addModal.data = { gid: '', note: '', length: 13, pre: '', num: 1, password: '', out: '' }
  addModal.visible = true
}

// 提交创建
const handleAddSubmit = async () => {
  addModal.btnLoading = true
  try {
    const res = await cdkKamiApi.add(addModal.data)
    if (res.code !== 200) {
      Message.error(res.msg)
      return
    }
    addModal.visible = false
    Message.success(res.msg)
    if (res.data && res.data.downUrl) {
      cdkKamiApi.downloadFile(res.data.downUrl)
    }
    loadData()
  } catch (e) {
    Message.error('出错了：' + e)
  } finally {
    addModal.btnLoading = false
  }
}

// 编辑
const handleEdit = async (record) => {
  editModal.form = { ...record }

  // 处理VIP到期时间
  if (record.type === 'vip' && record.vip_exp) {
    editModal.vip.timestamp = formatTime(record.vip_exp)
    editModal.form.vip = record.vip_exp >= 9999999999 ? 9999999999000 : record.vip_exp * 1000
  }

  // 处理封禁状态
  editModal.ban.status = false
  if (record.ban && record.ban > dayjs().unix()) {
    editModal.ban.status = true
    editModal.ban.timestamp = record.ban >= 9999999999 ? '9999-99-99 99:99:99' : formatTime(record.ban)
    editModal.form.ban = record.ban >= 9999999999 ? 9999999999000 : record.ban * 1000
  }

  // 设备列表
  if (record.sn_list) {
    deviceList.list = record.sn_list
  } else {
    deviceList.list = []
  }

  // 加载日志
  try {
    const res = await cdkKamiApi.getLog(record.id)
    if (res.code === 200) {
      logList.list = res.data || []
    }
  } catch (e) {
    console.error('加载日志失败', e)
  }

  editModal.visible = true
}

// 封禁状态变更
const handleBanStatusChange = (val) => {
  if (!val) {
    editModal.form.ban = null
    editModal.ban.timestamp = ''
  }
}

// 提交编辑
const handleEditSubmit = async () => {
  editModal.btnLoading = true
  try {
    const data = {
      id: editModal.form.id,
      note: editModal.form.note,
      vip: null,
      fen: editModal.form.fen,
      val: editModal.form.val,
      password: editModal.form.password,
      sn_max: editModal.form.sn_max,
      ban: null,
      ban_msg: editModal.form.ban_msg
    }

    // 处理VIP时间
    if (editModal.form.vip) {
      if (editModal.form.vip >= 9999999999000) {
        data.vip = 9999999999
      } else {
        data.vip = Math.floor(editModal.form.vip / 1000)
      }
    }

    // 处理封禁时间
    if (editModal.form.ban) {
      if (editModal.form.ban >= 9999999999000) {
        data.ban = 9999999999
      } else {
        data.ban = Math.floor(editModal.form.ban / 1000)
      }
    }

    const res = await cdkKamiApi.edit(data)
    if (res.code !== 200) {
      Message.error(res.msg)
      return
    }
    editModal.visible = false
    Message.success(res.msg)
    loadData()
  } catch (e) {
    Message.error('出错了：' + e)
  } finally {
    editModal.btnLoading = false
  }
}

// 解绑设备
const handleUnbindDevice = async (udid, index) => {
  try {
    const res = await cdkKamiApi.unbindSn(editModal.form.id, udid)
    if (res.code !== 200) {
      Message.error(res.msg)
      return false
    }
    Message.success(res.msg)
    deviceList.list.splice(index, 1)
    editModal.form.sn_list = deviceList.list
    return true
  } catch (e) {
    Message.error('出错了：' + e)
    return false
  }
}

// 状态切换
const handleStateChange = async (record, val, rowIndex) => {
  try {
    const res = await cdkKamiApi.editState(record.id, val)
    if (res.code !== 200) {
      Message.error(res.msg)
      tableConfig.list[rowIndex].state = val === 'y' ? 'n' : 'y'
      return
    }
    Message.success(res.msg)
  } catch (e) {
    tableConfig.list[rowIndex].state = val === 'y' ? 'n' : 'y'
    Message.error('出错了：' + e)
  }
}

// 删除
const handleDelete = async (id) => {
  try {
    const res = await cdkKamiApi.del(id)
    if (res.code !== 200) {
      Message.error(res.msg)
      return false
    }
    Message.success(res.msg)
    loadData()
    return true
  } catch (e) {
    Message.error('出错了：' + e)
    return false
  }
}

// 批量删除
const handleBatchDelete = async () => {
  try {
    const res = await cdkKamiApi.delAll(tableConfig.selectedKeys)
    if (res.code !== 200) {
      Message.error(res.msg)
      return false
    }
    tableConfig.selectedKeys = []
    Message.success(res.msg)
    loadData()
    return true
  } catch (e) {
    Message.error('出错了：' + e)
    return false
  }
}

// 导出
const handleExport = async () => {
  exportModal.btnLoading = true
  try {
    const res = await cdkKamiApi.outAll(tableConfig.selectedKeys, exportModal.data.out)
    if (res.code !== 200) {
      Message.error(res.msg)
      return
    }
    tableConfig.selectedKeys = []
    Message.success(res.msg)
    if (res.data && res.data.downUrl) {
      cdkKamiApi.downloadFile(res.data.downUrl)
    }
    exportModal.visible = false
  } catch (e) {
    Message.error('出错了：' + e)
  } finally {
    exportModal.btnLoading = false
  }
}

// 清理卡密
const handleClear = () => {
  Modal.info({
    titleAlign: 'start',
    title: '确认清理卡密',
    content: '提示：此操作仅清理已被使用的卡密，清理后不可恢复，请谨慎操作！',
    okText: '确认清理',
    hideCancel: false,
    width: 350,
    onBeforeOk: async () => {
      try {
        const res = await cdkKamiApi.clear()
        if (res.code !== 200) {
          Message.error(res.msg)
          return false
        }
        Message.success(res.msg)
        loadData()
        return true
      } catch (e) {
        Message.error('出错了：' + e)
        return false
      }
    }
  })
}

// 发送奖励
const handleAward = () => {
  awardModal.form = { object: 'vip', val: undefined }
  awardModal.vipType = 'd'
  awardModal.visible = true
}

// 提交奖励
const handleAwardSubmit = async () => {
  awardModal.btnLoading = true
  try {
    let val = awardModal.form.val
    if (awardModal.form.object === 'vip') {
      val = toSeconds(awardModal.form.val, awardModal.vipType)
    }

    const res = await cdkKamiApi.award({
      object: awardModal.form.object,
      val: val
    })
    if (res.code !== 200) {
      Message.error(res.msg)
      return
    }
    awardModal.visible = false
    Message.success(res.msg)
    loadData()
  } catch (e) {
    Message.error('出错了：' + e)
  } finally {
    awardModal.btnLoading = false
  }
}

onMounted(() => {
  handleResize()
  loadGroups()
  loadData()
  window.addEventListener('resize', handleResize)
})

onUnmounted(() => {
  window.removeEventListener('resize', handleResize)
})
</script>

<script>
export default { name: 'KamiKami' }
</script>