<script setup>
import { ref, reactive, computed, onMounted, onUnmounted, watch } from 'vue'
import { useRouter } from 'vue-router'
import { Message } from '@arco-design/web-vue'
import installApi from '@/api/install'

const router = useRouter()

// 当前步骤
const currentStep = ref(0)

// 加载状态
const loading = ref(false)
const envLoading = ref(false)
const installLoading = ref(false)

// 环境检测结果
const envInfo = ref(null)

// 安装类型
const installType = ref('new')

// 协议同意状态
const licenseAgreed = ref(false)

// 鼠标位置（用于交互式背景）
const mouseX = ref(50)
const mouseY = ref(50)

// 表单数据
const form = reactive({
  mysql_host: '127.0.0.1',
  mysql_port: 3306,
  mysql_name: '',
  mysql_user: '',
  mysql_pwd: '',
  mysql_pre: 'u_',
  redis_host: '127.0.0.1',
  redis_port: 6379,
  redis_pwd: '',
  admin_user: '',
  admin_pwd: '',
  admin_authcode: '',
  install_upgrade: '',
  adm_pwd: '',
  tls_enabled: true,
  cert_path: '',
  key_path: ''
})

// 步骤配置
const steps = [
  { icon: '📜', title: '许可协议', color: '#6366f1', glow: 'rgba(99, 102, 241, 0.5)' },
  { icon: '🔍', title: '环境检测', color: '#10b981', glow: 'rgba(16, 185, 129, 0.5)' },
  { icon: '💾', title: '数据库', color: '#f59e0b', glow: 'rgba(245, 158, 11, 0.5)' },
  { icon: '⚡', title: '缓存服务', color: '#ef4444', glow: 'rgba(239, 68, 68, 0.5)' },
  { icon: '👤', title: '管理员', color: '#8b5cf6', glow: 'rgba(139, 92, 246, 0.5)' },
  { icon: '🎉', title: '完成', color: '#06b6d4', glow: 'rgba(6, 182, 212, 0.5)' }
]

// 环境检测项
const envChecks = computed(() => {
  if (!envInfo.value) return []
  const info = envInfo.value
  return [
    { name: '主机名', value: info.name, status: true, icon: '🖥️' },
    { name: '软件环境', value: info.software ? '正常' : '异常', status: info.software, icon: '📦' },
    { name: 'PHP环境', value: info.php ? '正常' : '异常', status: info.php, icon: '🐘' },
    { name: '操作系统', value: info.os ? '正常' : '异常', status: info.os, icon: '⚙️' },
    { name: 'Redis', value: info.redis ? '正常' : '异常', status: info.redis, icon: '🔴' },
    { name: '上传组件', value: info.ue ? '正常' : '异常', status: info.ue, icon: '📤' },
    { name: 'MySQL', value: info.mysql ? '正常' : '异常', status: info.mysql, icon: '🐬' },
    { name: '配置目录', value: info.config_dir ? '可写' : '不可写', status: info.config_dir, icon: '📁' },
    { name: '数据库配置', value: info.config_db ? '正常' : '异常', status: info.config_db, icon: '🗃️' },
    { name: '应用配置', value: info.config_app ? '正常' : '异常', status: info.config_app, icon: '📱' },
    { name: '缓存配置', value: info.config_cache ? '正常' : '异常', status: info.config_cache, icon: '💾' },
    { name: '常规模式', value: info.normal ? '正常' : '异常', status: info.normal, icon: '✅' }
  ]
})

const envAllPass = computed(() => envChecks.value.every(item => item.status))

// 粒子系统
const particles = ref([])
const connections = ref([])
const particleCount = 80

const initParticles = () => {
  for (let i = 0; i < particleCount; i++) {
    particles.value.push({
      id: i,
      x: Math.random() * 100,
      y: Math.random() * 100,
      vx: (Math.random() - 0.5) * 0.3,
      vy: (Math.random() - 0.5) * 0.3,
      size: Math.random() * 3 + 1,
      hue: Math.random() * 60 + 220, // 紫蓝色调
      pulse: Math.random() * Math.PI * 2,
      pulseSpeed: 0.02 + Math.random() * 0.03
    })
  }
}

let animationFrame = null
const animate = () => {
  const time = Date.now() * 0.001
  
  particles.value.forEach(p => {
    // 基础运动
    p.x += p.vx
    p.y += p.vy
    
    // 边界反弹
    if (p.x < 0 || p.x > 100) p.vx *= -1
    if (p.y < 0 || p.y > 100) p.vy *= -1
    
    // 鼠标交互 - 轻迹效果
    const dx = (mouseX.value - p.x) * 0.01
    const dy = (mouseY.value - p.y) * 0.01
    p.vx += dx * 0.1
    p.vy += dy * 0.1
    
    // 速度限制
    const speed = Math.sqrt(p.vx * p.vx + p.vy * p.vy)
    if (speed > 0.5) {
      p.vx = (p.vx / speed) * 0.5
      p.vy = (p.vy / speed) * 0.5
    }
    
    // 脉冲动画
    p.pulse += p.pulseSpeed
  })
  
  // 计算连接线
  connections.value = []
  for (let i = 0; i < particles.value.length; i++) {
    for (let j = i + 1; j < particles.value.length; j++) {
      const p1 = particles.value[i]
      const p2 = particles.value[j]
      const dx = p1.x - p2.x
      const dy = p1.y - p2.y
      const dist = Math.sqrt(dx * dx + dy * dy)
      
      if (dist < 15) {
        connections.value.push({
          x1: p1.x,
          y1: p1.y,
          x2: p2.x,
          y2: p2.y,
          opacity: (1 - dist / 15) * 0.3
        })
      }
    }
  }
  
  animationFrame = requestAnimationFrame(animate)
}

// 浮动光球
const orbs = ref([
  { x: 20, y: 30, size: 300, hue: 260, speed: 0.5 },
  { x: 80, y: 70, size: 250, hue: 200, speed: 0.3 },
  { x: 50, y: 50, size: 200, hue: 320, speed: 0.4 }
])

const animateOrbs = () => {
  const time = Date.now() * 0.001
  orbs.value.forEach((orb, i) => {
    orb.x = 50 + Math.sin(time * orb.speed + i) * 30
    orb.y = 50 + Math.cos(time * orb.speed * 0.7 + i) * 30
  })
  requestAnimationFrame(animateOrbs)
}

// 鼠标移动处理
const handleMouseMove = (e) => {
  const rect = e.currentTarget.getBoundingClientRect()
  mouseX.value = ((e.clientX - rect.left) / rect.width) * 100
  mouseY.value = ((e.clientY - rect.top) / rect.height) * 100
}

// 初始化
onMounted(async () => {
  initParticles()
  animate()
  animateOrbs()
  await checkInstall()
  await fetchEnvInfo()
})

onUnmounted(() => {
  if (animationFrame) cancelAnimationFrame(animationFrame)
})

const checkInstall = async () => {
  try {
    const res = await installApi.check()
    if (res.code === 200) {
      Message.warning('系统已安装，即将跳转')
      setTimeout(() => router.push('/login'), 1500)
    }
  } catch (error) {
    console.error('检查安装状态失败:', error)
  }
}

const fetchEnvInfo = async () => {
  envLoading.value = true
  try {
    const res = await installApi.env()
    if (res.code === 200) envInfo.value = res.data
  } catch (error) {
    Message.error('获取环境信息失败')
  } finally {
    envLoading.value = false
  }
}

const nextStep = () => {
  if (currentStep.value === 0 && !licenseAgreed.value) {
    Message.warning('请先阅读并同意许可协议')
    return
  }
  if (currentStep.value === 1 && !envAllPass.value) {
    Message.error('环境检测未通过')
    return
  }
  if (currentStep.value < 5) currentStep.value++
}

const prevStep = () => {
  if (currentStep.value > 0) currentStep.value--
}

const doInstall = async () => {
  installLoading.value = true
  try {
    const data = {
      mysql_host: form.mysql_host,
      mysql_port: form.mysql_port,
      mysql_name: form.mysql_name,
      mysql_user: form.mysql_user,
      mysql_pwd: form.mysql_pwd,
      mysql_pre: form.mysql_pre,
      redis_host: form.redis_host,
      redis_port: form.redis_port,
      admin_user: form.admin_user,
      admin_pwd: form.admin_pwd,
      admin_authcode: form.admin_authcode,
      install_type: installType.value,
      tls_enabled: form.tls_enabled
    }
    if (form.redis_pwd) data.redis_pwd = form.redis_pwd
    if (installType.value === 'upgrade') {
      if (form.install_upgrade) data.install_upgrade = form.install_upgrade
      if (form.adm_pwd) data.adm_pwd = form.adm_pwd
    }
    if (form.cert_path) data.cert_path = form.cert_path
    if (form.key_path) data.key_path = form.key_path

    const res = await installApi.install(data)
    if (res.code === 200) {
      Message.success('安装成功！')
      currentStep.value = 5
    } else {
      Message.error(res.msg || '安装失败')
    }
  } catch (error) {
    Message.error('安装失败')
  } finally {
    installLoading.value = false
  }
}

const goToLogin = () => router.push('/login')

// 计算进度百分比
const progress = computed(() => (currentStep.value / (steps.length - 1)) * 100)
</script>

<template>
  <div class="install-universe" @mousemove="handleMouseMove">
    <!-- 星空背景 -->
    <div class="starfield">
      <div v-for="i in 100" :key="i" class="star" :style="{
        left: Math.random() * 100 + '%',
        top: Math.random() * 100 + '%',
        animationDelay: Math.random() * 3 + 's',
        animationDuration: (2 + Math.random() * 2) + 's'
      }" />
    </div>

    <!-- 粒子网络 -->
    <svg class="particle-network" viewBox="0 0 100 100" preserveAspectRatio="none">
      <defs>
        <linearGradient id="lineGradient" x1="0%" y1="0%" x2="100%" y2="100%">
          <stop offset="0%" stop-color="#6366f1" stop-opacity="0.5" />
          <stop offset="100%" stop-color="#8b5cf6" stop-opacity="0.5" />
        </linearGradient>
      </defs>
      <!-- 连接线 -->
      <line v-for="(conn, i) in connections" :key="'l'+i"
        :x1="conn.x1" :y1="conn.y1" :x2="conn.x2" :y2="conn.y2"
        stroke="url(#lineGradient)" :stroke-opacity="conn.opacity" stroke-width="0.1" />
      <!-- 粒子 -->
      <circle v-for="p in particles" :key="p.id"
        :cx="p.x" :cy="p.y" :r="p.size * 0.15"
        :fill="`hsl(${p.hue}, 80%, 60%)`"
        :opacity="0.6 + Math.sin(p.pulse) * 0.3">
        <animate attributeName="r" :values="`${p.size * 0.1};${p.size * 0.2};${p.size * 0.1}`" dur="2s" repeatCount="indefinite" />
      </circle>
    </svg>

    <!-- 流动光球 -->
    <div class="orb-container">
      <div v-for="(orb, i) in orbs" :key="i" class="glow-orb" :style="{
        left: orb.x + '%',
        top: orb.y + '%',
        width: orb.size + 'px',
        height: orb.size + 'px',
        background: `radial-gradient(circle, hsla(${orb.hue}, 100%, 60%, 0.3), transparent 70%)`,
        transform: `translate(-50%, -50%)`
      }" />
    </div>

    <!-- 光晕追踪鼠标 -->
    <div class="cursor-glow" :style="{
      left: mouseX + '%',
      top: mouseY + '%'
    }" />

    <!-- 主容器 -->
    <div class="install-container">
      <!-- 顶部进度条 -->
      <div class="progress-track">
        <div class="progress-fill" :style="{ width: progress + '%' }">
          <div class="progress-glow" />
        </div>
        <div class="progress-steps">
          <div v-for="(step, i) in steps" :key="i" 
               class="progress-dot"
               :class="{ active: currentStep === i, done: currentStep > i }"
               :style="{ '--step-color': step.color, '--step-glow': step.glow }">
            <span class="dot-emoji">{{ step.icon }}</span>
            <span class="dot-label">{{ step.title }}</span>
          </div>
        </div>
      </div>

      <!-- 内容区域 -->
      <div class="content-area">
        <transition name="page-flip" mode="out-in">
          <!-- 步骤0: 许可协议 -->
          <div v-if="currentStep === 0" key="license" class="page-panel">
            <div class="panel-hero">
              <div class="hero-icon">📜</div>
              <h1>开源许可协议</h1>
              <p>请仔细阅读以下条款</p>
            </div>
            
            <div class="license-scroll">
              <div class="license-card">
                <div class="license-badge">
                  <span class="badge-icon">⚖️</span>
                  <span class="badge-text">Apache License 2.0</span>
                </div>
                
                <div class="license-text">
                  <section>
                    <h3>🏛️ 版权声明</h3>
                    <p>Copyright © 2024 Nakamasa-Ichika Project. All rights reserved.</p>
                  </section>
                  
                  <section>
                    <h3>✅ 授权许可</h3>
                    <p>在遵守下列条件的前提下，您可以自由地：</p>
                    <ul>
                      <li>📱 <strong>使用</strong> - 在任何法律允许的范围内使用本软件</li>
                      <li>🔧 <strong>修改</strong> - 自由修改源代码以满足您的需求</li>
                      <li>📤 <strong>分发</strong> - 将原始或修改后的代码分发给他人</li>
                      <li>💼 <strong>商用</strong> - 将本软件用于商业目的</li>
                    </ul>
                  </section>
                  
                  <section>
                    <h3>⚠️ 免责声明</h3>
                    <div class="disclaimer-highlight">
                      <div class="disclaimer-icon">🛡️</div>
                      <div class="disclaimer-content">
                        <p><strong>本软件按"原样"提供，不附带任何明示或暗示的保证。</strong></p>
                        <p>在任何情况下，作者或版权持有人均不对任何索赔、损害或其他责任负责，无论是基于合同、侵权或其他法律理论。</p>
                        <div class="disclaimer-warning">
                          <span>⚠️</span>
                          <span>使用本软件所造成的任何直接或间接损失、数据丢失、系统故障或其他问题，均由用户自行承担，与项目开发者无关。</span>
                        </div>
                      </div>
                    </div>
                  </section>
                  
                  <section>
                    <h3>🏷️ 商标使用</h3>
                    <p>本许可不授予使用项目商标、服务标记或产品名称的权利。</p>
                  </section>
                </div>
                
                <div class="license-agree">
                  <label class="agree-checkbox" :class="{ checked: licenseAgreed }">
                    <input type="checkbox" v-model="licenseAgreed" />
                    <span class="checkbox-visual">
                      <span class="checkbox-check">✓</span>
                    </span>
                    <span class="checkbox-label">我已阅读并同意 Apache License 2.0 协议</span>
                  </label>
                </div>
              </div>
            </div>
          </div>

          <!-- 步骤1: 环境检测 -->
          <div v-else-if="currentStep === 1" key="env" class="page-panel">
            <div class="panel-hero">
              <div class="hero-icon">🔍</div>
              <h1>环境检测</h1>
              <p>正在扫描服务器环境</p>
            </div>
            
            <a-spin :loading="envLoading" class="env-spin">
              <div class="env-matrix">
                <div v-for="(item, i) in envChecks" :key="i" 
                     class="env-cell"
                     :class="{ pass: item.status, fail: !item.status }"
                     :style="{ animationDelay: i * 0.05 + 's' }">
                  <div class="cell-icon">{{ item.icon }}</div>
                  <div class="cell-info">
                    <span class="cell-name">{{ item.name }}</span>
                    <span class="cell-value">{{ item.value }}</span>
                  </div>
                  <div class="cell-status">
                    <span v-if="item.status" class="status-pass">✓</span>
                    <span v-else class="status-fail">✗</span>
                  </div>
                </div>
              </div>
              
              <div class="env-stats">
                <div class="stat-item total">
                  <div class="stat-ring">
                    <svg viewBox="0 0 36 36">
                      <circle cx="18" cy="18" r="16" fill="none" stroke="rgba(255,255,255,0.1)" stroke-width="2" />
                      <circle cx="18" cy="18" r="16" fill="none" stroke="#6366f1" stroke-width="2" 
                              stroke-dasharray="100, 100" />
                    </svg>
                    <span class="stat-value">{{ envChecks.length }}</span>
                  </div>
                  <span class="stat-label">检测项</span>
                </div>
                <div class="stat-item pass">
                  <div class="stat-ring">
                    <svg viewBox="0 0 36 36">
                      <circle cx="18" cy="18" r="16" fill="none" stroke="rgba(255,255,255,0.1)" stroke-width="2" />
                      <circle cx="18" cy="18" r="16" fill="none" stroke="#10b981" stroke-width="2"
                              :stroke-dasharray="`${(envChecks.filter(e => e.status).length / envChecks.length) * 100}, 100`" />
                    </svg>
                    <span class="stat-value">{{ envChecks.filter(e => e.status).length }}</span>
                  </div>
                  <span class="stat-label">通过</span>
                </div>
                <div class="stat-item fail">
                  <div class="stat-ring">
                    <svg viewBox="0 0 36 36">
                      <circle cx="18" cy="18" r="16" fill="none" stroke="rgba(255,255,255,0.1)" stroke-width="2" />
                      <circle cx="18" cy="18" r="16" fill="none" stroke="#ef4444" stroke-width="2"
                              :stroke-dasharray="`${(envChecks.filter(e => !e.status).length / envChecks.length) * 100}, 100`" />
                    </svg>
                    <span class="stat-value">{{ envChecks.filter(e => !e.status).length }}</span>
                  </div>
                  <span class="stat-label">失败</span>
                </div>
              </div>
            </a-spin>
          </div>

          <!-- 步骤2: MySQL配置 -->
          <div v-else-if="currentStep === 2" key="mysql" class="page-panel">
            <div class="panel-hero">
              <div class="hero-icon">💾</div>
              <h1>数据库配置</h1>
              <p>连接 MySQL 数据库</p>
            </div>
            
            <div class="config-block">
              <div class="block-header">
                <span class="header-icon">🐬</span>
                <span>MySQL 数据库</span>
              </div>
              <div class="block-form">
                <div class="form-row">
                  <div class="form-field flex-2">
                    <label>主机地址</label>
                    <div class="input-wrapper">
                      <span class="input-icon">🌐</span>
                      <input v-model="form.mysql_host" placeholder="127.0.0.1" />
                    </div>
                  </div>
                  <div class="form-field flex-1">
                    <label>端口</label>
                    <div class="input-wrapper">
                      <input type="number" v-model="form.mysql_port" />
                    </div>
                  </div>
                </div>
                <div class="form-row">
                  <div class="form-field">
                    <label>数据库名</label>
                    <div class="input-wrapper">
                      <span class="input-icon">📁</span>
                      <input v-model="form.mysql_name" placeholder="请输入数据库名" />
                    </div>
                  </div>
                </div>
                <div class="form-row">
                  <div class="form-field">
                    <label>用户名</label>
                    <div class="input-wrapper">
                      <span class="input-icon">👤</span>
                      <input v-model="form.mysql_user" placeholder="请输入用户名" />
                    </div>
                  </div>
                  <div class="form-field">
                    <label>密码</label>
                    <div class="input-wrapper">
                      <span class="input-icon">🔒</span>
                      <input type="password" v-model="form.mysql_pwd" placeholder="请输入密码" />
                    </div>
                  </div>
                </div>
                <div class="form-row">
                  <div class="form-field">
                    <label>表前缀</label>
                    <div class="input-wrapper">
                      <span class="input-icon">🏷️</span>
                      <input v-model="form.mysql_pre" placeholder="u_" />
                    </div>
                  </div>
                </div>
              </div>
            </div>
          </div>

          <!-- 步骤3: Redis配置 -->
          <div v-else-if="currentStep === 3" key="redis" class="page-panel">
            <div class="panel-hero">
              <div class="hero-icon">⚡</div>
              <h1>缓存服务</h1>
              <p>连接 Redis 缓存</p>
            </div>
            
            <div class="config-block">
              <div class="block-header redis">
                <span class="header-icon">🔴</span>
                <span>Redis 缓存</span>
              </div>
              <div class="block-form">
                <div class="form-row">
                  <div class="form-field flex-2">
                    <label>主机地址</label>
                    <div class="input-wrapper">
                      <span class="input-icon">🌐</span>
                      <input v-model="form.redis_host" placeholder="127.0.0.1" />
                    </div>
                  </div>
                  <div class="form-field flex-1">
                    <label>端口</label>
                    <div class="input-wrapper">
                      <input type="number" v-model="form.redis_port" />
                    </div>
                  </div>
                </div>
                <div class="form-row">
                  <div class="form-field">
                    <label>密码 <span class="optional">(可选)</span></label>
                    <div class="input-wrapper">
                      <span class="input-icon">🔒</span>
                      <input type="password" v-model="form.redis_pwd" placeholder="无密码留空" />
                    </div>
                  </div>
                </div>
              </div>
            </div>
          </div>

          <!-- 步骤4: 管理员配置 -->
          <div v-else-if="currentStep === 4" key="admin" class="page-panel">
            <div class="panel-hero">
              <div class="hero-icon">👤</div>
              <h1>管理员设置</h1>
              <p>创建系统管理员</p>
            </div>
            
            <div class="mode-switch">
              <button class="mode-btn" :class="{ active: installType === 'new' }" @click="installType = 'new'">
                <span>✨</span> 全新安装
              </button>
              <button class="mode-btn" :class="{ active: installType === 'upgrade' }" @click="installType = 'upgrade'">
                <span>⬆️</span> 升级安装
              </button>
            </div>
            
            <div class="config-block">
              <div class="block-header admin">
                <span class="header-icon">👑</span>
                <span>管理员账号</span>
              </div>
              <div class="block-form">
                <div class="form-row">
                  <div class="form-field">
                    <label>账号</label>
                    <div class="input-wrapper">
                      <span class="input-icon">👤</span>
                      <input v-model="form.admin_user" placeholder="5-12位" />
                    </div>
                  </div>
                  <div class="form-field">
                    <label>密码</label>
                    <div class="input-wrapper">
                      <span class="input-icon">🔑</span>
                      <input type="password" v-model="form.admin_pwd" placeholder="6-18位" />
                    </div>
                  </div>
                </div>
                <div class="form-row">
                  <div class="form-field">
                    <label>授权码</label>
                    <div class="input-wrapper">
                      <span class="input-icon">🎫</span>
                      <input v-model="form.admin_authcode" placeholder="16-32位" />
                    </div>
                  </div>
                </div>
                
                <template v-if="installType === 'upgrade'">
                  <div class="form-divider">升级配置</div>
                  <div class="form-row">
                    <div class="form-field">
                      <label>原版本</label>
                      <div class="input-wrapper">
                        <input v-model="form.install_upgrade" placeholder="如 3.2" />
                      </div>
                    </div>
                    <div class="form-field">
                      <label>密码密钥</label>
                      <div class="input-wrapper">
                        <input v-model="form.adm_pwd" placeholder="32位" />
                      </div>
                    </div>
                  </div>
                </template>
                
                <div class="form-divider">HTTPS 配置</div>
                <div class="tls-row">
                  <span>启用 TLS</span>
                  <label class="toggle-switch">
                    <input type="checkbox" v-model="form.tls_enabled" />
                    <span class="toggle-slider" />
                  </label>
                </div>
                <transition name="slide-down">
                  <div v-if="form.tls_enabled" class="form-row">
                    <div class="form-field">
                      <label>证书路径</label>
                      <div class="input-wrapper">
                        <input v-model="form.cert_path" placeholder="./certs/cert.pem" />
                      </div>
                    </div>
                    <div class="form-field">
                      <label>私钥路径</label>
                      <div class="input-wrapper">
                        <input v-model="form.key_path" placeholder="./certs/key.pem" />
                      </div>
                    </div>
                  </div>
                </transition>
              </div>
            </div>
          </div>

          <!-- 步骤5: 完成 -->
          <div v-else-if="currentStep === 5" key="finish" class="page-panel finish-panel">
            <div class="success-burst">
              <div class="burst-ring" v-for="i in 4" :key="i" :style="{ animationDelay: i * 0.15 + 's' }" />
              <div class="success-icon">
                <span>✓</span>
              </div>
            </div>
            <h1>安装成功！</h1>
            <p>系统已完成初始化，请重启服务后登录</p>
            <button class="finish-btn" @click="goToLogin">
              <span>前往登录</span>
              <span class="btn-arrow">→</span>
            </button>
          </div>
        </transition>
      </div>

      <!-- 底部导航 -->
      <div class="nav-bar" v-if="currentStep < 5">
        <button v-if="currentStep > 0" class="nav-btn back" @click="prevStep">
          <span class="btn-icon">←</span>
          <span>上一步</span>
        </button>
        <div class="nav-spacer" />
        <button v-if="currentStep < 4" class="nav-btn next" @click="nextStep">
          <span>下一步</span>
          <span class="btn-icon">→</span>
        </button>
        <button v-else class="nav-btn install" :disabled="installLoading" @click="doInstall">
          <span class="btn-icon">{{ installLoading ? '⏳' : '🚀' }}</span>
          <span>{{ installLoading ? '安装中...' : '开始安装' }}</span>
        </button>
      </div>
    </div>

    <!-- 版权 -->
    <div class="footer-copy">
      <span>Nakamasa-Ichika</span>
      <span>·</span>
      <a href="https://www.apache.org/licenses/LICENSE-2.0" target="_blank" rel="noopener noreferrer">Apache 2.0</a>
    </div>
  </div>
</template>

<style scoped lang="less">
* {
  box-sizing: border-box;
}

.install-universe {
  width: 100vw;
  height: 100vh;
  position: relative;
  overflow: hidden;
  background: linear-gradient(135deg, #0a0a1a 0%, #1a1a3a 50%, #0f0f2f 100%);
  font-family: 'Inter', -apple-system, sans-serif;
}

// 星空背景
.starfield {
  position: absolute;
  inset: 0;
  pointer-events: none;
}

.star {
  position: absolute;
  width: 2px;
  height: 2px;
  background: white;
  border-radius: 50%;
  animation: twinkle 3s ease-in-out infinite;
}

@keyframes twinkle {
  0%, 100% { opacity: 0.2; transform: scale(1); }
  50% { opacity: 1; transform: scale(1.5); }
}

// 粒子网络
.particle-network {
  position: absolute;
  inset: 0;
  width: 100%;
  height: 100%;
  pointer-events: none;
  opacity: 0.8;
}

// 光球
.orb-container {
  position: absolute;
  inset: 0;
  pointer-events: none;
}

.glow-orb {
  position: absolute;
  border-radius: 50%;
  filter: blur(60px);
  mix-blend-mode: screen;
  animation: orbPulse 4s ease-in-out infinite;
}

@keyframes orbPulse {
  0%, 100% { opacity: 0.4; transform: translate(-50%, -50%) scale(1); }
  50% { opacity: 0.6; transform: translate(-50%, -50%) scale(1.1); }
}

// 鼠标追踪光晕
.cursor-glow {
  position: absolute;
  width: 300px;
  height: 300px;
  transform: translate(-50%, -50%);
  background: radial-gradient(circle, rgba(99, 102, 241, 0.15), transparent 70%);
  pointer-events: none;
  transition: left 0.1s, top 0.1s;
}

// 主容器
.install-container {
  position: absolute;
  inset: 40px;
  display: flex;
  flex-direction: column;
  background: rgba(20, 20, 40, 0.7);
  backdrop-filter: blur(30px);
  border-radius: 24px;
  border: 1px solid rgba(255, 255, 255, 0.08);
  box-shadow: 0 30px 100px rgba(0, 0, 0, 0.5),
              inset 0 1px 0 rgba(255, 255, 255, 0.05);
  overflow: hidden;
}

// 进度轨道
.progress-track {
  padding: 20px 40px;
  border-bottom: 1px solid rgba(255, 255, 255, 0.05);
  background: rgba(0, 0, 0, 0.2);
}

.progress-fill {
  height: 3px;
  background: linear-gradient(90deg, #6366f1, #8b5cf6, #a855f7);
  border-radius: 2px;
  position: relative;
  transition: width 0.5s cubic-bezier(0.4, 0, 0.2, 1);
}

.progress-glow {
  position: absolute;
  right: 0;
  top: -3px;
  width: 20px;
  height: 9px;
  background: linear-gradient(90deg, transparent, rgba(167, 139, 250, 0.8));
  filter: blur(4px);
}

.progress-steps {
  display: flex;
  justify-content: space-between;
  margin-top: 12px;
}

.progress-dot {
  display: flex;
  flex-direction: column;
  align-items: center;
  gap: 6px;
  opacity: 0.4;
  transition: all 0.3s ease;
  
  &.active, &.done {
    opacity: 1;
  }
  
  &.active .dot-emoji {
    transform: scale(1.2);
    filter: drop-shadow(0 0 10px var(--step-glow));
  }
}

.dot-emoji {
  font-size: 20px;
  transition: all 0.3s ease;
}

.dot-label {
  font-size: 11px;
  color: rgba(255, 255, 255, 0.6);
  text-transform: uppercase;
  letter-spacing: 0.5px;
}

// 内容区域
.content-area {
  flex: 1;
  overflow-y: auto;
  padding: 30px 40px;
  
  &::-webkit-scrollbar { width: 6px; }
  &::-webkit-scrollbar-track { background: transparent; }
  &::-webkit-scrollbar-thumb { background: rgba(255, 255, 255, 0.1); border-radius: 3px; }
}

.page-panel {
  max-width: 700px;
  margin: 0 auto;
}

.panel-hero {
  text-align: center;
  margin-bottom: 30px;
}

.hero-icon {
  font-size: 48px;
  margin-bottom: 16px;
  display: inline-block;
  animation: float 3s ease-in-out infinite;
}

@keyframes float {
  0%, 100% { transform: translateY(0); }
  50% { transform: translateY(-10px); }
}

.panel-hero h1 {
  font-size: 28px;
  font-weight: 700;
  color: white;
  margin: 0 0 8px;
  background: linear-gradient(135deg, #fff, #a5b4fc);
  -webkit-background-clip: text;
  -webkit-text-fill-color: transparent;
}

.panel-hero p {
  font-size: 14px;
  color: rgba(255, 255, 255, 0.5);
  margin: 0;
}

// 许可证样式
.license-scroll {
  max-height: 380px;
  overflow-y: auto;
  padding-right: 10px;
  
  &::-webkit-scrollbar { width: 4px; }
  &::-webkit-scrollbar-thumb { background: rgba(255, 255, 255, 0.2); border-radius: 2px; }
}

.license-card {
  background: rgba(255, 255, 255, 0.03);
  border: 1px solid rgba(255, 255, 255, 0.06);
  border-radius: 16px;
  padding: 24px;
}

.license-badge {
  display: flex;
  align-items: center;
  gap: 10px;
  padding-bottom: 16px;
  border-bottom: 1px solid rgba(255, 255, 255, 0.06);
  margin-bottom: 20px;
  
  .badge-icon { font-size: 24px; }
  .badge-text {
    font-size: 18px;
    font-weight: 600;
    color: #fbbf24;
  }
}

.license-text {
  color: rgba(255, 255, 255, 0.75);
  font-size: 13px;
  line-height: 1.7;
  
  section { margin-bottom: 20px; }
  h3 {
    font-size: 14px;
    color: white;
    margin: 0 0 10px;
  }
  p { margin: 8px 0; }
  ul {
    list-style: none;
    padding: 0;
    margin: 12px 0;
    li {
      padding: 6px 0;
      display: flex;
      align-items: flex-start;
      gap: 8px;
    }
  }
}

.disclaimer-highlight {
  display: flex;
  gap: 16px;
  padding: 16px;
  background: rgba(239, 68, 68, 0.1);
  border: 1px solid rgba(239, 68, 68, 0.2);
  border-radius: 12px;
  margin-top: 12px;
}

.disclaimer-icon {
  font-size: 28px;
}

.disclaimer-content p { margin: 6px 0; }

.disclaimer-warning {
  display: flex;
  align-items: flex-start;
  gap: 8px;
  margin-top: 12px;
  padding: 12px;
  background: rgba(239, 68, 68, 0.15);
  border-radius: 8px;
  font-weight: 500;
}

.license-agree {
  padding-top: 20px;
  border-top: 1px solid rgba(255, 255, 255, 0.06);
  margin-top: 20px;
}

.agree-checkbox {
  display: flex;
  align-items: center;
  gap: 12px;
  cursor: pointer;
  color: rgba(255, 255, 255, 0.8);
  
  input { display: none; }
  
  .checkbox-visual {
    width: 24px;
    height: 24px;
    border: 2px solid rgba(255, 255, 255, 0.3);
    border-radius: 6px;
    display: flex;
    align-items: center;
    justify-content: center;
    transition: all 0.2s ease;
  }
  
  .checkbox-check {
    opacity: 0;
    transform: scale(0);
    transition: all 0.2s ease;
    color: white;
  }
  
  &.checked .checkbox-visual {
    background: linear-gradient(135deg, #6366f1, #8b5cf6);
    border-color: transparent;
  }
  
  &.checked .checkbox-check {
    opacity: 1;
    transform: scale(1);
  }
}

// 环境检测
.env-spin { width: 100%; }

.env-matrix {
  display: grid;
  grid-template-columns: repeat(3, 1fr);
  gap: 12px;
  margin-bottom: 24px;
}

.env-cell {
  display: flex;
  align-items: center;
  gap: 12px;
  padding: 14px;
  background: rgba(255, 255, 255, 0.03);
  border: 1px solid rgba(255, 255, 255, 0.06);
  border-radius: 12px;
  animation: cellIn 0.4s ease-out backwards;
  
  &.pass { border-color: rgba(16, 185, 129, 0.3); }
  &.fail { border-color: rgba(239, 68, 68, 0.3); }
}

@keyframes cellIn {
  from { opacity: 0; transform: translateY(10px); }
  to { opacity: 1; transform: translateY(0); }
}

.cell-icon { font-size: 24px; }

.cell-info {
  flex: 1;
  .cell-name {
    display: block;
    font-size: 12px;
    color: rgba(255, 255, 255, 0.5);
  }
  .cell-value {
    display: block;
    font-size: 14px;
    color: white;
    font-weight: 500;
  }
}

.cell-status {
  font-size: 18px;
  .status-pass { color: #10b981; }
  .status-fail { color: #ef4444; }
}

.env-stats {
  display: flex;
  justify-content: center;
  gap: 40px;
  padding: 20px;
  background: rgba(255, 255, 255, 0.02);
  border-radius: 12px;
}

.stat-item {
  text-align: center;
  
  &.pass .stat-value { color: #10b981; }
  &.fail .stat-value { color: #ef4444; }
}

.stat-ring {
  position: relative;
  width: 60px;
  height: 60px;
  margin: 0 auto 8px;
  
  svg {
    transform: rotate(-90deg);
    width: 100%;
    height: 100%;
  }
  
  .stat-value {
    position: absolute;
    inset: 0;
    display: flex;
    align-items: center;
    justify-content: center;
    font-size: 16px;
    font-weight: 700;
    color: white;
  }
}

.stat-label {
  font-size: 12px;
  color: rgba(255, 255, 255, 0.5);
}

// 配置块
.config-block {
  background: rgba(255, 255, 255, 0.03);
  border: 1px solid rgba(255, 255, 255, 0.06);
  border-radius: 16px;
  overflow: hidden;
}

.block-header {
  display: flex;
  align-items: center;
  gap: 10px;
  padding: 16px 20px;
  background: linear-gradient(90deg, rgba(245, 158, 11, 0.1), transparent);
  border-bottom: 1px solid rgba(255, 255, 255, 0.06);
  font-weight: 600;
  color: white;
  
  &.redis { background: linear-gradient(90deg, rgba(239, 68, 68, 0.1), transparent); }
  &.admin { background: linear-gradient(90deg, rgba(139, 92, 246, 0.1), transparent); }
  
  .header-icon { font-size: 20px; }
}

.block-form {
  padding: 20px;
}

.form-row {
  display: flex;
  gap: 16px;
  margin-bottom: 16px;
  
  &:last-child { margin-bottom: 0; }
}

.form-field {
  flex: 1;
  
  &.flex-2 { flex: 2; }
  &.flex-1 { flex: 1; }
}

.form-field label {
  display: block;
  font-size: 12px;
  color: rgba(255, 255, 255, 0.6);
  margin-bottom: 6px;
  
  .optional {
    color: rgba(255, 255, 255, 0.3);
    font-weight: normal;
  }
}

.input-wrapper {
  display: flex;
  align-items: center;
  background: rgba(255, 255, 255, 0.05);
  border: 1px solid rgba(255, 255, 255, 0.1);
  border-radius: 10px;
  padding: 0 12px;
  transition: all 0.2s ease;
  
  &:focus-within {
    border-color: rgba(99, 102, 241, 0.5);
    background: rgba(255, 255, 255, 0.08);
    box-shadow: 0 0 0 3px rgba(99, 102, 241, 0.1);
  }
  
  .input-icon {
    font-size: 16px;
    margin-right: 8px;
    opacity: 0.5;
  }
  
  input {
    flex: 1;
    background: transparent;
    border: none;
    outline: none;
    color: white;
    font-size: 14px;
    padding: 12px 0;
    width: 100%;
    
    &::placeholder { color: rgba(255, 255, 255, 0.25); }
  }
}

.form-divider {
  text-align: center;
  font-size: 12px;
  color: rgba(255, 255, 255, 0.4);
  padding: 16px 0;
  margin: 8px 0;
  border-top: 1px dashed rgba(255, 255, 255, 0.1);
  border-bottom: 1px dashed rgba(255, 255, 255, 0.1);
}

// 模式切换
.mode-switch {
  display: flex;
  gap: 12px;
  margin-bottom: 20px;
}

.mode-btn {
  flex: 1;
  display: flex;
  align-items: center;
  justify-content: center;
  gap: 8px;
  padding: 14px;
  background: rgba(255, 255, 255, 0.03);
  border: 2px solid rgba(255, 255, 255, 0.08);
  border-radius: 12px;
  color: rgba(255, 255, 255, 0.5);
  font-size: 14px;
  cursor: pointer;
  transition: all 0.2s ease;
  
  &:hover {
    background: rgba(255, 255, 255, 0.05);
    border-color: rgba(255, 255, 255, 0.15);
  }
  
  &.active {
    background: rgba(99, 102, 241, 0.1);
    border-color: #6366f1;
    color: white;
  }
}

// TLS 开关
.tls-row {
  display: flex;
  align-items: center;
  justify-content: space-between;
  padding: 12px 16px;
  background: rgba(255, 255, 255, 0.02);
  border-radius: 10px;
  color: rgba(255, 255, 255, 0.8);
  margin-bottom: 16px;
}

.toggle-switch {
  position: relative;
  width: 48px;
  height: 26px;
  
  input { display: none; }
  
  .toggle-slider {
    position: absolute;
    inset: 0;
    background: rgba(255, 255, 255, 0.1);
    border-radius: 13px;
    cursor: pointer;
    transition: background 0.2s ease;
    
    &::before {
      content: '';
      position: absolute;
      left: 3px;
      top: 3px;
      width: 20px;
      height: 20px;
      background: white;
      border-radius: 50%;
      transition: transform 0.2s ease;
    }
  }
  
  input:checked + .toggle-slider {
    background: linear-gradient(135deg, #6366f1, #8b5cf6);
    
    &::before { transform: translateX(22px); }
  }
}

// 完成页面
.finish-panel {
  display: flex;
  flex-direction: column;
  align-items: center;
  justify-content: center;
  text-align: center;
  min-height: 400px;
}

.success-burst {
  position: relative;
  width: 120px;
  height: 120px;
  margin-bottom: 30px;
}

.burst-ring {
  position: absolute;
  inset: 0;
  border: 2px solid #10b981;
  border-radius: 50%;
  animation: burst 1.5s ease-out infinite;
}

@keyframes burst {
  0% { transform: scale(0.5); opacity: 1; }
  100% { transform: scale(1.5); opacity: 0; }
}

.success-icon {
  position: absolute;
  inset: 20px;
  display: flex;
  align-items: center;
  justify-content: center;
  background: linear-gradient(135deg, #10b981, #059669);
  border-radius: 50%;
  color: white;
  font-size: 32px;
  font-weight: bold;
  box-shadow: 0 10px 40px rgba(16, 185, 129, 0.4);
}

.finish-panel h1 {
  font-size: 32px;
  font-weight: 700;
  color: white;
  margin: 0 0 12px;
}

.finish-panel p {
  font-size: 14px;
  color: rgba(255, 255, 255, 0.5);
  margin: 0 0 30px;
}

.finish-btn {
  display: flex;
  align-items: center;
  gap: 10px;
  padding: 16px 32px;
  background: linear-gradient(135deg, #6366f1, #8b5cf6);
  border: none;
  border-radius: 14px;
  color: white;
  font-size: 16px;
  font-weight: 600;
  cursor: pointer;
  transition: all 0.2s ease;
  
  &:hover {
    transform: translateY(-2px);
    box-shadow: 0 10px 30px rgba(99, 102, 241, 0.4);
  }
  
  .btn-arrow {
    transition: transform 0.2s ease;
  }
  
  &:hover .btn-arrow {
    transform: translateX(4px);
  }
}

// 底部导航
.nav-bar {
  display: flex;
  align-items: center;
  gap: 16px;
  padding: 20px 40px;
  border-top: 1px solid rgba(255, 255, 255, 0.06);
  background: rgba(0, 0, 0, 0.2);
}

.nav-spacer { flex: 1; }

.nav-btn {
  display: flex;
  align-items: center;
  gap: 8px;
  padding: 12px 24px;
  background: rgba(255, 255, 255, 0.05);
  border: 1px solid rgba(255, 255, 255, 0.1);
  border-radius: 12px;
  color: white;
  font-size: 14px;
  font-weight: 500;
  cursor: pointer;
  transition: all 0.2s ease;
  
  &:hover {
    background: rgba(255, 255, 255, 0.1);
    border-color: rgba(255, 255, 255, 0.2);
  }
  
  &.next, &.install {
    background: linear-gradient(135deg, #6366f1, #8b5cf6);
    border: none;
    
    &:hover {
      transform: translateY(-2px);
      box-shadow: 0 8px 25px rgba(99, 102, 241, 0.4);
    }
  }
  
  &:disabled {
    opacity: 0.6;
    cursor: not-allowed;
  }
  
  .btn-icon { font-size: 16px; }
}

// 版权
.footer-copy {
  position: absolute;
  bottom: 15px;
  left: 50%;
  transform: translateX(-50%);
  display: flex;
  align-items: center;
  gap: 8px;
  font-size: 12px;
  color: rgba(255, 255, 255, 0.3);
  
  a {
    color: rgba(255, 255, 255, 0.5);
    text-decoration: none;
    transition: color 0.2s;
    
    &:hover { color: #6366f1; }
  }
}

// 过渡动画
.page-flip-enter-active {
  animation: pageIn 0.4s cubic-bezier(0.4, 0, 0.2, 1);
}

.page-flip-leave-active {
  animation: pageOut 0.3s cubic-bezier(0.4, 0, 0.2, 1);
}

@keyframes pageIn {
  from { opacity: 0; transform: translateX(30px) scale(0.98); }
  to { opacity: 1; transform: translateX(0) scale(1); }
}

@keyframes pageOut {
  from { opacity: 1; transform: translateX(0) scale(1); }
  to { opacity: 0; transform: translateX(-30px) scale(0.98); }
}

.slide-down-enter-active,
.slide-down-leave-active {
  transition: all 0.3s ease;
  overflow: hidden;
}

.slide-down-enter-from,
.slide-down-leave-to {
  opacity: 0;
  max-height: 0;
  margin-top: 0;
}

.slide-down-enter-to,
.slide-down-leave-from {
  max-height: 100px;
  margin-top: 16px;
}

// 响应式
@media (max-width: 768px) {
  .install-container { inset: 10px; }
  
  .progress-track { padding: 15px 20px; }
  .content-area { padding: 20px; }
  .nav-bar { padding: 15px 20px; }
  
  .env-matrix { grid-template-columns: repeat(2, 1fr); }
  
  .progress-dot:not(.active) .dot-label { display: none; }
}
</style>
