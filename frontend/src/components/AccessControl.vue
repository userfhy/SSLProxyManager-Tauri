<template>
  <el-card class="config-card config-page" shadow="hover">
    <template #header>
      <h3>{{ $t('accessControl.title') }}</h3>
    </template>

    <el-form label-width="180px">
      <el-form-item :label="$t('accessControl.accessControlSwitch')">
      <div class="ac-switches">
        <div class="ac-switch">
          <el-switch v-model="localConfig.http_access_control_enabled" :active-text="$t('accessControl.httpHttps')"/>
        </div>
        <div class="ac-switch">
          <el-switch v-model="localConfig.ws_access_control_enabled" :active-text="$t('accessControl.websocket')"/>
        </div>
        <div class="ac-switch">
          <el-switch v-model="localConfig.stream_access_control_enabled" :active-text="$t('accessControl.stream')"/>
        </div>
      </div>
      <el-text type="info" size="small" class="mini-hint">
        {{ $t('accessControl.switchHint') }}
      </el-text>
    </el-form-item>

    <el-form-item>
        <el-checkbox v-model="localConfig.allow_all_lan">
          {{ $t('accessControl.allowAllLAN') }}
        </el-checkbox>
        <el-text type="info" size="small" class="mini-hint">
          {{ $t('accessControl.allowAllLANHint') }}
        </el-text>
      </el-form-item>

      <el-form-item>
        <el-checkbox v-model="localConfig.allow_all_ip">
          {{ $t('accessControl.allowAllIP') }}
        </el-checkbox>
        <el-text type="info" size="small" class="mini-hint">
          {{ $t('accessControl.allowAllIPHint') }}
        </el-text>
      </el-form-item>

      <el-form-item :label="$t('accessControl.ipWhitelist')">
        <TransitionGroup name="list" tag="div" class="whitelist-list">
          <div v-for="(item, index) in localConfig.whitelist" :key="item.id || index" class="whitelist-item">
            <el-input
              v-model="item.ip"
              placeholder="例如: 192.168.1.100"
              class="whitelist-input"
            />
            <el-button @click="removeWhitelistItem(index)" type="danger" size="small">{{ $t('accessControl.delete') }}</el-button>
          </div>
        </TransitionGroup>
        <el-button @click="addWhitelistItem" type="primary" style="margin-top: 10px;">
          <el-icon><Plus /></el-icon> {{ $t('accessControl.addIP') }}
        </el-button>
      </el-form-item>

      <el-divider style="margin: 16px 0;" />

      <div class="blacklist-header">
        <h4>{{ $t('accessControl.ipBlacklist') }}</h4>
        <div class="blacklist-actions">
          <el-button type="primary" @click="showAddDialog = true" :icon="Plus">
            {{ $t('accessControl.addBlacklist') }}
          </el-button>
          <el-button @click="refreshBlacklist" :loading="blacklistLoading" :icon="Refresh">
            {{ $t('accessControl.refreshList') }}
          </el-button>
          <el-button @click="refreshCache" :loading="refreshingCache" :icon="RefreshRight">
            {{ $t('accessControl.refreshCache') }}
          </el-button>
        </div>
      </div>

      <el-alert
        v-if="dbStatus && !dbStatus.enabled"
        :title="$t('accessControl.dbNotEnabled')"
        type="warning"
        :closable="false"
        show-icon
        style="margin-bottom: 12px;"
      >
        <template #default>
          <div>
            <p>{{ $t('accessControl.dbNotEnabledHint') }}</p>
            <p>{{ $t('accessControl.goToStorage') }}</p>
          </div>
        </template>
      </el-alert>

      <el-table
        :data="blacklist"
        v-loading="blacklistLoading"
        stripe
        border
        style="width: 100%; margin-top: 12px;"
      >
        <el-table-column prop="ip" :label="$t('accessControl.ipAddress')" width="180" sortable />
        <el-table-column prop="reason" :label="$t('accessControl.blacklistReason')" min-width="200" show-overflow-tooltip />
        <el-table-column prop="expires_at" :label="$t('accessControl.expiresAt')" width="180" sortable>
          <template #default="{ row }">
            <span v-if="row.expires_at === 0">{{ $t('accessControl.permanent') }}</span>
            <span v-else>
              {{ formatTime(row.expires_at) }}
              <el-tag
                v-if="isExpired(row.expires_at)"
                type="danger"
                size="small"
                style="margin-left: 8px;"
              >
                {{ $t('accessControl.expired') }}
              </el-tag>
              <el-tag
                v-else
                type="success"
                size="small"
                style="margin-left: 8px;"
              >
                {{ $t('accessControl.valid') }}
              </el-tag>
            </span>
          </template>
        </el-table-column>
        <el-table-column prop="created_at" :label="$t('accessControl.createdAt')" width="180" sortable>
          <template #default="{ row }">
            {{ formatTime(row.created_at) }}
          </template>
        </el-table-column>
        <el-table-column :label="$t('accessControl.actions')" width="120" fixed="right">
          <template #default="{ row }">
            <el-button
              type="danger"
              size="small"
              @click="handleRemove(row.ip)"
              :icon="Delete"
            >
              {{ $t('accessControl.delete') }}
            </el-button>
          </template>
        </el-table-column>
      </el-table>

      <el-dialog
        v-model="showAddDialog"
        :title="$t('accessControl.addBlacklistTitle')"
        width="520px"
        :close-on-click-modal="false"
      >
        <el-form :model="addForm" label-width="120px" :rules="addRules" ref="addFormRef">
          <el-form-item :label="$t('accessControl.ipAddress')" prop="ip">
            <el-input
              v-model="addForm.ip"
              :placeholder="$t('accessControl.ipPlaceholder')"
              clearable
            />
            <el-text type="info" size="small" class="hint">
              {{ $t('accessControl.ipHint') }}
            </el-text>
          </el-form-item>

          <el-form-item :label="$t('accessControl.reason')" prop="reason">
            <el-input
              v-model="addForm.reason"
              type="textarea"
              :rows="3"
              :placeholder="$t('accessControl.reasonPlaceholder')"
              clearable
            />
          </el-form-item>

          <el-form-item :label="$t('accessControl.expiresAtLabel')">
            <el-radio-group v-model="addForm.durationType">
              <el-radio label="permanent">{{ $t('accessControl.permanentLabel') }}</el-radio>
              <el-radio label="temporary">{{ $t('requestLogs.timeRange') }}</el-radio>
            </el-radio-group>
          </el-form-item>

          <el-form-item
            v-if="addForm.durationType === 'temporary'"
            :label="$t('accessControl.expiresAtLabel')"
            prop="expiresAt"
          >
            <el-config-provider :locale="zhCn">
              <el-date-picker
                v-model="addForm.expiresAt"
                type="datetime"
                format="YYYY-MM-DD HH:mm:ss"
                value-format="x"
                :shortcuts="dateShortcuts"
                placeholder="选择过期日期和时间"
                :disabled-date="disabledDate"
                style="width: 100%;"
              />
            </el-config-provider>
            <el-text type="info" size="small" class="hint">
              只允许选择未来时间
            </el-text>
          </el-form-item>
        </el-form>

        <template #footer>
          <el-button @click="showAddDialog = false">取消</el-button>
          <el-button type="primary" @click="handleAdd" :loading="adding">确定</el-button>
        </template>
      </el-dialog>
    </el-form>
  </el-card>
</template>

<script setup lang="ts">
import { ref, watch, onMounted } from 'vue'
import { Plus, Refresh, RefreshRight, Delete } from '@element-plus/icons-vue'
import { ElMessage, ElMessageBox, ElConfigProvider } from 'element-plus'
import zhCn from 'element-plus/dist/locale/zh-cn.mjs'
// @ts-ignore
import { AddBlacklistEntry, RemoveBlacklistEntry, GetBlacklistEntries, RefreshBlacklistCache, GetMetricsDBStatus } from '../api'
import { useI18n } from 'vue-i18n'

const { t } = useI18n()

interface BlacklistEntry {
  id: number
  ip: string
  reason?: string | null
  expires_at: number
  created_at: number
}

const props = defineProps<{ config: any }>()

const localConfig = ref({
  http_access_control_enabled: true,
  ws_access_control_enabled: true,
  stream_access_control_enabled: true,
  allow_all_lan: true,
  allow_all_ip: false,
  whitelist: [] as { id?: string; ip: string }[],
})

// 黑名单状态
const blacklist = ref<BlacklistEntry[]>([])
const blacklistLoading = ref(false)
const refreshingCache = ref(false)
const adding = ref(false)
const showAddDialog = ref(false)
const addFormRef = ref()
const dbStatus = ref<any>(null)

const addForm = ref({
  ip: '',
  reason: '',
  durationType: 'permanent' as 'permanent' | 'temporary',
  // el-date-picker value-format="x" 返回毫秒时间戳字符串
  expiresAt: null as string | null,
})

const dateShortcuts = [
  {
    text: '1小时',
    value: () => new Date(Date.now() + 1 * 60 * 60 * 1000),
  },
  {
    text: '6小时',
    value: () => new Date(Date.now() + 6 * 60 * 60 * 1000),
  },
  {
    text: '1天',
    value: () => new Date(Date.now() + 24 * 60 * 60 * 1000),
  },
  {
    text: '1个月',
    value: () => {
      const d = new Date()
      d.setMonth(d.getMonth() + 1)
      return d
    },
  },
  {
    text: '6个月',
    value: () => {
      const d = new Date()
      d.setMonth(d.getMonth() + 6)
      return d
    },
  },
  {
    text: '1年',
    value: () => {
      const d = new Date()
      d.setFullYear(d.getFullYear() + 1)
      return d
    },
  },
]

const addRules = {
  ip: [
    { required: true, message: '请输入IP地址', trigger: 'blur' },
    {
      validator: (rule: any, value: string, callback: Function) => {
        if (!value) {
          callback(new Error('请输入IP地址'))
          return
        }
        const ipv4Regex = /^(\d{1,3}\.){3}\d{1,3}$/
        const ipv6Regex = /^([0-9a-fA-F]{0,4}:){2,7}[0-9a-fA-F]{0,4}$/
        if (!ipv4Regex.test(value) && !ipv6Regex.test(value)) {
          callback(new Error('请输入有效的IP地址'))
          return
        }
        callback()
      },
      trigger: 'blur',
    },
  ],
  expiresAt: [
    {
      validator: (rule: any, value: any, callback: Function) => {
        if (addForm.value.durationType === 'permanent') {
          callback()
          return
        }
        if (!value) {
          callback(new Error('请选择过期时间'))
          return
        }
        const ms = Number(value)
        if (!Number.isFinite(ms)) {
          callback(new Error('过期时间格式无效'))
          return
        }
        if (ms <= Date.now()) {
          callback(new Error('过期时间必须大于当前时间'))
          return
        }
        callback()
      },
      trigger: 'change',
    },
  ],
}

const disabledDate = (time: Date) => {
  // 禁用今天之前的日期（今天可以选，但还会被 expiresAt 校验限制必须大于当前时间）
  return time.getTime() < new Date().setHours(0, 0, 0, 0)
}

watch(
  () => props.config,
  (newConfig) => {
    if (newConfig) {
      localConfig.value.http_access_control_enabled = newConfig.http_access_control_enabled !== false
      localConfig.value.ws_access_control_enabled = newConfig.ws_access_control_enabled !== false
      localConfig.value.stream_access_control_enabled = newConfig.stream_access_control_enabled !== false
      localConfig.value.allow_all_lan = newConfig.allow_all_lan ?? true
      localConfig.value.allow_all_ip = newConfig.allow_all_ip ?? false
      localConfig.value.whitelist = Array.isArray(newConfig.whitelist) ? [...newConfig.whitelist] : []
    }
  },
  { immediate: true, deep: true }
)

const addWhitelistItem = () => {
  localConfig.value.whitelist.push({ id: `new-ip-${Date.now()}`, ip: '' })
}

const removeWhitelistItem = (index: number) => {
  localConfig.value.whitelist.splice(index, 1)
}

const formatTime = (timestamp: number) => {
  if (!timestamp) return '-'
  const date = new Date(timestamp * 1000)
  return date.toLocaleString('zh-CN', {
    year: 'numeric',
    month: '2-digit',
    day: '2-digit',
    hour: '2-digit',
    minute: '2-digit',
    second: '2-digit',
  })
}

const isExpired = (expiresAt: number) => {
  if (expiresAt === 0) return false
  return Date.now() / 1000 > expiresAt
}

const refreshBlacklist = async () => {
  blacklistLoading.value = true
  try {
    // @ts-ignore
    const entries = await GetBlacklistEntries()
    blacklist.value = entries || []
  } catch (error: any) {
    console.error('获取黑名单失败:', error)
    ElMessage.error(t('accessControl.getBlacklistFailed', { error: error.message || String(error) }))
  } finally {
    blacklistLoading.value = false
  }
}

const refreshCache = async () => {
  refreshingCache.value = true
  try {
    // @ts-ignore
    await RefreshBlacklistCache()
    ElMessage.success(t('accessControl.cacheRefreshed'))
    await refreshBlacklist()
  } catch (error: any) {
    console.error('刷新缓存失败:', error)
    ElMessage.error(t('accessControl.refreshCacheFailed', { error: error.message || String(error) }))
  } finally {
    refreshingCache.value = false
  }
}

const handleAdd = async () => {
  if (!addFormRef.value) {
    ElMessage.warning(t('accessControl.formNotInitialized'))
    return
  }

  try {
    // @ts-ignore
    dbStatus.value = await GetMetricsDBStatus()
  } catch {
    dbStatus.value = null
  }

  if (!dbStatus.value || !dbStatus.value.enabled) {
    ElMessage.error({
      message: t('accessControl.dbNotEnabledMessage'),
      duration: 5000,
      showClose: true,
    })
    return
  }

  if (!dbStatus.value.initialized) {
    ElMessage.error({
      message: t('accessControl.dbNotInitializedMessage'),
      duration: 5000,
      showClose: true,
    })
    return
  }

  try {
    await addFormRef.value.validate()
  } catch {
    ElMessage.warning(t('accessControl.checkFormInput'))
    return
  }

  adding.value = true
  try {
    let durationSeconds = 0
    if (addForm.value.durationType === 'temporary') {
      const ms = Number(addForm.value.expiresAt)
      const now = Date.now()
      durationSeconds = Math.ceil((ms - now) / 1000)
      if (durationSeconds <= 0) {
        ElMessage.warning(t('accessControl.expiresAtMustBeGreater'))
        adding.value = false
        return
      }
    }

    // @ts-ignore
    await AddBlacklistEntry(addForm.value.ip, addForm.value.reason || '', durationSeconds)
    ElMessage.success(t('accessControl.blacklistAdded'))
    showAddDialog.value = false

    addForm.value = {
      ip: '',
      reason: '',
      durationType: 'permanent',
      expiresAt: null,
    }

    if (addFormRef.value) {
      addFormRef.value.resetFields()
    }

    await refreshBlacklist()
  } catch (error: any) {
    console.error('添加黑名单失败:', error)
    ElMessage.error({
      message: t('accessControl.addBlacklistFailed', { error: error?.message || String(error) }),
      duration: 5000,
      showClose: true,
    })
  } finally {
    adding.value = false
  }
}

const handleRemove = async (ip: string) => {
  try {
    await ElMessageBox.confirm(
      t('accessControl.removeBlacklistConfirm', { ip }),
      t('accessControl.removeBlacklistTitle'),
      {
        confirmButtonText: t('common.confirm'),
        cancelButtonText: t('common.cancel'),
        type: 'warning',
      }
    )

    // @ts-ignore
    await RemoveBlacklistEntry(ip)
    ElMessage.success(t('accessControl.blacklistRemoved'))
    await refreshBlacklist()
  } catch (error: any) {
    if (error !== 'cancel') {
      console.error('删除黑名单失败:', error)
      ElMessage.error(t('accessControl.removeBlacklistFailed', { error: error.message || String(error) }))
    }
  }
}

onMounted(async () => {
  try {
    // @ts-ignore
    dbStatus.value = await GetMetricsDBStatus()
  } catch {
    dbStatus.value = null
  }
  await refreshBlacklist()
})

// 供父组件调用
const getConfig = () => {
  return {
    http_access_control_enabled: !!localConfig.value.http_access_control_enabled,
    ws_access_control_enabled: !!localConfig.value.ws_access_control_enabled,
    stream_access_control_enabled: !!localConfig.value.stream_access_control_enabled,
    allow_all_lan: localConfig.value.allow_all_lan,
    allow_all_ip: localConfig.value.allow_all_ip,
    whitelist: localConfig.value.whitelist.filter((item) => item.ip.trim() !== ''),
  }
}

defineExpose({
  getConfig,
})
</script>

<style scoped>
.config-page {
  height: 100%;
  overflow-y: auto;
}

.config-page :deep(.el-card__header) {
  border-bottom: 1px solid var(--border);
  padding: 16px 20px;
}

.config-page :deep(.el-card__body) {
  padding: 24px;
}

.config-page h3 {
  font-size: 24px;
  font-weight: 700;
  color: var(--text);
  background: linear-gradient(135deg, var(--primary), var(--primary-hover));
  -webkit-background-clip: text;
  -webkit-text-fill-color: transparent;
  background-clip: text;
  letter-spacing: -0.5px;
  margin: 0;
}

.mini-hint {
  display: block;
  margin-top: 6px;
  font-size: 12px;
  color: var(--text-muted);
  line-height: 1.4;
}

.ac-switches {
  display: flex;
  gap: 24px;
  flex-wrap: wrap;
  align-items: center;
}

.whitelist-list {
  display: flex;
  flex-direction: column;
  gap: 12px;
  width: 100%;
}

.whitelist-item {
  display: grid;
  grid-template-columns: 1fr auto;
  gap: 10px;
  align-items: center;
  max-width: 400px;
}

.blacklist-header {
  display: flex;
  justify-content: space-between;
  align-items: center;
  margin: 24px 0 16px;
}

.blacklist-header h4 {
  margin: 0;
  font-size: 18px;
  font-weight: 600;
  color: var(--text);
}

.blacklist-actions {
  display: flex;
  gap: 10px;
}

.hint {
  font-size: 12px;
  display: block;
  margin-top: 4px;
  color: var(--text-muted);
}

/* Transition styles */
.list-enter-active,
.list-leave-active {
  transition: all 0.4s ease;
}
.list-enter-from,
.list-leave-to {
  opacity: 0;
  transform: translateX(30px);
}
</style>
