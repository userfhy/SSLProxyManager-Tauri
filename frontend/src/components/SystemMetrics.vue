<template>
  <div class="config-card config-page system-metrics">
    <el-card shadow="hover" class="main-panel">
      <template #header>
        <div class="header">
          <div class="title-area">
            <h3>{{ $t('systemMetrics.title') }}</h3>
            <el-text type="info" size="small">
              {{ $t('systemMetrics.updateHint', { seconds: sampleIntervalSeconds }) }}
            </el-text>

            <div class="title-inline-controls">
              <el-form-item :label="$t('systemMetrics.sampleIntervalSec')" style="margin-bottom: 0;">
                <el-input-number
                  v-model="configSampleIntervalSecs"
                  :min="1"
                  :step="1"
                  :precision="0"
                  :controls="false"
                  size="small"
                  style="width: 120px;"
                />
              </el-form-item>

              <el-form-item :label="$t('systemMetrics.persistence')" style="margin-bottom: 0;">
                <el-switch
                  v-model="configPersistenceEnabled"
                  size="small"
                  :disabled="!globalPersistenceEnabled"
                />
              </el-form-item>
              <el-text v-if="!globalPersistenceEnabled" type="warning" size="small">
                {{ $t('systemMetrics.persistenceRequiresGlobal') }}
              </el-text>
            </div>
          </div>

          <div class="header-actions">
            <el-form-item :label="$t('systemMetrics.window')" style="margin-bottom: 0;">
              <el-select v-model.number="selectedWindow" size="small" style="width: 140px;" :disabled="isHistoricalMode">
                <el-option :label="$t('systemMetrics.oneMinute')" :value="60" />
                <el-option :label="$t('systemMetrics.fiveMinutes')" :value="300" />
                <el-option :label="$t('systemMetrics.fifteenMinutes')" :value="900" />
                <el-option :label="$t('systemMetrics.oneHour')" :value="3600" />
                <el-option :label="$t('systemMetrics.sixHours')" :value="21600" />
                <el-option :label="$t('systemMetrics.twelveHours')" :value="43200" />
                <el-option :label="$t('systemMetrics.twentyFourHours')" :value="86400" />
                <el-option :label="$t('systemMetrics.threeDays')" :value="259200" />
                <el-option :label="$t('systemMetrics.sevenDays')" :value="604800" />
              </el-select>
            </el-form-item>

            <el-form-item :label="$t('systemMetrics.chartUnit')" style="margin-bottom: 0;">
              <el-select v-model="rateUnit" size="small" style="width: 100px;">
                <el-option
                  v-for="opt in rateUnitOptions"
                  :key="opt.value"
                  :label="opt.label"
                  :value="opt.value"
                />
              </el-select>
            </el-form-item>

            <el-form-item :label="$t('systemMetrics.historicalData')" style="margin-bottom: 0;">
              <el-config-provider :locale="datePickerLocale">
                <el-date-picker
                  v-model="dateRange"
                  type="datetimerange"
                  size="small"
                  :range-separator="$t('systemMetrics.to')"
                  :start-placeholder="$t('systemMetrics.startTime')"
                  :end-placeholder="$t('systemMetrics.endTime')"
                  format="YYYY-MM-DD HH:mm:ss"
                  value-format="x"
                  :shortcuts="dateShortcuts"
                  style="width: 380px;"
                />
              </el-config-provider>
            </el-form-item>

            <el-form-item style="margin-bottom: 0;">
              <el-button type="primary" size="small" :loading="loadingHistorical" @click="loadHistoricalData">
                {{ $t('systemMetrics.loadHistorical') }}
              </el-button>
              <el-button v-if="isHistoricalMode" size="small" @click="clearHistoricalData">
                {{ $t('systemMetrics.showRealtime') }}
              </el-button>
              <el-button size="small" :loading="loadingRealtime" @click="loadRealtimeSnapshot">
                {{ $t('systemMetrics.refreshNow') }}
              </el-button>
            </el-form-item>
          </div>
        </div>
      </template>

      <el-alert
        v-if="unsupported"
        type="warning"
        :closable="false"
        show-icon
        :title="$t('systemMetrics.unsupported', { message: unsupportedMessage })"
        style="margin-bottom: 12px;"
      />

      <el-alert
        v-else-if="errorMessage"
        type="error"
        :closable="false"
        show-icon
        :title="$t('systemMetrics.fetchFailed', { error: errorMessage })"
        style="margin-bottom: 12px;"
      />

      <el-alert
        v-if="isHistoricalMode"
        type="info"
        :closable="false"
        show-icon
        :title="$t('systemMetrics.historicalMode')"
        style="margin-bottom: 12px;"
      />

      <div class="stats-grid">
        <el-card class="stat-card" shadow="never">
          <div class="stat-label">{{ $t('systemMetrics.cpuUsage') }}</div>
          <div class="stat-value">{{ formatPercent(currentPoint?.cpu_usage_percent || 0) }}</div>
        </el-card>
        <el-card class="stat-card" shadow="never">
          <div class="stat-label">{{ $t('systemMetrics.memUsage') }}</div>
          <div class="stat-value">{{ formatPercent(currentPoint?.mem_used_percent || 0) }}</div>
        </el-card>
        <el-card class="stat-card" shadow="never">
          <div class="stat-label">{{ $t('systemMetrics.swapUsage') }}</div>
          <div class="stat-value">{{ formatPercent(currentPoint?.swap_used_percent || 0) }}</div>
        </el-card>
        <el-card class="stat-card" shadow="never">
          <div class="stat-label">{{ $t('systemMetrics.load1') }}</div>
          <div class="stat-value">{{ formatNumber(currentPoint?.load1 || 0, 2) }}</div>
        </el-card>
        <el-card class="stat-card" shadow="never">
          <div class="stat-label">{{ $t('systemMetrics.load5') }}</div>
          <div class="stat-value">{{ formatNumber(currentPoint?.load5 || 0, 2) }}</div>
        </el-card>
        <el-card class="stat-card" shadow="never">
          <div class="stat-label">{{ $t('systemMetrics.load15') }}</div>
          <div class="stat-value">{{ formatNumber(currentPoint?.load15 || 0, 2) }}</div>
        </el-card>

        <el-card class="stat-card" shadow="never">
          <div class="stat-label">{{ $t('systemMetrics.inboundRate') }}</div>
          <div class="stat-value">{{ formatRate(currentPoint?.net_rx_bps || 0) }}</div>
        </el-card>
        <el-card class="stat-card" shadow="never">
          <div class="stat-label">{{ $t('systemMetrics.outboundRate') }}</div>
          <div class="stat-value">{{ formatRate(currentPoint?.net_tx_bps || 0) }}</div>
        </el-card>
        <el-card class="stat-card" shadow="never">
          <div class="stat-label">{{ $t('systemMetrics.totalInbound') }}</div>
          <div class="stat-value">{{ formatBytes(currentPoint?.net_rx_bytes || 0) }}</div>
        </el-card>
        <el-card class="stat-card" shadow="never">
          <div class="stat-label">{{ $t('systemMetrics.totalOutbound') }}</div>
          <div class="stat-value">{{ formatBytes(currentPoint?.net_tx_bytes || 0) }}</div>
        </el-card>
        <el-card class="stat-card" shadow="never">
          <div class="stat-label">{{ $t('systemMetrics.diskReadRate') }}</div>
          <div class="stat-value">{{ formatRate(currentPoint?.disk_read_bps || 0) }}</div>
        </el-card>
        <el-card class="stat-card" shadow="never">
          <div class="stat-label">{{ $t('systemMetrics.diskWriteRate') }}</div>
          <div class="stat-value">{{ formatRate(currentPoint?.disk_write_bps || 0) }}</div>
        </el-card>

        <el-card class="stat-card" shadow="never">
          <div class="stat-label">{{ $t('systemMetrics.tcpEstablished') }}</div>
          <div class="stat-value">{{ currentPoint?.tcp_established || 0 }}</div>
        </el-card>
        <el-card class="stat-card" shadow="never">
          <div class="stat-label">{{ $t('systemMetrics.tcpTimeWait') }}</div>
          <div class="stat-value">{{ currentPoint?.tcp_time_wait || 0 }}</div>
        </el-card>
        <el-card class="stat-card" shadow="never">
          <div class="stat-label">{{ $t('systemMetrics.tcpCloseWait') }}</div>
          <div class="stat-value">{{ currentPoint?.tcp_close_wait || 0 }}</div>
        </el-card>
        <el-card class="stat-card" shadow="never">
          <div class="stat-label">{{ $t('systemMetrics.processCount') }}</div>
          <div class="stat-value">{{ currentPoint?.process_count || 0 }}</div>
        </el-card>
        <el-card class="stat-card" shadow="never">
          <div class="stat-label">{{ $t('systemMetrics.fdUsage') }}</div>
          <div class="stat-value">{{ formatPercent(currentPoint?.fd_usage_percent || 0) }}</div>
        </el-card>
      </div>

      <el-descriptions :column="4" border size="small" class="meta">
        <el-descriptions-item :label="$t('systemMetrics.lastUpdate')">
          {{ formatDateTime(currentPoint?.timestamp || null) }}
        </el-descriptions-item>
        <el-descriptions-item :label="$t('systemMetrics.interfaceCount')">
          {{ latestInterfaces.length }}
        </el-descriptions-item>
        <el-descriptions-item :label="$t('systemMetrics.uptime')">
          {{ formatUptime(currentPoint?.uptime_seconds || 0) }}
        </el-descriptions-item>
        <el-descriptions-item :label="$t('systemMetrics.fdUsedMax')">
          {{ currentPoint ? `${currentPoint.fd_used || 0} / ${currentPoint.fd_max || 0}` : '-' }}
        </el-descriptions-item>
        <el-descriptions-item :label="$t('systemMetrics.procsRunningBlocked')">
          {{ currentPoint ? `${currentPoint.procs_running || 0} / ${currentPoint.procs_blocked || 0}` : '-' }}
        </el-descriptions-item>
        <el-descriptions-item :label="$t('systemMetrics.contextSwitches')">
          {{ currentPoint?.context_switches || 0 }}
        </el-descriptions-item>
        <el-descriptions-item :label="$t('systemMetrics.forksTotal')">
          {{ currentPoint?.processes_forked_total || 0 }}
        </el-descriptions-item>
        <el-descriptions-item :label="$t('systemMetrics.memUsedTotal')">
          {{ `${formatBytes(currentPoint?.mem_used_bytes || 0)} / ${formatBytes(currentPoint?.mem_total_bytes || 0)}` }}
        </el-descriptions-item>
        <el-descriptions-item :label="$t('systemMetrics.swapUsedTotal')">
          {{ `${formatBytes(currentPoint?.swap_used_bytes || 0)} / ${formatBytes(currentPoint?.swap_total_bytes || 0)}` }}
        </el-descriptions-item>
        <el-descriptions-item :label="$t('systemMetrics.diskReadTotal')">
          {{ formatBytes(currentPoint?.disk_read_bytes || 0) }}
        </el-descriptions-item>
        <el-descriptions-item :label="$t('systemMetrics.diskWriteTotal')">
          {{ formatBytes(currentPoint?.disk_write_bytes || 0) }}
        </el-descriptions-item>
      </el-descriptions>

      <div class="charts-grid">
        <el-card class="chart-panel" shadow="never">
          <template #header>
            <div class="panel-title">{{ $t('systemMetrics.resourceTrend') }}</div>
          </template>
          <v-chart v-if="isActive && activePoints.length > 0" :option="resourceOption" class="chart" autoresize />
          <el-empty v-else :description="$t('systemMetrics.noData')" :image-size="64" />
        </el-card>

        <el-card class="chart-panel" shadow="never">
          <template #header>
            <div class="panel-title">{{ networkTrendTitle }}</div>
          </template>
          <v-chart v-if="isActive && activePoints.length > 0" :option="networkOption" class="chart" autoresize />
          <el-empty v-else :description="$t('systemMetrics.noData')" :image-size="64" />
        </el-card>

        <el-card class="chart-panel" shadow="never">
          <template #header>
            <div class="panel-title">{{ diskTrendTitle }}</div>
          </template>
          <v-chart v-if="isActive && activePoints.length > 0" :option="diskOption" class="chart" autoresize />
          <el-empty v-else :description="$t('systemMetrics.noData')" :image-size="64" />
        </el-card>

        <el-card class="chart-panel" shadow="never">
          <template #header>
            <div class="panel-title">{{ $t('systemMetrics.loadTrend') }}</div>
          </template>
          <v-chart v-if="isActive && activePoints.length > 0" :option="loadOption" class="chart" autoresize />
          <el-empty v-else :description="$t('systemMetrics.noData')" :image-size="64" />
        </el-card>

        <el-card class="chart-panel" shadow="never">
          <template #header>
            <div class="panel-title">{{ $t('systemMetrics.connectionTrend') }}</div>
          </template>
          <v-chart v-if="isActive && activePoints.length > 0" :option="connectionOption" class="chart" autoresize />
          <el-empty v-else :description="$t('systemMetrics.noData')" :image-size="64" />
        </el-card>
      </div>

      <el-card class="table-panel" shadow="never">
        <template #header>
          <div class="panel-title">{{ $t('systemMetrics.interfaces') }}</div>
        </template>
        <el-table :data="latestInterfaces" size="small" max-height="280">
          <el-table-column prop="name" :label="$t('systemMetrics.interfaceName')" min-width="140" />
          <el-table-column :label="$t('systemMetrics.rxBytes')" min-width="160">
            <template #default="{ row }">{{ formatBytes(row.rx_bytes) }}</template>
          </el-table-column>
          <el-table-column :label="$t('systemMetrics.txBytes')" min-width="160">
            <template #default="{ row }">{{ formatBytes(row.tx_bytes) }}</template>
          </el-table-column>
        </el-table>
      </el-card>
    </el-card>
  </div>
</template>

<script setup lang="ts">
import { computed, onBeforeUnmount, onMounted, ref, watch } from 'vue'
import { ElConfigProvider, ElMessage } from 'element-plus'
import zhCn from 'element-plus/dist/locale/zh-cn.mjs'
import enUs from 'element-plus/dist/locale/en.mjs'
import { useI18n } from 'vue-i18n'
import {
  EventsOn,
  EventsOff,
  GetSystemMetrics,
  QueryHistoricalSystemMetrics,
  SetSystemMetricsSubscription,
} from '../api'
import { useDateShortcuts } from '../composables/useDateShortcuts'
import VChart from 'vue-echarts'
import { use } from 'echarts/core'
import { CanvasRenderer } from 'echarts/renderers'
import { LineChart } from 'echarts/charts'
import { GridComponent, TooltipComponent, LegendComponent } from 'echarts/components'
import type { EChartsOption } from 'echarts'

type NetworkInterfaceStats = {
  name: string
  rx_bytes: number
  tx_bytes: number
}

type SystemMetricsPoint = {
  timestamp: number
  cpu_usage_percent: number
  load1: number
  load5: number
  load15: number
  mem_total_bytes: number
  mem_available_bytes: number
  mem_used_bytes: number
  mem_used_percent: number
  swap_total_bytes: number
  swap_free_bytes: number
  swap_used_bytes: number
  swap_used_percent: number
  net_rx_bytes: number
  net_tx_bytes: number
  net_rx_bps: number
  net_tx_bps: number
  disk_read_bytes: number
  disk_write_bytes: number
  disk_read_bps: number
  disk_write_bps: number
  tcp_established: number
  tcp_time_wait: number
  tcp_close_wait: number
  process_count: number
  fd_used: number
  fd_max: number
  fd_usage_percent: number
  procs_running: number
  procs_blocked: number
  context_switches: number
  processes_forked_total: number
  uptime_seconds: number
}

type SystemMetricsRealtimePayload = {
  sample_interval_seconds: number
  max_window_seconds: number
  supported: boolean
  message?: string
  latest?: SystemMetricsPoint
  points: SystemMetricsPoint[]
  interfaces: NetworkInterfaceStats[]
}

type QuerySystemMetricsResponse = {
  points: SystemMetricsPoint[]
  supported: boolean
  message?: string
}

type SystemMetricsEventPayload = {
  point?: SystemMetricsPoint
  interfaces?: NetworkInterfaceStats[]
}

type RateUnit = 'B' | 'KB' | 'MB'

use([CanvasRenderer, LineChart, GridComponent, TooltipComponent, LegendComponent])

const props = defineProps<{ isActive: boolean; config?: any }>()

const { t, locale } = useI18n()
const { dateShortcuts } = useDateShortcuts()
const datePickerLocale = computed(() => (locale.value === 'en-US' ? enUs : zhCn))
const isActive = computed(() => props.isActive)

const selectedWindow = ref<number>(86400)
const sampleIntervalSeconds = ref<number>(5)
const maxWindowSeconds = ref<number>(7 * 24 * 3600)
const configSampleIntervalSecs = ref<number>(10)
const configPersistenceEnabled = ref<boolean>(false)
const rateUnit = ref<RateUnit>('KB')
const globalPersistenceEnabled = computed(() => !!props.config?.metrics_storage?.enabled)

const loadingRealtime = ref(false)
const loadingHistorical = ref(false)
const unsupported = ref(false)
const unsupportedMessage = ref('')
const errorMessage = ref('')

const dateRange = ref<[number, number] | null>(null)
const realtimePoints = ref<SystemMetricsPoint[]>([])
const historicalPoints = ref<SystemMetricsPoint[] | null>(null)
const latestPoint = ref<SystemMetricsPoint | null>(null)
const latestInterfaces = ref<NetworkInterfaceStats[]>([])

let metricsUnlisten: (() => void) | null = null
let themeObserver: MutationObserver | null = null

const chartColors = ref({
  textMuted: '#94a3b8',
  border: 'rgba(148, 163, 184, 0.25)',
  primary: '#4f9cf9',
  success: '#22c55e',
  warning: '#f59e0b',
  danger: '#ef4444',
  info: '#0ea5e9',
})

const updateChartColors = () => {
  const style = getComputedStyle(document.documentElement)
  chartColors.value = {
    textMuted: style.getPropertyValue('--text-muted').trim() || '#94a3b8',
    border: style.getPropertyValue('--border').trim() || 'rgba(148, 163, 184, 0.25)',
    primary: style.getPropertyValue('--primary').trim() || '#4f9cf9',
    success: style.getPropertyValue('--success').trim() || '#22c55e',
    warning: style.getPropertyValue('--warning').trim() || '#f59e0b',
    danger: style.getPropertyValue('--danger').trim() || '#ef4444',
    info: '#0ea5e9',
  }
}

const formatNumber = (v: number, digits = 2) => {
  if (!Number.isFinite(v)) return '0'
  return Number(v).toFixed(digits)
}

const formatPercent = (v: number) => `${formatNumber(v, 2)}%`

const formatBytes = (bytes: number) => {
  if (!Number.isFinite(bytes) || bytes < 0) return '0 B'
  const units = ['B', 'KB', 'MB', 'GB', 'TB']
  let value = bytes
  let unit = 0
  while (value >= 1024 && unit < units.length - 1) {
    value /= 1024
    unit += 1
  }
  return `${value.toFixed(unit === 0 ? 0 : 2)} ${units[unit]}`
}

const formatRate = (bps: number) => `${formatBytes(bps)}/s`

const rateUnitOptions = computed(() => ([
  { value: 'B' as RateUnit, label: t('systemMetrics.unitBps') },
  { value: 'KB' as RateUnit, label: t('systemMetrics.unitKBps') },
  { value: 'MB' as RateUnit, label: t('systemMetrics.unitMBps') },
]))

const rateUnitLabel = computed(() => {
  if (rateUnit.value === 'MB') return t('systemMetrics.unitMBps')
  if (rateUnit.value === 'KB') return t('systemMetrics.unitKBps')
  return t('systemMetrics.unitBps')
})

const convertRateByUnit = (bytesPerSec: number) => {
  if (rateUnit.value === 'MB') {
    return Number((bytesPerSec / (1024 * 1024)).toFixed(2))
  }
  if (rateUnit.value === 'KB') {
    return Number((bytesPerSec / 1024).toFixed(2))
  }
  return Number(bytesPerSec.toFixed(2))
}

const formatDateTime = (tsSec: number | null) => {
  if (!tsSec) return '-'
  const localeCode = locale.value === 'zh-CN' ? 'zh-CN' : 'en-US'
  return new Date(tsSec * 1000).toLocaleString(localeCode, { hour12: false })
}

const formatUptime = (uptimeSec: number) => {
  if (!Number.isFinite(uptimeSec) || uptimeSec <= 0) return '-'
  const total = Math.floor(uptimeSec)
  const d = Math.floor(total / 86400)
  const h = Math.floor((total % 86400) / 3600)
  const m = Math.floor((total % 3600) / 60)
  const s = total % 60
  if (d > 0) return `${d}d ${h}h ${m}m ${s}s`
  if (h > 0) return `${h}h ${m}m ${s}s`
  return `${m}m ${s}s`
}

const formatAxisClock = (tsSec: number) => {
  const d = new Date(tsSec * 1000)
  const hh = String(d.getHours()).padStart(2, '0')
  const mm = String(d.getMinutes()).padStart(2, '0')
  const ss = String(d.getSeconds()).padStart(2, '0')
  return `${hh}:${mm}:${ss}`
}

const downsample = (arr: SystemMetricsPoint[], maxPoints = 1200): SystemMetricsPoint[] => {
  if (arr.length <= maxPoints) return arr
  const step = Math.ceil(arr.length / maxPoints)
  const out: SystemMetricsPoint[] = []
  for (let i = 0; i < arr.length; i += step) {
    out.push(arr[i])
  }
  const last = arr[arr.length - 1]
  if (!out.length || out[out.length - 1].timestamp !== last.timestamp) {
    out.push(last)
  }
  return out
}

const isHistoricalMode = computed(() => historicalPoints.value !== null)

const activePoints = computed<SystemMetricsPoint[]>(() => {
  if (historicalPoints.value) {
    return downsample(historicalPoints.value)
  }

  const now = Math.floor(Date.now() / 1000)
  const minTs = now - selectedWindow.value
  const filtered = realtimePoints.value.filter((p) => p.timestamp >= minTs)
  return downsample(filtered)
})

const adaptiveChartMaxPoints = (spanSec: number) => {
  if (spanSec <= 60 * 60) return 420
  if (spanSec <= 6 * 60 * 60) return 360
  if (spanSec <= 24 * 60 * 60) return 300
  if (spanSec <= 3 * 24 * 60 * 60) return 240
  if (spanSec <= 7 * 24 * 60 * 60) return 200
  return 160
}

const aggregateChartPoints = (points: SystemMetricsPoint[], maxPoints: number): SystemMetricsPoint[] => {
  if (points.length <= maxPoints || points.length <= 1) {
    return points
  }

  const firstTs = points[0].timestamp
  const lastTs = points[points.length - 1].timestamp
  const spanSec = Math.max(1, lastTs - firstTs)
  const bucketSec = Math.max(1, Math.ceil(spanSec / maxPoints))

  type BucketAgg = {
    count: number
    sumCpu: number
    sumMem: number
    sumSwap: number
    sumNetRxBps: number
    sumNetTxBps: number
    sumDiskReadBps: number
    sumDiskWriteBps: number
    sumTcpEstablished: number
    sumTcpTimeWait: number
    sumProcessCount: number
    last: SystemMetricsPoint
  }

  const buckets = new Map<number, BucketAgg>()
  for (const p of points) {
    const key = Math.floor(p.timestamp / bucketSec) * bucketSec
    const existing = buckets.get(key)
    if (!existing) {
      buckets.set(key, {
        count: 1,
        sumCpu: p.cpu_usage_percent,
        sumMem: p.mem_used_percent,
        sumSwap: p.swap_used_percent,
        sumNetRxBps: p.net_rx_bps,
        sumNetTxBps: p.net_tx_bps,
        sumDiskReadBps: p.disk_read_bps,
        sumDiskWriteBps: p.disk_write_bps,
        sumTcpEstablished: p.tcp_established,
        sumTcpTimeWait: p.tcp_time_wait,
        sumProcessCount: p.process_count,
        last: p,
      })
      continue
    }
    existing.count += 1
    existing.sumCpu += p.cpu_usage_percent
    existing.sumMem += p.mem_used_percent
    existing.sumSwap += p.swap_used_percent
    existing.sumNetRxBps += p.net_rx_bps
    existing.sumNetTxBps += p.net_tx_bps
    existing.sumDiskReadBps += p.disk_read_bps
    existing.sumDiskWriteBps += p.disk_write_bps
    existing.sumTcpEstablished += p.tcp_established
    existing.sumTcpTimeWait += p.tcp_time_wait
    existing.sumProcessCount += p.process_count
    existing.last = p
  }

  const out = Array.from(buckets.entries())
    .sort((a, b) => a[0] - b[0])
    .map(([bucketTs, agg]) => {
      const base = { ...agg.last }
      const c = agg.count || 1
      base.timestamp = bucketTs
      base.cpu_usage_percent = agg.sumCpu / c
      base.mem_used_percent = agg.sumMem / c
      base.swap_used_percent = agg.sumSwap / c
      base.net_rx_bps = agg.sumNetRxBps / c
      base.net_tx_bps = agg.sumNetTxBps / c
      base.disk_read_bps = agg.sumDiskReadBps / c
      base.disk_write_bps = agg.sumDiskWriteBps / c
      base.tcp_established = Math.round(agg.sumTcpEstablished / c)
      base.tcp_time_wait = Math.round(agg.sumTcpTimeWait / c)
      base.process_count = Math.round(agg.sumProcessCount / c)
      return base
    })

  return out
}

const chartPoints = computed<SystemMetricsPoint[]>(() => {
  if (activePoints.value.length <= 1) {
    return activePoints.value
  }
  const spanSec = Math.max(
    1,
    activePoints.value[activePoints.value.length - 1].timestamp - activePoints.value[0].timestamp,
  )
  return aggregateChartPoints(activePoints.value, adaptiveChartMaxPoints(spanSec))
})

const currentPoint = computed<SystemMetricsPoint | null>(() => {
  const arr = activePoints.value
  if (arr.length > 0) {
    return arr[arr.length - 1]
  }
  return latestPoint.value
})

const chartDerived = computed(() => {
  const points = chartPoints.value
  return {
    xAxis: points.map((p) => formatAxisClock(p.timestamp)),
    cpu: points.map((p) => Number(p.cpu_usage_percent.toFixed(2))),
    load1: points.map((p) => Number((p.load1 || 0).toFixed(2))),
    load5: points.map((p) => Number((p.load5 || 0).toFixed(2))),
    load15: points.map((p) => Number((p.load15 || 0).toFixed(2))),
    mem: points.map((p) => Number(p.mem_used_percent.toFixed(2))),
    swap: points.map((p) => Number(p.swap_used_percent.toFixed(2))),
    netRx: points.map((p) => convertRateByUnit(p.net_rx_bps)),
    netTx: points.map((p) => convertRateByUnit(p.net_tx_bps)),
    diskRead: points.map((p) => convertRateByUnit(p.disk_read_bps)),
    diskWrite: points.map((p) => convertRateByUnit(p.disk_write_bps)),
    tcpEstablished: points.map((p) => p.tcp_established || 0),
    tcpTimeWait: points.map((p) => p.tcp_time_wait || 0),
    processCount: points.map((p) => p.process_count || 0),
  }
})

const networkTrendTitle = computed(() => t('systemMetrics.networkTrend', { unit: rateUnitLabel.value }))
const diskTrendTitle = computed(() => t('systemMetrics.diskTrend', { unit: rateUnitLabel.value }))

const baseChart = computed(() => ({
  backgroundColor: 'transparent',
  animation: false,
  grid: { left: 50, right: 12, top: 56, bottom: 42 },
  tooltip: { trigger: 'axis' as const, confine: true },
  legend: {
    top: 0,
    left: 'center',
    itemWidth: 12,
    itemHeight: 8,
    itemGap: 12,
    textStyle: {
      color: chartColors.value.textMuted,
      fontSize: 12,
    },
  },
  xAxis: {
    type: 'category' as const,
    boundaryGap: false,
    data: chartDerived.value.xAxis,
    axisLabel: {
      color: chartColors.value.textMuted,
      hideOverlap: true,
      interval: 'auto' as const,
      margin: 10,
      fontSize: 11,
    },
    axisLine: { lineStyle: { color: chartColors.value.border } },
  },
  yAxis: {
    type: 'value' as const,
    axisLabel: { color: chartColors.value.textMuted },
    axisLine: { lineStyle: { color: chartColors.value.border } },
    splitLine: { lineStyle: { color: chartColors.value.border, type: 'dashed' as const } },
  },
}))

const resourceOption = computed<EChartsOption>(() => ({
  ...baseChart.value,
  legend: {
    ...baseChart.value.legend,
    data: [t('systemMetrics.cpuUsage'), t('systemMetrics.memUsage'), t('systemMetrics.swapUsage')],
  },
  series: [
    {
      name: t('systemMetrics.cpuUsage'),
      type: 'line',
      smooth: true,
      showSymbol: false,
      data: chartDerived.value.cpu,
      lineStyle: { width: 2, color: chartColors.value.primary },
      itemStyle: { color: chartColors.value.primary },
    },
    {
      name: t('systemMetrics.memUsage'),
      type: 'line',
      smooth: true,
      showSymbol: false,
      data: chartDerived.value.mem,
      lineStyle: { width: 2, color: chartColors.value.success },
      itemStyle: { color: chartColors.value.success },
    },
    {
      name: t('systemMetrics.swapUsage'),
      type: 'line',
      smooth: true,
      showSymbol: false,
      data: chartDerived.value.swap,
      lineStyle: { width: 2, color: chartColors.value.warning },
      itemStyle: { color: chartColors.value.warning },
    },
  ],
}))

const networkOption = computed<EChartsOption>(() => ({
  ...baseChart.value,
  tooltip: {
    ...(baseChart.value.tooltip as any),
    valueFormatter: (value: number) => `${formatNumber(Number(value) || 0, 2)} ${rateUnitLabel.value}`,
  },
  legend: {
    ...baseChart.value.legend,
    data: [t('systemMetrics.inbound'), t('systemMetrics.outbound')],
  },
  yAxis: {
    ...(baseChart.value.yAxis as any),
    axisLabel: {
      color: chartColors.value.textMuted,
      formatter: (v: number) => formatNumber(v, 2),
    },
  },
  series: [
    {
      name: t('systemMetrics.inbound'),
      type: 'line',
      smooth: true,
      showSymbol: false,
      data: chartDerived.value.netRx,
      lineStyle: { width: 2, color: chartColors.value.success },
      itemStyle: { color: chartColors.value.success },
      areaStyle: { opacity: 0.08, color: chartColors.value.success },
    },
    {
      name: t('systemMetrics.outbound'),
      type: 'line',
      smooth: true,
      showSymbol: false,
      data: chartDerived.value.netTx,
      lineStyle: { width: 2, color: chartColors.value.primary },
      itemStyle: { color: chartColors.value.primary },
      areaStyle: { opacity: 0.08, color: chartColors.value.primary },
    },
  ],
}))

const diskOption = computed<EChartsOption>(() => ({
  ...baseChart.value,
  tooltip: {
    ...(baseChart.value.tooltip as any),
    valueFormatter: (value: number) => `${formatNumber(Number(value) || 0, 2)} ${rateUnitLabel.value}`,
  },
  legend: {
    ...baseChart.value.legend,
    data: [t('systemMetrics.diskReadRate'), t('systemMetrics.diskWriteRate')],
  },
  yAxis: {
    ...(baseChart.value.yAxis as any),
    axisLabel: {
      color: chartColors.value.textMuted,
      formatter: (v: number) => formatNumber(v, 2),
    },
  },
  series: [
    {
      name: t('systemMetrics.diskReadRate'),
      type: 'line',
      smooth: true,
      showSymbol: false,
      data: chartDerived.value.diskRead,
      lineStyle: { width: 2, color: chartColors.value.info },
      itemStyle: { color: chartColors.value.info },
    },
    {
      name: t('systemMetrics.diskWriteRate'),
      type: 'line',
      smooth: true,
      showSymbol: false,
      data: chartDerived.value.diskWrite,
      lineStyle: { width: 2, color: chartColors.value.danger },
      itemStyle: { color: chartColors.value.danger },
    },
  ],
}))

const loadOption = computed<EChartsOption>(() => ({
  ...baseChart.value,
  legend: {
    ...baseChart.value.legend,
    data: [t('systemMetrics.load1'), t('systemMetrics.load5'), t('systemMetrics.load15')],
  },
  series: [
    {
      name: t('systemMetrics.load1'),
      type: 'line',
      smooth: true,
      showSymbol: false,
      data: chartDerived.value.load1,
      lineStyle: { width: 2, color: chartColors.value.primary },
      itemStyle: { color: chartColors.value.primary },
    },
    {
      name: t('systemMetrics.load5'),
      type: 'line',
      smooth: true,
      showSymbol: false,
      data: chartDerived.value.load5,
      lineStyle: { width: 2, color: chartColors.value.success },
      itemStyle: { color: chartColors.value.success },
    },
    {
      name: t('systemMetrics.load15'),
      type: 'line',
      smooth: true,
      showSymbol: false,
      data: chartDerived.value.load15,
      lineStyle: { width: 2, color: chartColors.value.warning },
      itemStyle: { color: chartColors.value.warning },
    },
  ],
}))

const connectionOption = computed<EChartsOption>(() => ({
  ...baseChart.value,
  legend: {
    ...baseChart.value.legend,
    data: [t('systemMetrics.tcpEstablished'), t('systemMetrics.tcpTimeWait'), t('systemMetrics.processCount')],
  },
  series: [
    {
      name: t('systemMetrics.tcpEstablished'),
      type: 'line',
      smooth: true,
      showSymbol: false,
      data: chartDerived.value.tcpEstablished,
      lineStyle: { width: 2, color: chartColors.value.success },
      itemStyle: { color: chartColors.value.success },
    },
    {
      name: t('systemMetrics.tcpTimeWait'),
      type: 'line',
      smooth: true,
      showSymbol: false,
      data: chartDerived.value.tcpTimeWait,
      lineStyle: { width: 2, color: chartColors.value.warning },
      itemStyle: { color: chartColors.value.warning },
    },
    {
      name: t('systemMetrics.processCount'),
      type: 'line',
      smooth: true,
      showSymbol: false,
      data: chartDerived.value.processCount,
      lineStyle: { width: 2, color: chartColors.value.primary },
      itemStyle: { color: chartColors.value.primary },
    },
  ],
}))

const appendRealtimePoint = (point: SystemMetricsPoint) => {
  realtimePoints.value.push(point)
  const keepSince = point.timestamp - maxWindowSeconds.value
  const firstValidIdx = realtimePoints.value.findIndex((p) => p.timestamp >= keepSince)
  if (firstValidIdx > 0) {
    realtimePoints.value.splice(0, firstValidIdx)
  }
  latestPoint.value = point
}

const handleRealtimePayload = (res: SystemMetricsRealtimePayload) => {
  sampleIntervalSeconds.value = Number(res.sample_interval_seconds) || 5
  maxWindowSeconds.value = Number(res.max_window_seconds) || 7 * 24 * 3600

  if (!res.supported) {
    unsupported.value = true
    unsupportedMessage.value = res.message || 'not supported'
    return
  }

  unsupported.value = false
  unsupportedMessage.value = ''
  errorMessage.value = ''

  realtimePoints.value = Array.isArray(res.points) ? res.points : []
  latestPoint.value = res.latest || (realtimePoints.value.length ? realtimePoints.value[realtimePoints.value.length - 1] : null)
  latestInterfaces.value = Array.isArray(res.interfaces) ? res.interfaces : []
}

const loadRealtimeSnapshot = async () => {
  loadingRealtime.value = true
  try {
    const res = await GetSystemMetrics(selectedWindow.value) as SystemMetricsRealtimePayload
    handleRealtimePayload(res)
  } catch (e: any) {
    errorMessage.value = e?.message || String(e)
  } finally {
    loadingRealtime.value = false
  }
}

const getSuggestedGranularity = (spanSec: number) => {
  if (spanSec <= 6 * 3600) return sampleIntervalSeconds.value
  if (spanSec <= 3 * 24 * 3600) return 60
  if (spanSec <= 14 * 24 * 3600) return 300
  return 900
}

const loadHistoricalData = async () => {
  if (!dateRange.value || dateRange.value.length !== 2) {
    ElMessage.warning(t('systemMetrics.selectDateRange'))
    return
  }

  const [startMs, endMs] = dateRange.value
  if (startMs >= endMs) {
    ElMessage.warning(t('systemMetrics.startTimeMustBeLess'))
    return
  }

  const startSec = Math.floor(startMs / 1000)
  const endSec = Math.floor(endMs / 1000)
  const spanSec = endSec - startSec

  loadingHistorical.value = true
  try {
    const res = await QueryHistoricalSystemMetrics({
      start_time: startSec,
      end_time: endSec,
      granularity_secs: getSuggestedGranularity(spanSec),
    }) as QuerySystemMetricsResponse

    if (!res.supported) {
      unsupported.value = true
      unsupportedMessage.value = res.message || 'not supported'
      return
    }

    if (res.message) {
      ElMessage.warning(res.message)
    }

    historicalPoints.value = Array.isArray(res.points) ? res.points : []
    errorMessage.value = ''
    if (historicalPoints.value.length > 0) {
      ElMessage.success(t('systemMetrics.loadHistoricalSuccess', { count: historicalPoints.value.length }))
    } else {
      ElMessage.warning(t('systemMetrics.noHistoricalData'))
    }
  } catch (e: any) {
    errorMessage.value = e?.message || String(e)
    ElMessage.error(t('systemMetrics.loadHistoricalFailed', { error: errorMessage.value }))
  } finally {
    loadingHistorical.value = false
  }
}

const clearHistoricalData = () => {
  historicalPoints.value = null
  ElMessage.info(t('systemMetrics.historicalDataCleared'))
}

const onSystemMetricsEvent = (payload: SystemMetricsEventPayload) => {
  const point = payload?.point
  if (point) {
    appendRealtimePoint(point)
  }
  if (Array.isArray(payload?.interfaces)) {
    latestInterfaces.value = payload.interfaces
  }
}

const startSubscription = async () => {
  if (metricsUnlisten) {
    void SetSystemMetricsSubscription(true).catch(() => {})
    return
  }

  try {
    const unlisten = await EventsOn('system-metrics', onSystemMetricsEvent)
    metricsUnlisten = unlisten
    void SetSystemMetricsSubscription(true).catch(() => {})
  } catch (e: any) {
    errorMessage.value = e?.message || String(e)
  }
}

const stopSubscription = () => {
  void SetSystemMetricsSubscription(false).catch(() => {})
  if (!metricsUnlisten) return
  try {
    EventsOff(metricsUnlisten)
  } catch {
    // ignore
  }
  metricsUnlisten = null
}

watch(selectedWindow, () => {
  if (props.isActive && !isHistoricalMode.value) {
    loadRealtimeSnapshot()
  }
})

watch(
  () => props.config,
  (cfg: any) => {
    const rawInterval = Number(cfg?.system_metrics_sample_interval_secs)
    const normalizedInterval = Number.isFinite(rawInterval) && rawInterval > 0
      ? Math.max(1, Math.floor(rawInterval))
      : 10
    if (Number.isFinite(rawInterval) && rawInterval > 0) {
      configSampleIntervalSecs.value = normalizedInterval
    } else {
      configSampleIntervalSecs.value = 10
    }
    // 同步刷新 updateHint 中的采样秒数，避免必须重启才更新
    sampleIntervalSeconds.value = normalizedInterval
    if (typeof cfg?.system_metrics_persistence_enabled === 'boolean') {
      configPersistenceEnabled.value = cfg.system_metrics_persistence_enabled
    } else {
      configPersistenceEnabled.value = true
    }
  },
  { immediate: true, deep: true },
)

const getConfig = () => {
  const raw = Number(configSampleIntervalSecs.value)
  const interval = Number.isFinite(raw) ? Math.max(1, Math.floor(raw)) : 10
  return {
    system_metrics_sample_interval_secs: interval,
    system_metrics_persistence_enabled: !!configPersistenceEnabled.value,
  }
}

defineExpose({
  getConfig,
})

watch(
  () => props.isActive,
  async (active) => {
    if (active) {
      await loadRealtimeSnapshot()
      await startSubscription()
    } else {
      stopSubscription()
    }
  },
  { immediate: true },
)

onMounted(() => {
  updateChartColors()
  themeObserver = new MutationObserver(() => {
    updateChartColors()
  })
  themeObserver.observe(document.documentElement, { attributes: true, attributeFilter: ['class'] })
})

onBeforeUnmount(() => {
  stopSubscription()
  if (themeObserver) {
    themeObserver.disconnect()
    themeObserver = null
  }
})
</script>

<style scoped>
.system-metrics {
  height: 100%;
  overflow-y: auto;
  overflow-x: hidden;
  box-sizing: border-box;
  padding: 6px;
}

.config-card {
  background: transparent;
  border-radius: 0;
  box-shadow: none;
  border: none;
}

.main-panel :deep(.el-card__body) {
  display: flex;
  flex-direction: column;
  gap: 8px;
  padding: 10px 12px;
}

.main-panel :deep(.el-card__header) {
  padding: 10px 12px;
}

.header {
  display: flex;
  justify-content: space-between;
  align-items: flex-start;
  gap: 8px;
  flex-wrap: wrap;
}

.header h3 {
  margin: 0 0 2px 0;
  font-size: 18px;
  color: var(--text);
}

.title-area {
  display: flex;
  flex-direction: column;
  gap: 4px;
}

.title-inline-controls {
  display: flex;
  align-items: center;
  gap: 8px;
  flex-wrap: wrap;
}

.header-actions {
  display: flex;
  align-items: center;
  gap: 6px;
  flex-wrap: wrap;
}

.title-inline-controls :deep(.el-form-item__label),
.header-actions :deep(.el-form-item__label) {
  padding-right: 6px;
  font-size: 12px;
}

.stats-grid {
  display: grid;
  grid-template-columns: repeat(4, minmax(0, 1fr));
  gap: 6px;
}

.stat-card {
  border: 1px solid var(--border);
  background: var(--card-bg);
}

.stat-card :deep(.el-card__body) {
  padding: 8px 10px;
}

.stat-label {
  font-size: 11px;
  line-height: 1.2;
  color: var(--text-muted);
}

.stat-value {
  margin-top: 2px;
  font-size: 16px;
  line-height: 1.2;
  font-weight: 700;
  color: var(--text);
}

.meta {
  margin-top: 2px;
}

.charts-grid {
  display: grid;
  grid-template-columns: 1fr;
  gap: 6px;
}

.chart-panel,
.table-panel {
  border: 1px solid var(--border);
  background: var(--card-bg);
}

.chart-panel :deep(.el-card__body),
.table-panel :deep(.el-card__body) {
  padding: 8px 10px;
}

.panel-title {
  font-weight: 600;
}

.chart {
  width: 100%;
  height: 230px;
}

@media (max-width: 1300px) {
  .stats-grid {
    grid-template-columns: repeat(3, minmax(0, 1fr));
  }
}

@media (max-width: 1100px) {
  .stats-grid {
    grid-template-columns: repeat(2, minmax(0, 1fr));
  }
}

@media (max-width: 700px) {
  .system-metrics {
    padding: 4px;
  }

  .stats-grid {
    grid-template-columns: 1fr;
  }

  .chart {
    height: 210px;
  }
}
</style>
