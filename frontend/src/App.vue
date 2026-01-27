<template>
  <div class="app-container">
    <TitleBar />
    <!-- 顶部标题栏 -->
    <el-card class="top-bar" shadow="hover">
      <div class="top-bar-content">
        <h1>{{ $t('app.title') }}</h1>
        <div class="top-bar-right">
          <div class="theme-control">
            <LanguageSelector />
            <el-switch
              v-model="autoThemeEnabled"
              @change="handleAutoThemeChange"
              :active-text="$t('app.autoTheme')"
              :inactive-text="$t('app.manualTheme')"
              size="small"
              class="auto-theme-switch"
            />
            <el-button 
              @click="toggleTheme" 
              circle
              class="theme-btn"
              :title="autoThemeEnabled ? $t('app.autoTheme') : (isDark ? $t('app.manualTheme') : $t('app.manualTheme'))"
            >
              <el-icon><Sunny v-if="isDark" /><Moon v-else /></el-icon>
            </el-button>
          </div>
          <div class="status-control">
            <span class="status-label">{{ $t('app.status') }}</span>
            <el-tag :type="status === 'running' ? 'success' : 'info'" effect="dark" class="status-badge">
              {{ status === 'running' ? $t('app.running') : $t('app.stopped') }}
            </el-tag>
            <span v-if="status === 'running' && runTime" class="runtime-text">
              ({{ $t('app.runtime', { time: runTime }) }})
            </span>
            <el-button 
              @click="status==='running'?stop():start()" 
              :loading="starting"
              :type="status === 'running' ? 'danger' : 'primary'"
              class="control-btn"
            >
              {{ status === 'running' ? $t('app.stopService') : $t('app.startService') }}
            </el-button>
            <el-tooltip 
              :content="$t('app.saveConfigHint')" 
              placement="bottom" 
              :disabled="status==='stopped'">
              <span>
                <el-button 
                  @click="handleSaveConfig" 
                  :loading="saving"
                  :disabled="status!=='stopped' || saving || starting"
                  type="primary"
                  class="save-btn"
                >
                  <el-icon><Check /></el-icon> {{ saving ? $t('app.saving') : $t('app.saveConfig') }}
                </el-button>
              </span>
            </el-tooltip>
          </div>
        </div>
      </div>
    </el-card>

    <!-- 主内容区域：侧边栏 + 内容区 -->
    <div class="main-content">
      <Sidebar 
        :is-collapsed="isCollapsed" 
        :active-tab="activeTab" 
        @toggle-sidebar="toggleSidebar" 
        @select-menu="handleMenuSelect"
      />

      <!-- 内容区域 -->
      <div class="content-area">
        <BaseConfig v-show="activeTab === 'base'" ref="baseConfigRef" />
        <ConfigCard 
          v-show="activeTab === 'config'" 
          ref="configCardRef"
        />
        <WsProxyConfig v-show="activeTab === 'ws'" ref="wsProxyConfigRef" />
        <StreamProxyConfig v-show="activeTab === 'stream'" ref="streamProxyConfigRef" />
        <Dashboard :is-active="activeTab === 'dashboard'" v-show="activeTab === 'dashboard'" />
        <AccessControl 
          v-show="activeTab === 'access'"
          ref="accessControlRef"
          :config="globalConfig"
        />
        <MetricsStorage 
          v-show="activeTab === 'storage'"
          ref="metricsStorageRef"
          :config="globalConfig"
        />
        <RequestLogs v-show="activeTab === 'requestLogs'" />
        <LogViewer v-show="activeTab === 'logs'" />
        <About v-show="activeTab === 'about'" ref="aboutRef" />
      </div>
    </div>

    <!-- 使用条款对话框（首次启动，要求必须接受） -->
    <TermsDialog v-if="showTermsDialog" :require-accept="true" @close="handleTermsAccepted" />
  </div>
</template>

<script setup lang="ts">
import { ref, computed, onMounted, onBeforeUnmount, watch } from 'vue'
import { StartServer, StopServer, GetStatus, QuitApp, EventsOn, SetTrayProxyState } from './api'
import { openUrl } from '@tauri-apps/plugin-opener'
import { enable as enableAutostart, disable as disableAutostart, isEnabled as isAutostartEnabled } from '@tauri-apps/plugin-autostart'
import TitleBar from './components/TitleBar.vue'
import BaseConfig from './components/BaseConfig.vue'
import ConfigCard from './components/ConfigCard.vue'
import WsProxyConfig from './components/WsProxyConfig.vue'
import StreamProxyConfig from './components/StreamProxyConfig.vue'
import LogViewer from './components/LogViewer.vue'
import Dashboard from './components/Dashboard.vue'
import AccessControl from './components/AccessControl.vue'
import MetricsStorage from './components/MetricsStorage.vue'
import RequestLogs from './components/RequestLogs.vue'
import About from './components/About.vue'
import Sidebar from './components/Sidebar.vue'
import TermsDialog from './components/TermsDialog.vue'
import LanguageSelector from './components/LanguageSelector.vue'
import { Sunny, Moon, Check } from '@element-plus/icons-vue'
import { ElMessage, ElMessageBox, ElNotification } from 'element-plus'
import { GetConfig, SaveConfig } from './api'
import { GetTermsAccepted } from './api'
import { useI18n } from 'vue-i18n'

const { t, locale } = useI18n()

const activeTab = ref<'base' | 'config' | 'ws' | 'stream' | 'logs' | 'dashboard' | 'access' | 'storage' | 'requestLogs' | 'about'>('config')
const status = ref('stopped')
const starting = ref(false)
const saving = ref(false)
const baseConfigRef = ref<InstanceType<typeof BaseConfig> | null>(null)
const configCardRef = ref<InstanceType<typeof ConfigCard> | null>(null)
const wsProxyConfigRef = ref<InstanceType<typeof WsProxyConfig> | null>(null)
const streamProxyConfigRef = ref<InstanceType<typeof StreamProxyConfig> | null>(null)
const accessControlRef = ref<InstanceType<typeof AccessControl> | null>(null)
const metricsStorageRef = ref<InstanceType<typeof MetricsStorage> | null>(null)
const aboutRef = ref<InstanceType<typeof About> | null>(null)
const globalConfig = ref<any>({})
const showTermsDialog = ref(false)

// 运行时间相关
const startTime = ref<number | null>(null)
const currentTime = ref<number>(Date.now())
let runtimeTimer: number | null = null

// 格式化运行时间
const runTime = computed(() => {
  if (!startTime.value || status.value !== 'running') {
    return null
  }
  const elapsed = Math.floor((currentTime.value - startTime.value) / 1000) // 秒
  const days = Math.floor(elapsed / 86400)
  const hours = Math.floor((elapsed % 86400) / 3600)
  const minutes = Math.floor((elapsed % 3600) / 60)
  const seconds = elapsed % 60
  
  const formatTimeUnit = (value: number, unitKey: string, pluralKey?: string) => {
    if (locale.value === 'en-US') {
      // 英文：使用空格，处理复数
      const unit = (pluralKey && value !== 1) ? pluralKey : unitKey
      return `${value} ${t(`app.timeUnit.${unit}`)}`
    } else {
      // 中文：直接连接，无空格
      return `${value}${t(`app.timeUnit.${unitKey}`)}`
    }
  }
  
  const parts: string[] = []
  if (days > 0) {
    parts.push(formatTimeUnit(days, 'day', 'days'))
  }
  if (hours > 0 || days > 0) {
    parts.push(formatTimeUnit(hours, 'hour', 'hours'))
  }
  if (minutes > 0 || hours > 0 || days > 0) {
    parts.push(formatTimeUnit(minutes, 'minute', 'minutes'))
  }
  parts.push(formatTimeUnit(seconds, 'second', 'seconds'))
  
  // 中文直接连接，英文用空格连接
  const separator = locale.value === 'en-US' ? ' ' : ''
  return parts.join(separator)
})

// 启动运行时间计时器
const startRuntimeTimer = () => {
  if (runtimeTimer) {
    clearInterval(runtimeTimer)
  }
  runtimeTimer = window.setInterval(() => {
    if (status.value === 'running' && startTime.value) {
      currentTime.value = Date.now()
    }
  }, 1000) // 每秒更新一次
}

// 停止运行时间计时器
const stopRuntimeTimer = () => {
  if (runtimeTimer) {
    clearInterval(runtimeTimer)
    runtimeTimer = null
  }
}

// 全局主题状态
const isDark = ref(true)

// 自动切换主题开关
const autoThemeEnabled = ref(true)


// 自动切换主题的定时器
let autoThemeTimer: number | null = null

// 根据时间判断是否应该使用夜间模式
const shouldUseDarkMode = (): boolean => {
  const hour = new Date().getHours()
  // 18:00 (晚上6点) 到 6:00 (早上6点) 使用夜间模式
  return hour >= 18 || hour < 6
}

// 检查并自动切换主题
const checkAndAutoSwitchTheme = () => {
  // 只有在自动切换开启时才自动切换
  if (autoThemeEnabled.value) {
    const shouldDark = shouldUseDarkMode()
    if (isDark.value !== shouldDark) {
      isDark.value = shouldDark
      applyTheme()
      console.log(`自动切换主题: ${shouldDark ? '夜间模式' : '日间模式'}`)
    }
  }
}

// 启动自动主题切换
const startAutoTheme = () => {
  // 清除旧的定时器
  if (autoThemeTimer) {
    clearInterval(autoThemeTimer)
  }
  
  // 如果自动切换已启用，才启动定时器
  if (autoThemeEnabled.value) {
    // 立即检查一次
    checkAndAutoSwitchTheme()
    
    // 每60秒检查一次（更频繁的检查，确保及时切换）
    autoThemeTimer = window.setInterval(() => {
      checkAndAutoSwitchTheme()
    }, 60000) // 60秒检查一次
  }
}

// 停止自动主题切换
const stopAutoTheme = () => {
  if (autoThemeTimer) {
    clearInterval(autoThemeTimer)
    autoThemeTimer = null
  }
}

// 侧边栏折叠状态
const isCollapsed = ref(false)

// 切换侧边栏折叠状态
const toggleSidebar = () => {
  isCollapsed.value = !isCollapsed.value
  // 保存到 localStorage
  localStorage.setItem('sidebarCollapsed', String(isCollapsed.value))
}

// 加载侧边栏折叠状态
const loadSidebarState = () => {
  const saved = localStorage.getItem('sidebarCollapsed')
  if (saved !== null) {
    isCollapsed.value = saved === 'true'
  }
}

// 读取并应用主题
const loadTheme = () => {
  // 加载自动切换开关状态
  const savedAutoTheme = localStorage.getItem('autoThemeEnabled')
  if (savedAutoTheme !== null) {
    autoThemeEnabled.value = savedAutoTheme === 'true'
  }
  
  // 如果自动切换开启，根据时间判断
  if (autoThemeEnabled.value) {
    isDark.value = shouldUseDarkMode()
  } else {
    // 手动模式：使用保存的主题
    const savedTheme = localStorage.getItem('theme')
    if (savedTheme) {
      isDark.value = savedTheme === 'dark'
    } else {
      // 默认使用夜间模式
      isDark.value = true
    }
  }
  applyTheme()
}

// 应用主题
const applyTheme = () => {
  document.documentElement.classList.toggle('light-mode', !isDark.value)
}

// 切换主题
const toggleTheme = () => {
  // 如果自动切换开启，关闭自动切换并切换到手动模式
  if (autoThemeEnabled.value) {
    autoThemeEnabled.value = false
    localStorage.setItem('autoThemeEnabled', 'false')
    stopAutoTheme()
  }
  
  isDark.value = !isDark.value
  // 保存当前主题
  localStorage.setItem('theme', isDark.value ? 'dark' : 'light')
  applyTheme()
}

// 处理自动切换开关变化
const handleAutoThemeChange = (enabled: boolean | string | number) => {
  const isEnabled = enabled === true || enabled === 'true' || enabled === 1
  localStorage.setItem('autoThemeEnabled', String(isEnabled))
  
  if (isEnabled) {
    // 开启自动切换：根据时间设置主题
    isDark.value = shouldUseDarkMode()
    localStorage.setItem('theme', 'auto')
    applyTheme()
    startAutoTheme()
    ElMessage.success(t('app.autoThemeEnabled'))
  } else {
    // 关闭自动切换：停止定时器，保持当前主题
    stopAutoTheme()
    // 保存当前主题为手动模式
    localStorage.setItem('theme', isDark.value ? 'dark' : 'light')
    ElMessage.info(t('app.autoThemeDisabled'))
  }
}

const start = async () => {
  // 防止重复点击导致状态/事件竞争
  if (starting.value) return

  starting.value = true
  try {
    await StartServer()

    // 后端 start_server 目前是"异步启动"：即使端口占用，StartServer 也可能先返回 Ok。
    // 因此这里不能立即提示"启动成功"，而是进入等待状态。
    status.value = 'stopped'
    ElMessage.info(t('app.startRequestSent'))

    // 轮询等待一小段时间，直到真正 running 才提示成功；
    // 如果后端启动失败，会通过 server-start-error 事件提示，并保持 stopped。
    const deadline = Date.now() + 5000
    while (Date.now() < deadline) {
      try {
        const s = await GetStatus()
        status.value = s as any
        if (status.value === 'running') {
          startTime.value = Date.now()
          currentTime.value = Date.now()
          startRuntimeTimer()
          ElMessage.success(t('app.serviceStarted'))
          return
        }
      } catch {
        // ignore and continue
      }
      await new Promise((r) => setTimeout(r, 200))
    }

    // 超时不代表失败，只是不确定；保持等待，后续靠 status/server-start-error 事件更新
  } catch (e: any) {
    ElMessage.error(t('app.startFailed', { error: e?.message || String(e) }))
  } finally {
    starting.value = false
  }
}

const stop = async () => {
  try {
    await StopServer()
    status.value = 'stopped'
    // 重置启动时间
    startTime.value = null
    stopRuntimeTimer()
    ElMessage.success(t('app.serviceStopped'))
  } catch (e: any) {
    ElMessage.error(t('app.stopFailed', { error: e?.message || String(e) }))
  }
}


// 保存配置
const handleSaveConfig = async () => {
  saving.value = true
  try {
    // 从 ConfigCard 获取配置
    let configCardConfig = {}
    try {
      if (!configCardRef.value) {
        throw new Error('ConfigCard 组件未加载')
      }
      configCardConfig = configCardRef.value.getConfig() || {}

      // 从 WsProxyConfig 获取配置
      if (wsProxyConfigRef.value) {
        const wsCfg = wsProxyConfigRef.value.getConfig() || {}
        configCardConfig = {
          ...configCardConfig,
          ...wsCfg,
        }
      }

      // 从 StreamProxyConfig 获取配置
      if (streamProxyConfigRef.value) {
        const streamCfg = streamProxyConfigRef.value.getConfig() || {}
        configCardConfig = {
          ...configCardConfig,
          ...streamCfg,
        }
      }

      // 从 BaseConfig 获取配置（基础配置）
      if (baseConfigRef.value) {
        const baseCfg = baseConfigRef.value.getConfig() || {}
        configCardConfig = {
          ...baseCfg,
          ...configCardConfig,
        }
      }
    } catch (e: any) {
      ElMessage.error(t('app.configValidationFailed', { error: e?.message || String(e) }))
      saving.value = false
      return
    }
    
    // 从 AccessControl 获取配置
    let accessConfig = {}
    try {
      if (accessControlRef.value) {
        accessConfig = accessControlRef.value.getConfig() || {}
      } else {
        accessConfig = {}
      }
    } catch (e: any) {
      accessConfig = {}
    }
    
    // 从 MetricsStorage 获取配置
    let storageConfig = {}
    try {
      if (metricsStorageRef.value) {
        storageConfig = metricsStorageRef.value.getConfig() || {}
      } else {
        storageConfig = {}
      }
    } catch (e: any) {
      storageConfig = {}
    }

    // 从 About 获取配置
    let aboutConfig = {}
    try {
      if (aboutRef.value) {
        aboutConfig = aboutRef.value.getConfig() || {}
      } else {
        aboutConfig = {}
      }
    } catch (e: any) {
      aboutConfig = {}
    }
    
    // 合并配置：直接使用新获取的配置，避免被 globalConfig 中的旧数据覆盖
    const finalConfig: any = {
      // 直接使用新获取的配置
      ...configCardConfig,
      ...accessConfig,
      ...storageConfig,
      ...aboutConfig
    }
    
    // 只保留 globalConfig 中可能需要的其他字段（如果有的话）
    // 但确保 Rules、AllowAllLAN、Whitelist、MetricsStorage、Update 使用最新的
    for (const key in globalConfig.value) {
      if (key !== 'Rules' && key !== 'AllowAllLAN' && key !== 'Whitelist' && key !== 'MetricsStorage' && key !== 'Update') {
        if (!(key in finalConfig)) {
          finalConfig[key] = globalConfig.value[key]
        }
      }
    }

    const savedCfg = await SaveConfig(finalConfig)

    // 同步开机自启（由前端插件执行；后端只负责持久化 auto_start 到 config.toml）
    try {
      const wantAutoStart = !!finalConfig.auto_start
      const enabled = await isAutostartEnabled()
      if (wantAutoStart && !enabled) {
        await enableAutostart()
      } else if (!wantAutoStart && enabled) {
        await disableAutostart()
      }
    } catch (e: any) {
      ElMessage.warning(t('app.autostartFailed', { error: e?.message || String(e) }))
    }

    // 保存成功后，用后端返回的最终配置（包含补齐后的 id）刷新本地缓存，
    // 避免下次保存时丢失 id 导致重新生成。
    try {
      if (savedCfg) {
        globalConfig.value = savedCfg
      }
    } catch {
      // ignore
    }

    ElMessage.success(t('app.configSaved'))

    // 后端已负责重启，等待状态事件更新即可
    // 如需立即刷新配置，可稍后主动重新读取配置
    setTimeout(async () => {
      try {
        const refreshedCfg = await GetConfig();
        globalConfig.value = refreshedCfg;
      } catch {}
    }, 1000)

  } catch (e: any) {
    ElMessage.error(t('app.saveFailed', { error: e?.message || String(e) }))
  } finally {
    saving.value = false
  }
}

watch(isDark, applyTheme)

// 菜单选择处理
const handleMenuSelect = (key: string) => {
  activeTab.value = key as typeof activeTab.value
}

// 初始化
onMounted(async () => {
  loadTheme()
  loadSidebarState()
  
  // 同步语言设置到后端（更新托盘菜单）
  // 语言选择器组件会在初始化时自动同步，这里不需要手动调用
  
  // 启动自动主题切换（如果已启用）
  startAutoTheme()
  
  // 立即设置退出事件监听，确保无论是否接受条款都能响应退出请求
  await setupQuitHandler()
  
  // 检查条款接受状态（使用 localStorage）
  try {
    const termsAccepted = GetTermsAccepted()
    if (!termsAccepted) {
      showTermsDialog.value = true
      // 如果未接受条款，等待用户接受后再继续初始化
      // 初始化逻辑将在用户接受条款后通过页面重新加载执行
      return
    }
  } catch (error: any) {
    console.error('检查条款接受状态失败:', error)
    // 如果检查失败，默认显示条款对话框
    showTermsDialog.value = true
    return
  }
  
  // 只有接受条款后才继续初始化
  await initializeApp()
})

// 处理条款接受后的逻辑
const handleTermsAccepted = async () => {
  showTermsDialog.value = false
  // 继续初始化应用（但保留已加载的配置，避免覆盖用户设置）
  await initializeAppWithoutReloadConfig()
}

// 设置退出事件处理器（在应用启动时立即设置，确保退出功能始终可用）
const setupQuitHandler = async () => {
  // 托盘请求退出：由前端弹确认框，避免后端/GTK 死锁
  await EventsOn('request-quit', () => {
    ElMessageBox.confirm(
      t('app.confirmQuit'),
      t('app.quit'),
      {
        confirmButtonText: t('app.quit'),
        cancelButtonText: t('common.cancel'),
        type: 'warning',
      }
    )
      .then(() => {
        QuitApp()
      })
      .catch(() => {
        // 用户取消
      })
  })
}

// 初始化应用核心逻辑（公共部分）
const initializeAppCore = async () => {
  status.value = (await GetStatus()) as string
  try {
    await SetTrayProxyState(status.value === 'running')
  } catch {
    // ignore
  }

  // 如果服务已经在运行，记录启动时间（使用当前时间作为近似值）
  if (status.value === 'running') {
    startTime.value = Date.now()
    currentTime.value = Date.now()
    startRuntimeTimer()
  }
  await EventsOn('status', async (s: unknown) => {
    status.value = s as string

    try {
      await SetTrayProxyState(status.value === 'running')
    } catch {
      // ignore
    }

    // 如果状态变为运行，开始计时
    if (s === 'running' && !startTime.value) {
      startTime.value = Date.now()
      currentTime.value = Date.now()
      startRuntimeTimer()
    } else if (s === 'stopped') {
      // 如果状态变为停止，重置计时
      startTime.value = null
      stopRuntimeTimer()
    }
  })

  // 监听后端启动错误
  await EventsOn('server-start-error', (payload: any) => {
    try {
      const msg = t('app.portStartFailed', { 
        port: payload?.listen_addr ?? '', 
        error: payload?.error ?? JSON.stringify(payload) 
      })
      console.error('[server-start-error]', payload)
      ElNotification({
        title: t('app.serverStartError'),
        message: msg,
        type: 'error',
        duration: 0, // 不自动关闭
      })

      // 发生启动错误时，不要让 UI 继续显示"运行中"
      status.value = 'stopped'
      startTime.value = null
      stopRuntimeTimer()
    } catch (e: any) {
      console.error('处理 server-start-error 事件失败', e)
      ElMessage.error(t('app.serverStartErrorWithException', { error: e?.message || String(e) }))
    }
  });
  
  // 自动启动服务
  if (status.value === 'stopped') {
    setTimeout(() => {
      start()
    }, 500) // 延迟500ms启动，确保界面已完全加载
  }

  // 监听自动检查更新结果
  await EventsOn('update-check-result', (result: any) => {
    if (result?.has_update && result?.update_info) {
      const info = result.update_info;
      const message = t('app.newVersion', { 
        version: info.latest_version, 
        notes: info.release_notes || '' 
      });
      
      ElMessageBox.confirm(
        message,
        t('app.updateAvailable'),
        {
          confirmButtonText: t('app.downloadUpdate'),
          cancelButtonText: t('app.later'),
          type: 'warning',
          dangerouslyUseHTMLString: true,
          showClose: false,
          closeOnClickModal: false,
          closeOnPressEscape: false,
          closeOnHashChange: false,
        }
      )
        .then(async () => {
          if (info.download_url) {
            try {
              await openUrl(info.download_url);
            } catch (e: any) {
              console.error('打开下载链接失败:', e);
            }
          }
        })
        .catch(() => {
          // 用户点击取消
        });
    }
  });
}

// 将初始化逻辑提取为单独函数（包含配置加载）
const initializeApp = async () => {
  await initializeAppCore()
  
  // 加载配置
  try {
    const config = await GetConfig()
    globalConfig.value = config
  } catch (e) {
    // 加载配置失败时使用默认配置
  }
}

// 初始化应用（不重新加载配置，保留已有配置状态）
const initializeAppWithoutReloadConfig = async () => {
  await initializeAppCore()
  // 不重新加载配置，保留用户已设置的配置状态（如数据库持久化开关）
}

// 组件卸载时清理定时器
onBeforeUnmount(() => {
  stopAutoTheme()
  stopRuntimeTimer()
})
</script>

<style scoped>
.app-container {
  display: flex;
  flex-direction: column;
  height: 100vh;
  width: 100%;
  padding: 0;
  background: var(--bg-gradient);
  overflow: hidden;
  transition: background-color 0.3s;
  position: relative;
}

.app-container > .top-bar {
  margin: 16px 16px 0;
}

.app-container::before {
  content: '';
  position: absolute;
  top: 32px;
  left: 0;
  right: 0;
  height: 1px;
  background: linear-gradient(90deg, transparent, var(--primary), transparent);
  opacity: 0.3;
}

.top-bar {
  flex-shrink: 0;
  border-radius: var(--radius-lg);
  background: var(--card-bg);
  border: 1px solid var(--border);
  backdrop-filter: blur(12px);
}

.top-bar :deep(.el-card__body) {
  padding: 16px 24px;
}

.top-bar-content {
  display: flex;
  justify-content: space-between;
  align-items: center;
}

.top-bar-right {
  display: flex;
  align-items: center;
  gap: 16px;
}

.theme-control {
  display: flex;
  align-items: center;
  gap: 12px;
  margin-right: 8px;
}

.auto-theme-switch {
  --el-switch-on-color: var(--primary);
  --el-switch-off-color: var(--text-muted);
}

.theme-btn {
  transition: all 0.3s cubic-bezier(0.4, 0, 0.2, 1);
}

.theme-btn:hover {
  transform: scale(1.08) rotate(15deg);
}

h1 {
  font-size: 24px;
  font-weight: 700;
  color: var(--text);
  margin: 0;
  transition: color 0.3s;
  background: linear-gradient(135deg, var(--primary), var(--primary-hover));
  -webkit-background-clip: text;
  -webkit-text-fill-color: transparent;
  background-clip: text;
  letter-spacing: -0.5px;
}

.status-control {
  display: flex;
  align-items: center;
  gap: 12px;
}

.status-label {
  color: var(--text-muted);
  font-size: 14px;
}

.status-badge {
  font-weight: 600;
  font-size: 13px;
  text-transform: uppercase;
  letter-spacing: 0.5px;
}

.runtime-text {
  color: var(--text-muted);
  font-size: 13px;
  margin-left: 8px;
}

.status-badge.running {
  animation: pulse 2s ease-in-out infinite;
}

@keyframes pulse {
  0%, 100% {
    box-shadow: 0 0 15px rgba(34, 197, 94, 0.4);
  }
  50% {
    box-shadow: 0 0 25px rgba(34, 197, 94, 0.6);
  }
}

.control-btn {
  transition: all 0.3s cubic-bezier(0.4, 0, 0.2, 1);
}

.control-btn:hover:not(:disabled) {
  transform: translateY(-2px);
}

.save-btn {
  transition: all 0.3s cubic-bezier(0.4, 0, 0.2, 1);
}

.save-btn:hover:not(:disabled) {
  transform: translateY(-2px);
}

.main-content {
  display: flex;
  gap: 16px;
  flex: 1;
  min-height: 0;
  overflow: hidden;
  padding: 16px;
}



.content-area {
  flex: 1;
  min-width: 0;
  min-height: 0;
  overflow: hidden;
  display: flex;
  flex-direction: column;
  border-radius: var(--radius-lg);
  background: var(--card-bg);
  border: 1px solid var(--border);
}

/* 响应式布局 */
@media (max-width: 1024px) {
  .app-container > .top-bar {
    margin: 16px;
  }

  .main-content {
    padding: 0 16px 16px 16px;
  }

  .top-bar {
    padding: 14px 20px;
    flex-wrap: wrap;
    gap: 12px;
  }

  .top-bar-right {
    flex-wrap: wrap;
    gap: 12px;
  }

  h1 {
    font-size: 20px;
  }

  .status-control {
    flex-wrap: wrap;
    gap: 8px;
  }

  .control-btn {
    padding: 8px 20px;
    font-size: 13px;
  }
}

@media (max-width: 768px) {
  .app-container > .top-bar {
    margin: 12px;
  }

  .main-content {
    padding: 0 12px 12px 12px;
  }

  .top-bar {
    padding: 12px 16px;
    flex-direction: column;
    align-items: flex-start;
    gap: 12px;
  }

  .top-bar-right {
    width: 100%;
    justify-content: space-between;
  }

  h1 {
    font-size: 18px;
  }

  .main-content {
    flex-direction: column;
    gap: 16px;
  }

  .sidebar-nav {
    width: 100% !important;
    max-height: 200px;
  }

  .sidebar-nav.sidebar-collapsed {
    width: 100% !important;
  }

  .nav-menu {
    display: flex;
    flex-wrap: wrap;
    gap: 8px;
  }

  .nav-menu :deep(.el-menu-item-group) {
    width: 100%;
  }

  .nav-menu :deep(.el-menu-item-group__title) {
    padding: 8px 12px 4px;
    font-size: 11px;
  }

  .nav-menu :deep(.el-menu-item) {
    flex: 1;
    min-width: 120px;
    height: 40px;
    line-height: 40px;
    margin: 0;
    font-size: 13px;
  }

  .status-badge {
    padding: 6px 14px;
    font-size: 12px;
  }

  .control-btn {
    padding: 8px 18px;
    font-size: 12px;
  }

  .theme-btn {
    width: 36px;
    height: 36px;
    font-size: 18px;
  }
  
  .theme-control {
    flex-direction: column;
    gap: 8px;
  }
  
  .auto-theme-switch {
    font-size: 12px;
  }
}

@media (max-width: 480px) {
  .app-container > .top-bar {
    margin: 8px;
  }

  .main-content {
    padding: 0 8px 8px 8px;
  }

  .top-bar {
    padding: 10px 12px;
    border-radius: 12px;
  }

  h1 {
    font-size: 16px;
  }

  .sidebar-nav {
    max-height: 300px;
  }

  .nav-menu :deep(.el-menu-item) {
    min-width: 100px;
    font-size: 12px;
    padding: 0 12px;
  }

  .status-control {
    flex-direction: column;
    align-items: flex-start;
    width: 100%;
    gap: 8px;
  }

  .status-label {
    font-size: 12px;
  }

  .status-badge {
    padding: 6px 12px;
    font-size: 11px;
  }

  .control-btn {
    width: 100%;
    padding: 10px;
  }

  .theme-btn {
    width: 32px;
    height: 32px;
    font-size: 16px;
  }
}
</style>
