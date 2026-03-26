<!-- frontend/src/components/BaseConfig.vue -->
<template>
  <el-card class="config-card config-page" shadow="hover">
    <template #header>
      <div class="header-row">
        <h3>{{ $t('baseConfig.title') }}</h3>
        <div class="header-actions">
          <el-button
            v-if="activeTab === 'general'"
            type="warning"
            size="small"
            plain
            @click="resetToDefaults"
          >
            {{ $t('baseConfig.restoreDefaults') }}
          </el-button>
        </div>
      </div>
    </template>

    <el-tabs v-model="activeTab" class="base-tabs">
      <el-tab-pane :label="$t('baseConfig.tabGeneral')" name="general">
        <BaseConfigGeneralTab :model="generalForm" />
      </el-tab-pane>

      <el-tab-pane :label="$t('baseConfig.tabWebhook')" name="webhook">
        <BaseConfigWebhookTab
          :model="alertForm"
          :sending-test-alert="sendingTestAlert"
          :interval-options="systemReportIntervalOptions"
          :weekday-options="weekdayOptions"
          @send-test-alert="handleSendTestAlert"
        />
      </el-tab-pane>

      <el-tab-pane :label="$t('baseConfig.tabSnapshots')" name="snapshots">
        <BaseConfigSnapshotsTab
          :model="snapshotsForm"
          :format-ts="formatTs"
          :format-size="formatSize"
          @refresh="loadSnapshots"
          @restore="handleRestoreSnapshot"
        />
      </el-tab-pane>
    </el-tabs>
  </el-card>
</template>

<script setup lang="ts">
import { computed, onMounted, reactive, ref, watch } from 'vue'
import { ElMessage, ElMessageBox } from 'element-plus'
import {
  GetConfig,
  ListConfigSnapshots,
  RestoreConfigSnapshot,
  SendTestAlert,
  type AlertingConfig,
} from '../api'
import { useI18n } from 'vue-i18n'
import BaseConfigGeneralTab from './base-config/BaseConfigGeneralTab.vue'
import BaseConfigWebhookTab from './base-config/BaseConfigWebhookTab.vue'
import BaseConfigSnapshotsTab from './base-config/BaseConfigSnapshotsTab.vue'
import {
  DEFAULT_COMPRESSION_BROTLI,
  DEFAULT_COMPRESSION_BROTLI_LEVEL,
  DEFAULT_COMPRESSION_ENABLED,
  DEFAULT_COMPRESSION_GZIP,
  DEFAULT_COMPRESSION_GZIP_LEVEL,
  DEFAULT_COMPRESSION_MIN_LENGTH,
  DEFAULT_CONNECT_TIMEOUT_MS,
  DEFAULT_ENABLE_HTTP2,
  DEFAULT_MAX_BODY_SIZE_MB,
  DEFAULT_MAX_RESPONSE_BODY_SIZE_MB,
  DEFAULT_POOL_IDLE_TIMEOUT_SEC,
  DEFAULT_POOL_MAX_IDLE,
  DEFAULT_QUIET_HOURS_END,
  DEFAULT_QUIET_HOURS_START,
  DEFAULT_READ_TIMEOUT_MS,
  DEFAULT_SYSTEM_REPORT_INTERVAL_MINUTES,
  DEFAULT_SYSTEM_REPORT_WEEKDAYS,
  SYSTEM_REPORT_INTERVAL_OPTIONS,
  type AlertingForm,
  type BaseGeneralForm,
  type BaseSnapshotsForm,
} from './base-config/types'

const { t } = useI18n()
const activeTab = ref<'general' | 'webhook' | 'snapshots'>('general')

const generalForm = reactive<BaseGeneralForm>({
  autoStart: false,
  showRealtimeLogs: true,
  realtimeLogsOnlyErrors: false,
  streamProxy: true,
  enableHttp2: DEFAULT_ENABLE_HTTP2,
  maxBodySizeMB: DEFAULT_MAX_BODY_SIZE_MB,
  maxResponseBodySizeMB: DEFAULT_MAX_RESPONSE_BODY_SIZE_MB,
  upstreamConnectTimeoutMs: DEFAULT_CONNECT_TIMEOUT_MS,
  upstreamReadTimeoutMs: DEFAULT_READ_TIMEOUT_MS,
  upstreamPoolMaxIdle: DEFAULT_POOL_MAX_IDLE,
  upstreamPoolIdleTimeoutSec: DEFAULT_POOL_IDLE_TIMEOUT_SEC,
  compressionEnabled: DEFAULT_COMPRESSION_ENABLED,
  compressionGzip: DEFAULT_COMPRESSION_GZIP,
  compressionBrotli: DEFAULT_COMPRESSION_BROTLI,
  compressionGzipLevel: DEFAULT_COMPRESSION_GZIP_LEVEL,
  compressionBrotliLevel: DEFAULT_COMPRESSION_BROTLI_LEVEL,
})

const alertForm = reactive<AlertingForm>({
  enabled: false,
  webhook: {
    enabled: false,
    provider: 'wecom',
    url: '',
    secret: '',
    system_report_enabled: false,
    quiet_hours_enabled: false,
    quiet_hours_start: DEFAULT_QUIET_HOURS_START,
    quiet_hours_end: DEFAULT_QUIET_HOURS_END,
    system_report_interval_minutes: DEFAULT_SYSTEM_REPORT_INTERVAL_MINUTES,
    system_report_weekdays: [...DEFAULT_SYSTEM_REPORT_WEEKDAYS],
  },
  rules: {
    server_start_error: true,
  },
})

const sendingTestAlert = ref(false)
const snapshotsForm = reactive<BaseSnapshotsForm>({
  loading: false,
  restoringSnapshotName: '',
  list: [],
})

const systemReportIntervalOptions = SYSTEM_REPORT_INTERVAL_OPTIONS
const weekdayOptions = computed(() => [
  { value: 1, label: t('about.weekdayMon') },
  { value: 2, label: t('about.weekdayTue') },
  { value: 3, label: t('about.weekdayWed') },
  { value: 4, label: t('about.weekdayThu') },
  { value: 5, label: t('about.weekdayFri') },
  { value: 6, label: t('about.weekdaySat') },
  { value: 7, label: t('about.weekdaySun') },
])

const normalizeIntervalMinutes = (value: unknown) => {
  const raw = typeof value === 'string' ? value.trim() : value
  if (raw === '' || raw === null || raw === undefined) {
    throw new Error(t('about.systemReportIntervalInvalid'))
  }

  const parsed = typeof raw === 'number' ? raw : Number(raw)
  if (!Number.isInteger(parsed)) {
    throw new Error(t('about.systemReportIntervalInvalid'))
  }
  if (parsed < 1 || parsed > 10080) {
    throw new Error(t('about.systemReportIntervalRange'))
  }

  return parsed
}

const normalizeWeekdays = (value: unknown) => {
  const days = Array.isArray(value) ? value : []
  const normalized = [...new Set(days.map((day) => Number(day)).filter((day) => Number.isInteger(day) && day >= 1 && day <= 7))].sort((a, b) => a - b)
  if (!normalized.length) {
    throw new Error(t('about.systemReportWeekdaysRequired'))
  }
  return normalized
}

const validateTimeValue = (value: unknown, field: 'start' | 'end') => {
  const text = String(value || '').trim()
  if (!/^\d{2}:\d{2}$/.test(text)) {
    throw new Error(field === 'start' ? t('about.quietHoursStartInvalid') : t('about.quietHoursEndInvalid'))
  }

  const [hourText, minuteText] = text.split(':')
  const hour = Number(hourText)
  const minute = Number(minuteText)
  if (!Number.isInteger(hour) || !Number.isInteger(minute) || hour < 0 || hour > 23 || minute < 0 || minute > 59) {
    throw new Error(field === 'start' ? t('about.quietHoursStartInvalid') : t('about.quietHoursEndInvalid'))
  }

  return text
}

const normalizeAlertingConfig = (): AlertingConfig => {
  const quietHoursStart = validateTimeValue(alertForm.webhook.quiet_hours_start, 'start')
  const quietHoursEnd = validateTimeValue(alertForm.webhook.quiet_hours_end, 'end')
  if (alertForm.webhook.quiet_hours_enabled && quietHoursStart === quietHoursEnd) {
    throw new Error(t('about.quietHoursSameTime'))
  }

  return {
    enabled: !!alertForm.enabled,
    webhook: {
      ...alertForm.webhook,
      enabled: !!alertForm.webhook.enabled,
      provider: (alertForm.webhook.provider || 'wecom').trim(),
      url: (alertForm.webhook.url || '').trim(),
      secret: (alertForm.webhook.secret || '').trim() || null,
      system_report_enabled: !!alertForm.webhook.system_report_enabled,
      quiet_hours_enabled: !!alertForm.webhook.quiet_hours_enabled,
      quiet_hours_start: quietHoursStart,
      quiet_hours_end: quietHoursEnd,
      system_report_interval_minutes: normalizeIntervalMinutes(alertForm.webhook.system_report_interval_minutes),
      system_report_weekdays: normalizeWeekdays(alertForm.webhook.system_report_weekdays),
    },
    rules: {
      server_start_error: alertForm.rules.server_start_error !== false,
    },
  }
}

const resetToDefaults = async () => {
  try {
    await ElMessageBox.confirm(
      t('baseConfig.restoreConfirm'),
      t('baseConfig.restoreDefaults'),
      {
        confirmButtonText: t('common.restore'),
        cancelButtonText: t('common.cancel'),
        type: 'warning',
      }
    )

    generalForm.autoStart = false
    generalForm.showRealtimeLogs = true
    generalForm.realtimeLogsOnlyErrors = false
    generalForm.streamProxy = true
    generalForm.enableHttp2 = DEFAULT_ENABLE_HTTP2
    generalForm.maxBodySizeMB = DEFAULT_MAX_BODY_SIZE_MB
    generalForm.maxResponseBodySizeMB = DEFAULT_MAX_RESPONSE_BODY_SIZE_MB
    generalForm.upstreamConnectTimeoutMs = DEFAULT_CONNECT_TIMEOUT_MS
    generalForm.upstreamReadTimeoutMs = DEFAULT_READ_TIMEOUT_MS
    generalForm.upstreamPoolMaxIdle = DEFAULT_POOL_MAX_IDLE
    generalForm.upstreamPoolIdleTimeoutSec = DEFAULT_POOL_IDLE_TIMEOUT_SEC
    generalForm.compressionEnabled = DEFAULT_COMPRESSION_ENABLED
    generalForm.compressionGzip = DEFAULT_COMPRESSION_GZIP
    generalForm.compressionBrotli = DEFAULT_COMPRESSION_BROTLI
    generalForm.compressionGzipLevel = DEFAULT_COMPRESSION_GZIP_LEVEL
    generalForm.compressionBrotliLevel = DEFAULT_COMPRESSION_BROTLI_LEVEL

    window.dispatchEvent(new CustomEvent('toggle-realtime-logs', { detail: generalForm.showRealtimeLogs }))
    ElMessage.success(t('baseConfig.restoreSuccess'))
  } catch {
    // 用户取消
  }
}

const formatTs = (ms: number) => {
  if (!ms) return '-'
  try {
    return new Date(ms).toLocaleString()
  } catch {
    return String(ms)
  }
}

const formatSize = (size: number) => {
  if (size < 1024) return `${size} B`
  if (size < 1024 * 1024) return `${(size / 1024).toFixed(1)} KB`
  return `${(size / (1024 * 1024)).toFixed(1)} MB`
}

const loadSnapshots = async () => {
  snapshotsForm.loading = true
  try {
    snapshotsForm.list = await ListConfigSnapshots()
  } catch (e: any) {
    ElMessage.error(e?.message || String(e))
  } finally {
    snapshotsForm.loading = false
  }
}

const handleRestoreSnapshot = async (name: string) => {
  await ElMessageBox.confirm(
    t('about.restoreSnapshotConfirm', { name }),
    t('about.restoreSnapshotTitle'),
    {
      type: 'warning',
      confirmButtonText: t('common.confirm'),
      cancelButtonText: t('common.cancel'),
    }
  )

  snapshotsForm.restoringSnapshotName = name
  try {
    await RestoreConfigSnapshot(name)
    ElMessage.success(t('about.restoreSnapshotSuccess'))
    await loadSnapshots()
  } catch (e: any) {
    ElMessage.error(t('about.restoreSnapshotFailed', { error: e?.message || String(e) }))
  } finally {
    snapshotsForm.restoringSnapshotName = ''
  }
}

const handleSendTestAlert = async () => {
  if (!alertForm.enabled || !alertForm.webhook?.enabled) {
    ElMessage.warning(t('about.alertConfigIncomplete'))
    return
  }
  if (!alertForm.webhook.url?.trim()) {
    ElMessage.warning(t('about.alertWebhookUrlRequired'))
    return
  }

  sendingTestAlert.value = true
  try {
    await SendTestAlert(normalizeAlertingConfig())
    ElMessage.success(t('about.sendTestAlertSuccess'))
  } catch (e: any) {
    ElMessage.error(t('about.sendTestAlertFailed', { error: e?.message || String(e) }))
  } finally {
    sendingTestAlert.value = false
  }
}

onMounted(async () => {
  try {
    const configData = (await GetConfig()) as any
    generalForm.autoStart = !!configData.auto_start
    generalForm.showRealtimeLogs = configData.show_realtime_logs !== false
    generalForm.realtimeLogsOnlyErrors = !!configData.realtime_logs_only_errors
    generalForm.streamProxy = configData.stream_proxy !== false
    generalForm.enableHttp2 = configData.enable_http2 !== false
    generalForm.maxBodySizeMB = Math.round(((configData.max_body_size ?? DEFAULT_MAX_BODY_SIZE_MB * 1024 * 1024) / 1024 / 1024) * 100) / 100
    generalForm.maxResponseBodySizeMB = Math.round(((configData.max_response_body_size ?? DEFAULT_MAX_RESPONSE_BODY_SIZE_MB * 1024 * 1024) / 1024 / 1024) * 100) / 100
    generalForm.upstreamConnectTimeoutMs = configData.upstream_connect_timeout_ms ?? DEFAULT_CONNECT_TIMEOUT_MS
    generalForm.upstreamReadTimeoutMs = configData.upstream_read_timeout_ms ?? DEFAULT_READ_TIMEOUT_MS
    generalForm.upstreamPoolMaxIdle = configData.upstream_pool_max_idle ?? DEFAULT_POOL_MAX_IDLE
    generalForm.upstreamPoolIdleTimeoutSec = configData.upstream_pool_idle_timeout_sec ?? DEFAULT_POOL_IDLE_TIMEOUT_SEC
    generalForm.compressionEnabled = configData.compression_enabled ?? DEFAULT_COMPRESSION_ENABLED
    generalForm.compressionGzip = configData.compression_gzip ?? DEFAULT_COMPRESSION_GZIP
    generalForm.compressionBrotli = configData.compression_brotli ?? DEFAULT_COMPRESSION_BROTLI
    generalForm.compressionGzipLevel = configData.compression_gzip_level ?? DEFAULT_COMPRESSION_GZIP_LEVEL
    generalForm.compressionBrotliLevel = configData.compression_brotli_level ?? DEFAULT_COMPRESSION_BROTLI_LEVEL

    const alerting = configData?.alerting
    if (alerting) {
      const savedWeekdays = Array.isArray(alerting?.webhook?.system_report_weekdays)
        ? alerting.webhook.system_report_weekdays
        : null
      alertForm.enabled = !!alerting.enabled
      alertForm.webhook = {
        enabled: !!alerting?.webhook?.enabled,
        provider: alerting?.webhook?.provider || 'wecom',
        url: alerting?.webhook?.url || '',
        secret: alerting?.webhook?.secret || '',
        system_report_enabled: !!alerting?.webhook?.system_report_enabled,
        quiet_hours_enabled: !!alerting?.webhook?.quiet_hours_enabled,
        quiet_hours_start: alerting?.webhook?.quiet_hours_start || DEFAULT_QUIET_HOURS_START,
        quiet_hours_end: alerting?.webhook?.quiet_hours_end || DEFAULT_QUIET_HOURS_END,
        system_report_interval_minutes: alerting?.webhook?.system_report_interval_minutes ?? DEFAULT_SYSTEM_REPORT_INTERVAL_MINUTES,
        system_report_weekdays: savedWeekdays?.length
          ? [...savedWeekdays]
          : [...DEFAULT_SYSTEM_REPORT_WEEKDAYS],
      }
      alertForm.rules = {
        server_start_error: alerting?.rules?.server_start_error !== false,
      }
    }
  } catch {
    // ignore
  }

  await loadSnapshots()
})

watch(() => generalForm.showRealtimeLogs, (v) => {
  if (!v) {
    generalForm.realtimeLogsOnlyErrors = false
  }
  window.dispatchEvent(new CustomEvent('toggle-realtime-logs', { detail: v }))
})

const getConfig = () => ({
  auto_start: !!generalForm.autoStart,
  show_realtime_logs: !!generalForm.showRealtimeLogs,
  realtime_logs_only_errors: !!generalForm.realtimeLogsOnlyErrors,
  stream_proxy: !!generalForm.streamProxy,
  enable_http2: !!generalForm.enableHttp2,
  max_body_size: Math.floor(generalForm.maxBodySizeMB * 1024 * 1024),
  max_response_body_size: Math.floor(generalForm.maxResponseBodySizeMB * 1024 * 1024),
  upstream_connect_timeout_ms: Number(generalForm.upstreamConnectTimeoutMs),
  upstream_read_timeout_ms: Number(generalForm.upstreamReadTimeoutMs),
  upstream_pool_max_idle: Number(generalForm.upstreamPoolMaxIdle),
  upstream_pool_idle_timeout_sec: Number(generalForm.upstreamPoolIdleTimeoutSec),
  compression_enabled: !!generalForm.compressionEnabled,
  compression_gzip: !!generalForm.compressionGzip,
  compression_brotli: !!generalForm.compressionBrotli,
  compression_min_length: DEFAULT_COMPRESSION_MIN_LENGTH,
  compression_gzip_level: Number(generalForm.compressionGzipLevel),
  compression_brotli_level: Number(generalForm.compressionBrotliLevel),
  alerting: normalizeAlertingConfig(),
})

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
  padding: 16px 20px 24px;
}

.base-tabs {
  margin-top: -6px;
}

.base-tabs :deep(.el-tabs__header) {
  position: sticky;
  top: 0;
  z-index: 20;
  margin-bottom: 18px;
  padding-top: 4px;
}

.header-row {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 12px;
}

.header-actions {
  display: flex;
  align-items: center;
  gap: 12px;
}

.header-row h3 {
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

.el-form-item {
  margin-bottom: 24px;
}

.config-page :deep(.el-input-number),
.config-page :deep(.el-slider) {
  max-width: 340px;
}

.config-page :deep(.el-divider) {
  margin: 28px 0 22px;
}

:deep(.el-form-item__content) {
  display: flex;
  align-items: center;
  gap: 12px;
  flex-wrap: wrap;
}

.mini-hint {
  font-size: 12px;
  color: var(--text-muted);
  line-height: 1.4;
}

@media (prefers-color-scheme: light) {
  .snapshot-restore-btn {
    --el-button-bg-color: #fde9c8;
    --el-button-border-color: #d8942e;
    --el-button-text-color: #ffffff;
    --el-button-hover-bg-color: #f0ae3a;
    --el-button-hover-border-color: #bf7d1f;
    --el-button-hover-text-color: #ffffff;
    --el-button-active-bg-color: #dc971d;
    --el-button-active-border-color: #a96610;
    --el-button-active-text-color: #ffffff;
    color: #ffffff !important;
    border-color: #d8942e !important;
    background: linear-gradient(135deg, #f2b24a, #e59e25) !important;
    font-weight: 600;
  }
}
</style>
