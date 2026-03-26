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
        <el-form label-width="190px">
          <el-form-item :label="$t('baseConfig.autoStart')">
            <el-switch v-model="autoStart" />
            <el-text type="info" size="small" class="mini-hint" style="margin-left: 10px;">
              {{ $t('baseConfig.autoStartHint') }}
            </el-text>
          </el-form-item>

          <el-form-item :label="$t('baseConfig.showRealtimeLogs')">
            <el-switch v-model="showRealtimeLogs" />
            <el-text type="info" size="small" class="mini-hint" style="margin-left: 10px;">
              {{ $t('baseConfig.showRealtimeLogsHint') }}
            </el-text>
          </el-form-item>

          <el-form-item v-if="showRealtimeLogs" :label="$t('baseConfig.realtimeLogsOnlyErrors')">
            <el-switch v-model="realtimeLogsOnlyErrors" />
            <el-text type="info" size="small" class="mini-hint" style="margin-left: 10px;">
              {{ $t('baseConfig.realtimeLogsOnlyErrorsHint') }}
            </el-text>
          </el-form-item>

          <el-form-item :label="$t('baseConfig.streamProxy')">
            <el-switch v-model="streamProxy" :active-text="$t('common.on')" :inactive-text="$t('common.off')" />
            <el-text type="info" size="small" class="mini-hint" style="margin-left: 10px;">
              {{ $t('baseConfig.streamProxyHint') }}
            </el-text>
          </el-form-item>

          <el-form-item v-if="!streamProxy" :label="$t('baseConfig.maxBodySizeMB')">
            <el-input-number v-model="maxBodySizeMB" :min="1" :max="1024" :step="1" controls-position="right" />
            <el-text type="info" size="small" class="mini-hint" style="margin-left: 10px;">
              {{ $t('baseConfig.maxBodySizeMBHint') }}
            </el-text>
          </el-form-item>

          <el-form-item v-if="!streamProxy" :label="$t('baseConfig.maxResponseBodySizeMB')">
            <el-input-number v-model="maxResponseBodySizeMB" :min="1" :max="1024" :step="1" controls-position="right" />
            <el-text type="info" size="small" class="mini-hint" style="margin-left: 10px;">
              {{ $t('baseConfig.maxResponseBodySizeMBHint') }}
            </el-text>
          </el-form-item>

          <el-form-item :label="$t('baseConfig.upstreamConnectTimeoutMs')">
            <el-input-number v-model="upstreamConnectTimeoutMs" :min="100" :max="600000" :step="100" controls-position="right" />
          </el-form-item>

          <el-form-item :label="$t('baseConfig.upstreamReadTimeoutMs')">
            <el-input-number v-model="upstreamReadTimeoutMs" :min="100" :max="600000" :step="100" controls-position="right" />
          </el-form-item>

          <el-form-item :label="$t('baseConfig.upstreamPoolMaxIdle')">
            <el-input-number v-model="upstreamPoolMaxIdle" :min="0" :max="1024" :step="1" controls-position="right" />
          </el-form-item>

          <el-form-item :label="$t('baseConfig.enableHttp2')">
            <el-switch v-model="enableHttp2" :active-text="$t('common.on')" :inactive-text="$t('common.off')" />
            <el-text type="info" size="small" class="mini-hint" style="margin-left: 10px;">
              {{ $t('baseConfig.enableHttp2Hint') }}
            </el-text>
          </el-form-item>

          <el-form-item :label="$t('baseConfig.upstreamPoolIdleTimeoutSec')">
            <el-input-number v-model="upstreamPoolIdleTimeoutSec" :min="0" :max="3600" :step="1" controls-position="right" />
          </el-form-item>

          <el-divider />

          <el-form-item :label="$t('baseConfig.compressionEnabled')">
            <el-switch v-model="compressionEnabled" :active-text="$t('common.on')" :inactive-text="$t('common.off')" />
            <el-text type="info" size="small" class="mini-hint" style="margin-left: 10px;">
              {{ $t('baseConfig.compressionEnabledHint') }}
            </el-text>
          </el-form-item>

          <template v-if="compressionEnabled">
            <el-form-item :label="$t('baseConfig.compressionGzip')">
              <el-switch v-model="compressionGzip" :active-text="$t('common.on')" :inactive-text="$t('common.off')" />
              <el-text type="info" size="small" class="mini-hint" style="margin-left: 10px;">
                {{ $t('baseConfig.compressionGzipHint') }}
              </el-text>
            </el-form-item>

            <el-form-item v-if="compressionGzip" :label="$t('baseConfig.compressionGzipLevel')">
              <el-slider
                v-model="compressionGzipLevel"
                :min="1"
                :max="9"
                :step="1"
                show-stops
                show-input
                :show-input-controls="false"
                style="width: 300px; margin-right: 12px;"
              />
              <el-text type="info" size="small" class="mini-hint">
                {{ $t('baseConfig.compressionGzipLevelHint') }}
              </el-text>
            </el-form-item>

            <el-form-item :label="$t('baseConfig.compressionBrotli')">
              <el-switch v-model="compressionBrotli" :active-text="$t('common.on')" :inactive-text="$t('common.off')" />
              <el-text type="info" size="small" class="mini-hint" style="margin-left: 10px;">
                {{ $t('baseConfig.compressionBrotliHint') }}
              </el-text>
            </el-form-item>

            <el-form-item v-if="compressionBrotli" :label="$t('baseConfig.compressionBrotliLevel')">
              <el-slider
                v-model="compressionBrotliLevel"
                :min="0"
                :max="11"
                :step="1"
                show-stops
                show-input
                :show-input-controls="false"
                style="width: 300px; margin-right: 12px;"
              />
              <el-text type="info" size="small" class="mini-hint">
                {{ $t('baseConfig.compressionBrotliLevelHint') }}
              </el-text>
            </el-form-item>
          </template>
        </el-form>
      </el-tab-pane>

      <el-tab-pane :label="$t('baseConfig.tabWebhook')" name="webhook">
        <el-form :model="alertForm" label-width="180px">
          <el-form-item :label="$t('about.alertingEnabled')">
            <el-switch v-model="alertForm.enabled" />
          </el-form-item>

          <template v-if="alertForm.enabled">
            <el-form-item :label="$t('about.alertWebhookEnabled')">
              <el-switch v-model="alertForm.webhook.enabled" />
            </el-form-item>

            <template v-if="alertForm.webhook.enabled">
              <el-form-item :label="$t('about.alertProvider')">
                <el-select v-model="alertForm.webhook.provider" style="width: 220px;">
                  <el-option label="企业微信 WeCom" value="wecom" />
                  <el-option label="飞书 Feishu" value="feishu" />
                </el-select>
              </el-form-item>

              <el-form-item :label="$t('about.alertWebhookUrl')">
                <el-input v-model="alertForm.webhook.url" :placeholder="$t('about.alertWebhookUrlPlaceholder')" />
              </el-form-item>

              <el-divider />

              <el-form-item :label="$t('about.systemReportEnabled')">
                <el-switch v-model="alertForm.webhook.system_report_enabled" />
                <el-text type="info" size="small" class="mini-hint">
                  {{ $t('about.systemReportHint') }}
                </el-text>
              </el-form-item>

              <template v-if="alertForm.webhook.system_report_enabled">
                <el-form-item :label="$t('about.systemReportIntervalMinutes')">
                  <el-select
                    v-model="alertForm.webhook.system_report_interval_minutes"
                    filterable
                    allow-create
                    default-first-option
                    style="width: 220px;"
                  >
                    <el-option
                      v-for="item in systemReportIntervalOptions"
                      :key="item"
                      :label="String(item)"
                      :value="item"
                    />
                  </el-select>
                  <el-text type="info" size="small" class="mini-hint">
                    {{ $t('about.systemReportIntervalHint') }}
                  </el-text>
                </el-form-item>

                <el-form-item :label="$t('about.systemReportWeekdays')">
                  <el-checkbox-group v-model="alertForm.webhook.system_report_weekdays">
                    <el-checkbox
                      v-for="day in weekdayOptions"
                      :key="day.value"
                      :label="day.value"
                    >
                      {{ day.label }}
                    </el-checkbox>
                  </el-checkbox-group>
                </el-form-item>

                <el-form-item :label="$t('about.quietHoursEnabled')">
                  <el-switch v-model="alertForm.webhook.quiet_hours_enabled" />
                </el-form-item>

                <template v-if="alertForm.webhook.quiet_hours_enabled">
                  <el-form-item :label="$t('about.quietHoursStart')">
                    <el-time-picker
                      v-model="alertForm.webhook.quiet_hours_start"
                      format="HH:mm"
                      value-format="HH:mm"
                      :clearable="false"
                    />
                  </el-form-item>

                  <el-form-item :label="$t('about.quietHoursEnd')">
                    <el-time-picker
                      v-model="alertForm.webhook.quiet_hours_end"
                      format="HH:mm"
                      value-format="HH:mm"
                      :clearable="false"
                    />
                    <el-text type="info" size="small" class="mini-hint">
                      {{ $t('about.quietHoursHint') }}
                    </el-text>
                  </el-form-item>
                </template>
              </template>
            </template>

            <el-form-item :label="$t('about.alertRuleServerStartError')">
              <el-switch v-model="alertForm.rules.server_start_error" />
            </el-form-item>

            <el-form-item>
              <el-button type="primary" @click="handleSendTestAlert" :loading="sendingTestAlert">
                {{ $t('about.sendTestAlert') }}
              </el-button>
            </el-form-item>
          </template>
        </el-form>
      </el-tab-pane>

      <el-tab-pane :label="$t('baseConfig.tabSnapshots')" name="snapshots">
        <el-form label-width="180px">
          <el-form-item :label="$t('about.configSnapshots')">
            <el-button @click="loadSnapshots" :loading="loadingSnapshots">{{ $t('about.refreshSnapshots') }}</el-button>
          </el-form-item>

          <el-form-item>
            <el-table :data="snapshotList" style="width: 100%" size="small" v-loading="loadingSnapshots">
              <el-table-column prop="name" :label="$t('about.snapshotName')" min-width="240" />
              <el-table-column :label="$t('about.snapshotTime')" min-width="180">
                <template #default="scope">
                  {{ formatTs(scope.row.created_at_unix_ms) }}
                </template>
              </el-table-column>
              <el-table-column :label="$t('about.snapshotSize')" width="120">
                <template #default="scope">
                  {{ formatSize(scope.row.size_bytes) }}
                </template>
              </el-table-column>
              <el-table-column :label="$t('about.actions')" width="120">
                <template #default="scope">
                  <el-button
                    size="small"
                    type="warning"
                    class="snapshot-restore-btn"
                    @click="handleRestoreSnapshot(scope.row.name)"
                    :loading="restoringSnapshotName === scope.row.name"
                  >
                    {{ $t('about.restoreSnapshot') }}
                  </el-button>
                </template>
              </el-table-column>
            </el-table>
          </el-form-item>
        </el-form>
      </el-tab-pane>
    </el-tabs>
  </el-card>
</template>

<script setup lang="ts">
import { computed, ref, onMounted, watch } from 'vue'
import { ElMessageBox, ElMessage } from 'element-plus'
import {
  GetConfig,
  ListConfigSnapshots,
  RestoreConfigSnapshot,
  SendTestAlert,
  type AlertingConfig,
  type AlertWebhookConfig,
  type ConfigSnapshotInfo,
} from '../api'
import { useI18n } from 'vue-i18n'

const { t } = useI18n()
const activeTab = ref<'general' | 'webhook' | 'snapshots'>('general')

const DEFAULT_CONNECT_TIMEOUT_MS = 5000
const DEFAULT_READ_TIMEOUT_MS = 30000
const DEFAULT_POOL_MAX_IDLE = 100
const DEFAULT_POOL_IDLE_TIMEOUT_SEC = 60
const DEFAULT_MAX_BODY_SIZE_MB = 10
const DEFAULT_MAX_RESPONSE_BODY_SIZE_MB = 10
const DEFAULT_ENABLE_HTTP2 = true
const DEFAULT_COMPRESSION_ENABLED = false
const DEFAULT_COMPRESSION_GZIP = true
const DEFAULT_COMPRESSION_BROTLI = true
const DEFAULT_COMPRESSION_MIN_LENGTH = 1024
const DEFAULT_COMPRESSION_GZIP_LEVEL = 6
const DEFAULT_COMPRESSION_BROTLI_LEVEL = 6
const DEFAULT_QUIET_HOURS_START = '23:00'
const DEFAULT_QUIET_HOURS_END = '08:00'
const DEFAULT_SYSTEM_REPORT_INTERVAL_MINUTES = 60
const DEFAULT_SYSTEM_REPORT_WEEKDAYS = [1, 2, 3, 4, 5, 6, 7]

interface AlertWebhookForm extends Omit<AlertWebhookConfig, 'system_report_interval_minutes'> {
  system_report_enabled: boolean
  quiet_hours_enabled: boolean
  quiet_hours_start: string
  quiet_hours_end: string
  system_report_interval_minutes: number | string
  system_report_weekdays: number[]
}

interface AlertingForm extends Omit<AlertingConfig, 'webhook'> {
  webhook: AlertWebhookForm
}

const autoStart = ref(false)
const showRealtimeLogs = ref(true)
const realtimeLogsOnlyErrors = ref(false)
const streamProxy = ref(true)
const enableHttp2 = ref(DEFAULT_ENABLE_HTTP2)
const maxBodySizeMB = ref(DEFAULT_MAX_BODY_SIZE_MB)
const maxResponseBodySizeMB = ref(DEFAULT_MAX_RESPONSE_BODY_SIZE_MB)
const upstreamConnectTimeoutMs = ref(DEFAULT_CONNECT_TIMEOUT_MS)
const upstreamReadTimeoutMs = ref(DEFAULT_READ_TIMEOUT_MS)
const upstreamPoolMaxIdle = ref(DEFAULT_POOL_MAX_IDLE)
const upstreamPoolIdleTimeoutSec = ref(DEFAULT_POOL_IDLE_TIMEOUT_SEC)
const compressionEnabled = ref(DEFAULT_COMPRESSION_ENABLED)
const compressionGzip = ref(DEFAULT_COMPRESSION_GZIP)
const compressionBrotli = ref(DEFAULT_COMPRESSION_BROTLI)
const compressionGzipLevel = ref(DEFAULT_COMPRESSION_GZIP_LEVEL)
const compressionBrotliLevel = ref(DEFAULT_COMPRESSION_BROTLI_LEVEL)

const sendingTestAlert = ref(false)
const loadingSnapshots = ref(false)
const restoringSnapshotName = ref('')
const snapshotList = ref<ConfigSnapshotInfo[]>([])
const systemReportIntervalOptions = [5, 10, 15, 30, 60, 120]
const weekdayOptions = computed(() => [
  { value: 1, label: t('about.weekdayMon') },
  { value: 2, label: t('about.weekdayTue') },
  { value: 3, label: t('about.weekdayWed') },
  { value: 4, label: t('about.weekdayThu') },
  { value: 5, label: t('about.weekdayFri') },
  { value: 6, label: t('about.weekdaySat') },
  { value: 7, label: t('about.weekdaySun') },
])

const alertForm = ref<AlertingForm>({
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
  const quietHoursStart = validateTimeValue(alertForm.value.webhook.quiet_hours_start, 'start')
  const quietHoursEnd = validateTimeValue(alertForm.value.webhook.quiet_hours_end, 'end')
  if (alertForm.value.webhook.quiet_hours_enabled && quietHoursStart === quietHoursEnd) {
    throw new Error(t('about.quietHoursSameTime'))
  }

  return {
    enabled: !!alertForm.value.enabled,
    webhook: {
      ...alertForm.value.webhook,
      enabled: !!alertForm.value.webhook.enabled,
      provider: (alertForm.value.webhook.provider || 'wecom').trim(),
      url: (alertForm.value.webhook.url || '').trim(),
      secret: (alertForm.value.webhook.secret || '').trim() || null,
      system_report_enabled: !!alertForm.value.webhook.system_report_enabled,
      quiet_hours_enabled: !!alertForm.value.webhook.quiet_hours_enabled,
      quiet_hours_start: quietHoursStart,
      quiet_hours_end: quietHoursEnd,
      system_report_interval_minutes: normalizeIntervalMinutes(alertForm.value.webhook.system_report_interval_minutes),
      system_report_weekdays: normalizeWeekdays(alertForm.value.webhook.system_report_weekdays),
    },
    rules: {
      server_start_error: alertForm.value.rules.server_start_error !== false,
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

    autoStart.value = false
    showRealtimeLogs.value = true
    realtimeLogsOnlyErrors.value = false
    streamProxy.value = true
    enableHttp2.value = DEFAULT_ENABLE_HTTP2
    maxBodySizeMB.value = DEFAULT_MAX_BODY_SIZE_MB
    maxResponseBodySizeMB.value = DEFAULT_MAX_RESPONSE_BODY_SIZE_MB
    upstreamConnectTimeoutMs.value = DEFAULT_CONNECT_TIMEOUT_MS
    upstreamReadTimeoutMs.value = DEFAULT_READ_TIMEOUT_MS
    upstreamPoolMaxIdle.value = DEFAULT_POOL_MAX_IDLE
    upstreamPoolIdleTimeoutSec.value = DEFAULT_POOL_IDLE_TIMEOUT_SEC
    compressionEnabled.value = DEFAULT_COMPRESSION_ENABLED
    compressionGzip.value = DEFAULT_COMPRESSION_GZIP
    compressionBrotli.value = DEFAULT_COMPRESSION_BROTLI
    compressionGzipLevel.value = DEFAULT_COMPRESSION_GZIP_LEVEL
    compressionBrotliLevel.value = DEFAULT_COMPRESSION_BROTLI_LEVEL

    window.dispatchEvent(new CustomEvent('toggle-realtime-logs', { detail: showRealtimeLogs.value }))
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
  loadingSnapshots.value = true
  try {
    snapshotList.value = await ListConfigSnapshots()
  } catch (e: any) {
    ElMessage.error(e?.message || String(e))
  } finally {
    loadingSnapshots.value = false
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

  restoringSnapshotName.value = name
  try {
    await RestoreConfigSnapshot(name)
    ElMessage.success(t('about.restoreSnapshotSuccess'))
    await loadSnapshots()
  } catch (e: any) {
    ElMessage.error(t('about.restoreSnapshotFailed', { error: e?.message || String(e) }))
  } finally {
    restoringSnapshotName.value = ''
  }
}

const handleSendTestAlert = async () => {
  if (!alertForm.value.enabled || !alertForm.value.webhook?.enabled) {
    ElMessage.warning(t('about.alertConfigIncomplete'))
    return
  }
  if (!alertForm.value.webhook.url?.trim()) {
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
    autoStart.value = !!configData.auto_start
    showRealtimeLogs.value = configData.show_realtime_logs !== false
    realtimeLogsOnlyErrors.value = !!configData.realtime_logs_only_errors
    streamProxy.value = configData.stream_proxy !== false
    enableHttp2.value = configData.enable_http2 !== false
    maxBodySizeMB.value = Math.round(((configData.max_body_size ?? DEFAULT_MAX_BODY_SIZE_MB * 1024 * 1024) / 1024 / 1024) * 100) / 100
    maxResponseBodySizeMB.value = Math.round(((configData.max_response_body_size ?? DEFAULT_MAX_RESPONSE_BODY_SIZE_MB * 1024 * 1024) / 1024 / 1024) * 100) / 100
    upstreamConnectTimeoutMs.value = configData.upstream_connect_timeout_ms ?? DEFAULT_CONNECT_TIMEOUT_MS
    upstreamReadTimeoutMs.value = configData.upstream_read_timeout_ms ?? DEFAULT_READ_TIMEOUT_MS
    upstreamPoolMaxIdle.value = configData.upstream_pool_max_idle ?? DEFAULT_POOL_MAX_IDLE
    upstreamPoolIdleTimeoutSec.value = configData.upstream_pool_idle_timeout_sec ?? DEFAULT_POOL_IDLE_TIMEOUT_SEC
    compressionEnabled.value = configData.compression_enabled ?? DEFAULT_COMPRESSION_ENABLED
    compressionGzip.value = configData.compression_gzip ?? DEFAULT_COMPRESSION_GZIP
    compressionBrotli.value = configData.compression_brotli ?? DEFAULT_COMPRESSION_BROTLI
    compressionGzipLevel.value = configData.compression_gzip_level ?? DEFAULT_COMPRESSION_GZIP_LEVEL
    compressionBrotliLevel.value = configData.compression_brotli_level ?? DEFAULT_COMPRESSION_BROTLI_LEVEL

    const alerting = configData?.alerting
    if (alerting) {
      const savedWeekdays = Array.isArray(alerting?.webhook?.system_report_weekdays)
        ? alerting.webhook.system_report_weekdays
        : null
      alertForm.value.enabled = !!alerting.enabled
      alertForm.value.webhook = {
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
      alertForm.value.rules = {
        server_start_error: alerting?.rules?.server_start_error !== false,
      }
    }
  } catch {
    // ignore
  }

  await loadSnapshots()
})

watch(showRealtimeLogs, (v) => {
  if (!v) {
    realtimeLogsOnlyErrors.value = false
  }
  window.dispatchEvent(new CustomEvent('toggle-realtime-logs', { detail: v }))
})

const getConfig = () => ({
  auto_start: !!autoStart.value,
  show_realtime_logs: !!showRealtimeLogs.value,
  realtime_logs_only_errors: !!realtimeLogsOnlyErrors.value,
  stream_proxy: !!streamProxy.value,
  enable_http2: !!enableHttp2.value,
  max_body_size: Math.floor(maxBodySizeMB.value * 1024 * 1024),
  max_response_body_size: Math.floor(maxResponseBodySizeMB.value * 1024 * 1024),
  upstream_connect_timeout_ms: Number(upstreamConnectTimeoutMs.value),
  upstream_read_timeout_ms: Number(upstreamReadTimeoutMs.value),
  upstream_pool_max_idle: Number(upstreamPoolMaxIdle.value),
  upstream_pool_idle_timeout_sec: Number(upstreamPoolIdleTimeoutSec.value),
  compression_enabled: !!compressionEnabled.value,
  compression_gzip: !!compressionGzip.value,
  compression_brotli: !!compressionBrotli.value,
  compression_min_length: DEFAULT_COMPRESSION_MIN_LENGTH,
  compression_gzip_level: Number(compressionGzipLevel.value),
  compression_brotli_level: Number(compressionBrotliLevel.value),
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
  background: var(--card);
}

.base-tabs :deep(.el-tabs__nav-wrap) {
  background: var(--card);
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
    --el-button-text-color: #5c3300;
    --el-button-hover-bg-color: #f9dcad;
    --el-button-hover-border-color: #bf7d1f;
    --el-button-hover-text-color: #452600;
    --el-button-active-bg-color: #f5d39f;
    --el-button-active-border-color: #a96610;
    --el-button-active-text-color: #351d00;
    color: #5c3300 !important;
    border-color: #d8942e !important;
    background: #fde9c8 !important;
    font-weight: 600;
  }
}
</style>
