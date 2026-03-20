<!-- frontend/src/components/MetricsStorage.vue -->
<template>
  <el-card class="config-card config-page" shadow="hover">
    <template #header>
      <h3>{{ $t('metricsStorage.title') }}</h3>
    </template>

    <el-form :model="localConfig" label-width="180px">
      <el-form-item :label="$t('metricsStorage.enable')">
        <el-switch v-model="localConfig.enabled" />
        <el-text type="info" size="small" class="hint">
          {{ $t('metricsStorage.enableHint') }}
        </el-text>
      </el-form-item>

      <el-form-item v-if="localConfig.enabled" :label="$t('metricsStorage.dbPath')">
        <div class="db-path-selector">
          <el-input
            v-model="localConfig.db_path"
            :placeholder="$t('metricsStorage.dbPathPlaceholder')"
          />
          <el-button type="danger" plain :icon="FolderAdd" @click="createDbFile">
            {{ $t('metricsStorage.createDb') }}
          </el-button>
          <el-button type="primary" plain :icon="FolderOpened" @click="loadDbFile">
            {{ $t('metricsStorage.loadDb') }}
          </el-button>
        </div>
        <el-text type="info" size="small" class="hint">
          {{ $t('metricsStorage.dbPathHint') }}
        </el-text>
      </el-form-item>

      <!-- 数据库状态显示 -->
      <el-card v-if="localConfig.enabled" class="status-card" shadow="never">
        <template #header>
          <span>{{ $t('metricsStorage.dbStatus') }}</span>
        </template>
        <div v-if="dbStatus" class="status-content">
          <el-alert
            v-if="dbStatus.initialized && dbStatus.file_exists"
            :title="$t('metricsStorage.dbReady')"
            type="success"
            :closable="false"
            show-icon
          >
            <template #default>
              <div class="status-grid">
                <el-descriptions :column="2" border class="status-detail-table">
                  <template #title>{{ $t('metricsStorage.fileAndCapacity') }}</template>
                  <el-descriptions-item :label="$t('metricsStorage.dbPathLabel')" :span="3">{{ dbStatus.path }}</el-descriptions-item>

                  <el-descriptions-item :label="$t('metricsStorage.dbMB')">{{ formatBytes(dbStatus.db_file_size_bytes) }}</el-descriptions-item>
                  <el-descriptions-item :label="$t('metricsStorage.walMB')">{{ formatBytes(dbStatus.wal_file_size_bytes) }}</el-descriptions-item>
                  <el-descriptions-item :label="$t('metricsStorage.shmMB')">{{ formatBytes(dbStatus.shm_file_size_bytes) }}</el-descriptions-item>

                  <el-descriptions-item :label="$t('metricsStorage.walShmMB')">{{ formatBytes(walShmBytesTotal) }}</el-descriptions-item>
                  <el-descriptions-item :label="$t('metricsStorage.totalPageMB')">{{ formatBytes(pageBytesTotal) }}</el-descriptions-item>
                  <el-descriptions-item :label="$t('metricsStorage.recyclableMB')">{{ formatBytes(freeBytesTotal) }}</el-descriptions-item>
                </el-descriptions>

                <el-descriptions :column="2" border class="status-detail-table">
                  <template #title>{{ $t('metricsStorage.logsAndTimeRange') }}</template>
                  <el-descriptions-item :label="$t('metricsStorage.recordCount')">{{ formatNumber(dbStatus.request_logs_count) }}</el-descriptions-item>
                  <el-descriptions-item :label="$t('metricsStorage.earliestRecord')">{{ formatTs(dbStatus.request_logs_min_ts) }}</el-descriptions-item>
                  <el-descriptions-item :label="$t('metricsStorage.latestRecord')">{{ formatTs(dbStatus.request_logs_max_ts) }}</el-descriptions-item>
                </el-descriptions>

                <el-descriptions :column="2" border class="status-detail-table">
                  <template #title>{{ $t('metricsStorage.sqliteConfig') }}</template>
                  <el-descriptions-item :label="$t('metricsStorage.sqliteVersion')">{{ dbStatus.sqlite_version || '—' }}</el-descriptions-item>
                  <el-descriptions-item :label="$t('metricsStorage.journalMode')">{{ dbStatus.journal_mode || '—' }}</el-descriptions-item>
                  <el-descriptions-item :label="$t('metricsStorage.synchronous')">{{ formatSynchronous(dbStatus.synchronous) }}</el-descriptions-item>

                  <el-descriptions-item :label="$t('metricsStorage.pageSize')">{{ formatNumber(dbStatus.page_size) }}</el-descriptions-item>
                  <el-descriptions-item :label="$t('metricsStorage.pageCount')">{{ formatNumber(dbStatus.page_count) }}</el-descriptions-item>
                  <el-descriptions-item :label="$t('metricsStorage.freelistCount')">{{ formatNumber(dbStatus.freelist_count) }}</el-descriptions-item>

                  <el-descriptions-item :label="$t('metricsStorage.fragmentationRate')">{{ fragRateText }}</el-descriptions-item>
                  <el-descriptions-item :label="$t('metricsStorage.cacheSize')">{{ formatCacheSize(dbStatus.cache_size) }}</el-descriptions-item>
                  <el-descriptions-item :label="$t('metricsStorage.busyTimeout')">{{ formatNumber(dbStatus.busy_timeout_ms) }}</el-descriptions-item>

                  <el-descriptions-item :label="$t('metricsStorage.walAutocheckpoint')">{{ formatNumber(dbStatus.wal_autocheckpoint) }}</el-descriptions-item>
                  <el-descriptions-item label="—">—</el-descriptions-item>
                  <el-descriptions-item label="—">—</el-descriptions-item>
                </el-descriptions>
              </div>
            </template>
          </el-alert>

          <el-alert
            v-else-if="dbStatus.initialized && !dbStatus.file_exists && dbStatus.dir_exists && dbStatus.dir_writable"
            :title="$t('metricsStorage.dbReadyWaiting')"
            type="success"
            :closable="false"
            show-icon
          >
            <template #default>
              <div class="status-detail">
                <p><strong>{{ $t('metricsStorage.dbPathLabel') }}：</strong>{{ dbStatus.path }}</p>
                <p><strong>{{ $t('metricsStorage.dirStatus') }}</strong></p>
                <p v-if="dbStatus.message" class="info-hint">{{ dbStatus.message }}</p>
              </div>
            </template>
          </el-alert>

          <el-alert
            v-else-if="dbStatus.error"
            :title="dbStatus.initialized ? $t('metricsStorage.dbConfigError') : $t('metricsStorage.dbInitFailed')"
            type="error"
            :closable="false"
            show-icon
          >
            <template #default>
              <div class="status-detail">
                <p v-if="dbStatus.path"><strong>{{ $t('metricsStorage.dbPathLabel') }}：</strong>{{ dbStatus.path }}</p>
                <p v-if="dbStatus.error"><strong>{{ $t('metricsStorage.errorInfo') }}</strong>{{ dbStatus.error }}</p>
                <p v-if="!dbStatus.dir_exists" class="error-hint">
                  {{ $t('metricsStorage.dirNotExists') }}
                </p>
                <p v-else-if="!dbStatus.dir_writable" class="error-hint">
                  {{ $t('metricsStorage.dirNotWritable') }}
                </p>
                <p v-else class="error-hint">{{ $t('metricsStorage.checkPathAndPermission') }}</p>
              </div>
            </template>
          </el-alert>

          <el-alert v-else :title="$t('metricsStorage.checkingStatus')" type="info" :closable="false" show-icon />
        </div>

        <el-button
          type="primary"
          size="small"
          @click="handleCheckDBStatus"
          :loading="checkingStatus"
          style="margin-top: 10px;"
        >
          {{ $t('metricsStorage.refreshStatus') }}
        </el-button>
      </el-card>

      <el-card v-if="localConfig.enabled" class="info-card" shadow="never">
        <template #header>
          <span>{{ $t('metricsStorage.dataDescription') }}</span>
        </template>
        <ul class="info-list">
          <li>{{ $t('metricsStorage.asyncWrite') }}</li>
          <li>{{ $t('metricsStorage.batchWrite') }}</li>
          <li>{{ $t('metricsStorage.retention') }}</li>
          <li>{{ $t('metricsStorage.connectionPool') }}</li>
        </ul>
      </el-card>
    </el-form>
  </el-card>
</template>

<script setup lang="ts">
import { computed, ref, watch, onMounted } from 'vue'
import { ElMessage, ElMessageBox } from 'element-plus'
import { FolderAdd, FolderOpened } from '@element-plus/icons-vue'
import { OpenDbFileDialog, OpenExistingDbFileDialog } from '../api'
import { useDBStatus } from '../composables/useDBStatus'
import { useI18n } from 'vue-i18n'

const { t, locale } = useI18n()

const props = defineProps<{
  config: any
}>()

const localConfig = ref({
  enabled: false,
  db_path: '',
})

const { dbStatus, loading: checkingStatus, checkDBStatus } = useDBStatus()

const formatNumber = (n: any) => {
  if (n === null || n === undefined) return '—'
  const num = Number(n)
  if (!Number.isFinite(num)) return '—'
  try {
    const localeStr = locale.value === 'zh-CN' ? 'zh-CN' : 'en-US'
    return new Intl.NumberFormat(localeStr).format(num)
  } catch {
    return String(num)
  }
}

const formatBytes = (bytes: any) => {
  if (bytes === null || bytes === undefined) return '—'
  const num = Number(bytes)
  if (!Number.isFinite(num) || num < 0) return '—'

  const units = ['B', 'KB', 'MB', 'GB', 'TB']
  let v = num
  let i = 0
  while (v >= 1024 && i < units.length - 1) {
    v /= 1024
    i++
  }
  return `${v.toFixed(i === 0 ? 0 : 2)} ${units[i]}`
}

const formatTs = (ts: any) => {
  if (ts === null || ts === undefined) return '—'
  const num = Number(ts)
  if (!Number.isFinite(num) || num <= 0) return '—'
  const localeStr = locale.value === 'zh-CN' ? 'zh-CN' : 'en-US'
  return new Date(num * 1000).toLocaleString(localeStr, { hour12: false })
}

const formatSynchronous = (v: any) => {
  if (v === null || v === undefined) return '—'
  const s = String(v).trim()
  if (!s) return '—'

  // PRAGMA synchronous 可能返回数字
  const n = Number(s)
  if (Number.isFinite(n)) {
    switch (n) {
      case 0:
        return 'OFF(0)'
      case 1:
        return 'NORMAL(1)'
      case 2:
        return 'FULL(2)'
      case 3:
        return 'EXTRA(3)'
      default:
        return `${n}`
    }
  }
  return s
}

const formatCacheSize = (v: any) => {
  if (v === null || v === undefined) return '—'
  const n = Number(v)
  if (!Number.isFinite(n)) return '—'

  // SQLite: cache_size > 0 表示页数；< 0 表示 KB
  if (n < 0) {
    return `${Math.abs(n)} KB`
  }
  return `${n} pages`
}

const pageBytesTotal = computed(() => {
  const ps = Number(dbStatus.value?.page_size)
  const pc = Number(dbStatus.value?.page_count)
  if (!Number.isFinite(ps) || !Number.isFinite(pc) || ps <= 0 || pc <= 0) return null
  return ps * pc
})

const freeBytesTotal = computed(() => {
  const ps = Number(dbStatus.value?.page_size)
  const fc = Number(dbStatus.value?.freelist_count)
  if (!Number.isFinite(ps) || !Number.isFinite(fc) || ps <= 0 || fc <= 0) return null
  return ps * fc
})

const walShmBytesTotal = computed(() => {
  const wal = Number(dbStatus.value?.wal_file_size_bytes) || 0
  const shm = Number(dbStatus.value?.shm_file_size_bytes) || 0
  const total = wal + shm
  return total > 0 ? total : null
})

const fragRateText = computed(() => {
  const pc = Number(dbStatus.value?.page_count)
  const fc = Number(dbStatus.value?.freelist_count)
  if (!Number.isFinite(pc) || !Number.isFinite(fc) || pc <= 0) return '—'
  const rate = (fc / pc) * 100
  return `${rate.toFixed(2)}%`
})

const createDbFile = async () => {
  try {
    await ElMessageBox.confirm(
      t('metricsStorage.createDbRiskMessage'),
      t('metricsStorage.createDbRiskTitle'),
      {
        type: 'warning',
        confirmButtonText: t('common.confirm'),
        cancelButtonText: t('common.cancel'),
      }
    )

    const filePath = await OpenDbFileDialog()
    if (filePath) {
      localConfig.value.db_path = String(filePath)
      window.dispatchEvent(
        new CustomEvent('save-config-request', {
          detail: { source: 'metrics-storage-create-db' },
        })
      )
    }
  } catch (error: any) {
    if (error === 'cancel' || error === 'close') {
      return
    }
    ElMessage.error(t('metricsStorage.createDbFileFailed', { error: error?.message || String(error) }))
  }
}

const loadDbFile = async () => {
  try {
    const filePath = await OpenExistingDbFileDialog()
    if (filePath) {
      localConfig.value.db_path = String(filePath)
      window.dispatchEvent(
        new CustomEvent('save-config-request', {
          detail: { source: 'metrics-storage-load-db' },
        })
      )
    }
  } catch (error: any) {
    ElMessage.error(t('metricsStorage.loadDbFileFailed', { error: error?.message || String(error) }))
  }
}

watch(
  () => props.config,
  (newConfig) => {
    if (!newConfig) return

    if (newConfig.metrics_storage) {
      localConfig.value.enabled = newConfig.metrics_storage.enabled || false
      localConfig.value.db_path = newConfig.metrics_storage.db_path || ''
    } else {
      localConfig.value.enabled = false
      localConfig.value.db_path = ''
    }
  },
  { immediate: true, deep: true },
)

const handleCheckDBStatus = async () => {
  await checkDBStatus(true)
}

const getConfig = () => {
  return {
    metrics_storage: {
      enabled: localConfig.value.enabled || false,
      db_path: localConfig.value.db_path || '',
    },
  }
}

watch(
  () => localConfig.value.enabled,
  (enabled) => {
    if (enabled) {
      setTimeout(() => {
        checkDBStatus(false)
      }, 1000)
    } else {
      dbStatus.value = null
    }
  },
)

watch(
  () => localConfig.value.db_path,
  () => {
    if (localConfig.value.enabled) {
      setTimeout(() => {
        checkDBStatus(true)
      }, 1000)
    }
  },
)

onMounted(() => {})

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

.el-form-item {
  margin-bottom: 24px;
}

:deep(.el-form-item__content) {
  display: flex;
  align-items: center;
  gap: 12px;
}

.hint {
  font-size: 12px;
  color: var(--text-muted);
  line-height: 1.4;
}

.db-path-selector {
  width: min(760px, 100%);
  display: flex;
  align-items: center;
  gap: 10px;
}

.db-path-selector :deep(.el-input) {
  flex: 1;
}

.status-card,
.info-card {
  margin-top: 24px;
  border-radius: var(--radius-md);
  border: 1px solid var(--border);
  background: var(--input-bg);
}

.status-card :deep(.el-card__header),
.info-card :deep(.el-card__header) {
  padding: 12px 16px;
  border-bottom: 1px solid var(--border);
  font-weight: 600;
}

.status-grid {
  display: grid;
  grid-template-columns: 1fr;
  gap: 16px;
}

.status-detail-table {
  border-radius: var(--radius-sm);
  overflow: hidden;
}

.status-detail-table :deep(.el-descriptions__label) {
  white-space: nowrap;
  width: 140px;
  color: var(--text);
}

.status-detail-table :deep(.el-descriptions__content) {
  word-break: break-all;
  color: var(--text);
}

.status-detail-table :deep(.el-descriptions__cell) {
  background: var(--card-bg);
  border-color: var(--border);
}

.status-detail-table :deep(.el-descriptions__header) {
  color: var(--text);
}

.info-list {
  margin: 0;
  padding-left: 20px;
  color: var(--text-muted);
  font-size: 13px;
  line-height: 1.8;
}

.info-list li {
  margin-bottom: 8px;
}

.status-detail {
  font-size: 13px;
  line-height: 1.8;
}

.status-detail p {
  margin: 5px 0;
}

.error-hint {
  color: var(--el-color-danger);
}

.info-hint {
  color: var(--el-color-info);
  font-style: italic;
}

@media (max-width: 980px) {
  .config-page :deep(.el-form-item__label) {
    width: 140px !important;
  }

  .db-path-selector {
    flex-direction: column;
    align-items: stretch;
  }
}
</style>
