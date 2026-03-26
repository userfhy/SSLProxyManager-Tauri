<template>
  <div class="config-card config-page">
    <div class="header">
        <div>
          <h3>{{ $t('dashboard.title') }}</h3>
          <el-text type="info" size="small" class="hint">{{ $t('dashboard.updateHint') }}</el-text>
        </div>

        <div class="controls">
          <el-form-item :label="$t('dashboard.listenAddr')" style="margin-bottom: 0;">
            <el-select v-model="selectedListen" style="width: 200px;">
              <el-option v-for="a in listenAddrs" :key="a" :label="getListenAddrLabel(a)" :value="a" />
            </el-select>
          </el-form-item>

          <el-form-item :label="$t('dashboard.displayPeriod')" style="margin-bottom: 0;">
            <el-select v-model.number="selectedWindow" style="width: 150px;">
              <el-option :label="$t('dashboard.oneMinute')" :value="60" />
              <el-option :label="$t('dashboard.fifteenMinutes')" :value="900" />
              <el-option :label="$t('dashboard.thirtyMinutes')" :value="1800" />
              <el-option :label="$t('dashboard.oneHour')" :value="3600" />
              <el-option :label="$t('dashboard.threeHours')" :value="10800" />
              <el-option :label="$t('dashboard.sixHours')" :value="21600" />
              <el-option :label="$t('dashboard.twelveHours')" :value="43200" />
              <el-option :label="$t('dashboard.twentyFourHours')" :value="86400" />
            </el-select>
          </el-form-item>

          <el-form-item :label="$t('dashboard.historicalData')" style="margin-bottom: 0;">
            <el-config-provider :locale="datePickerLocale">
              <el-date-picker
                v-model="dateRange"
                type="datetimerange"
                :range-separator="$t('dashboard.to')"
                :start-placeholder="$t('dashboard.startTime')"
                :end-placeholder="$t('dashboard.endTime')"
                format="YYYY-MM-DD HH:mm:ss"
                value-format="x"
                :shortcuts="dateShortcuts"
                style="width: 380px;"
              />
            </el-config-provider>
          </el-form-item>

          <el-form-item style="margin-bottom: 0;">
            <el-button type="primary" @click="loadHistoricalData" :loading="loadingHistorical">
              {{ $t('dashboard.loadHistorical') }}
            </el-button>
            <el-button v-if="historicalData" @click="clearHistoricalData">
              {{ $t('dashboard.showRealtime') }}
            </el-button>
          </el-form-item>
        </div>
      </div>

    <div class="grid">
      <el-card class="panel panel--stats" shadow="hover">
        <div class="stats">
          <div class="stat">
            <div class="stat-label">{{ $t('dashboard.totalRequests') }}</div>
            <div class="stat-value">{{ totalReq }}</div>
          </div>
          <div class="stat">
            <div class="stat-label">{{ $t('dashboard.successRate') }}</div>
            <div class="stat-value">{{ successRate }}</div>
          </div>
          <div class="stat">
            <div class="stat-label">{{ $t('dashboard.errorRate') }}</div>
            <div class="stat-value">{{ errorRate }}</div>
          </div>
          <div class="stat">
            <div class="stat-label">{{ $t('dashboard.avgLatency') }}</div>
            <div class="stat-value">{{ avgLatency }}</div>
          </div>
        </div>

        <div v-if="phaseTiming" class="phase-stats">
          <div class="phase-title">{{ $t('dashboard.phaseTiming') }}</div>
          <div class="phase-grid">
            <div class="phase-item">
              <div class="phase-name">{{ $t('dashboard.phaseGuard') }}</div>
              <div class="phase-line">avg {{ formatPhaseMs(phaseTiming?.guard?.avg_ms) }} ms</div>
              <div class="phase-line">p95 {{ formatPhaseMs(phaseTiming?.guard?.p95_ms) }} ms</div>
              <div class="phase-line">p99 {{ formatPhaseMs(phaseTiming?.guard?.p99_ms) }} ms</div>
            </div>
            <div class="phase-item">
              <div class="phase-name">{{ $t('dashboard.phasePrepare') }}</div>
              <div class="phase-line">avg {{ formatPhaseMs(phaseTiming?.prepare?.avg_ms) }} ms</div>
              <div class="phase-line">p95 {{ formatPhaseMs(phaseTiming?.prepare?.p95_ms) }} ms</div>
              <div class="phase-line">p99 {{ formatPhaseMs(phaseTiming?.prepare?.p99_ms) }} ms</div>
            </div>
            <div class="phase-item">
              <div class="phase-name">{{ $t('dashboard.phaseUpstream') }}</div>
              <div class="phase-line">avg {{ formatPhaseMs(phaseTiming?.upstream?.avg_ms) }} ms</div>
              <div class="phase-line">p95 {{ formatPhaseMs(phaseTiming?.upstream?.p95_ms) }} ms</div>
              <div class="phase-line">p99 {{ formatPhaseMs(phaseTiming?.upstream?.p99_ms) }} ms</div>
            </div>
          </div>
        </div>
      </el-card>

      <el-card class="panel panel--qps" shadow="hover">
        <template #header>
          <div class="panel-header">
            <div class="panel-title">{{ $t('dashboard.qpsTrend') }}</div>
            <el-button text size="small" class="panel-preview-btn" @click="openChartPreview($t('dashboard.qpsTrend'), 'qps')">
              {{ $t('common.fullscreenPreview') }}
            </el-button>
          </div>
        </template>
        <v-chart v-if="isActive" :option="qpsOption" class="chart" autoresize />
      </el-card>

      <el-card class="panel panel--status" shadow="hover">
        <template #header>
          <div class="panel-header">
            <div class="panel-title">{{ $t('dashboard.statusDistribution') }}</div>
            <el-button text size="small" class="panel-preview-btn" @click="openChartPreview($t('dashboard.statusDistribution'), 'status')">
              {{ $t('common.fullscreenPreview') }}
            </el-button>
          </div>
        </template>
        <v-chart v-if="isActive" :option="statusOption" class="chart" autoresize />
      </el-card>

      <el-card class="panel panel--latency" shadow="hover">
        <template #header>
          <div class="panel-header">
            <div class="panel-title">{{ $t('dashboard.latencyTrend') }}</div>
            <el-button text size="small" class="panel-preview-btn" @click="openChartPreview($t('dashboard.latencyTrend'), 'latency')">
              {{ $t('common.fullscreenPreview') }}
            </el-button>
          </div>
        </template>
        <v-chart v-if="isActive" :option="latencyOption" class="chart" autoresize />
      </el-card>

      <el-card class="panel panel--percentile" shadow="hover">
        <template #header>
          <div class="panel-header">
            <div class="panel-title">{{ $t('dashboard.percentile') }}</div>
            <el-button text size="small" class="panel-preview-btn" @click="openChartPreview($t('dashboard.percentile'), 'percentile')">
              {{ $t('common.fullscreenPreview') }}
            </el-button>
          </div>
        </template>
        <v-chart v-if="isActive" :option="pOption" class="chart" autoresize />
      </el-card>

      <el-card class="panel panel--upstream" shadow="hover">
        <template #header>
          <div class="panel-header">
            <div class="panel-title">{{ $t('dashboard.upstreamRequestDistributionTop20') }}</div>
            <el-button text size="small" class="panel-preview-btn" @click="openChartPreview($t('dashboard.upstreamRequestDistributionTop20'), 'upstream')">
              {{ $t('common.fullscreenPreview') }}
            </el-button>
          </div>
        </template>
        <v-chart v-if="isActive" :option="upDistOption" class="chart" autoresize />
      </el-card>

      <el-card class="panel panel--errors" shadow="hover">
        <template #header>
          <div class="panel-title">{{ $t('dashboard.topsRouteUpstream') }}</div>
        </template>
        <div class="tables">

          <el-card class="table" shadow="never">
            <template #header>
              <div class="table-title">{{ $t('dashboard.topClientIps') }}</div>
            </template>
            <div class="rows">
              <div v-for="(it, idx) in topClientIps" :key="idx" class="row">
                <div class="k">{{ it.item }}</div>
                <div class="v">{{ it.count }}</div>
              </div>
              <el-empty v-if="topClientIps.length===0" :description="$t('dashboard.noData')" :image-size="60" />
            </div>
          </el-card>

          <el-card class="table" shadow="never">
            <template #header>
              <div class="table-title">{{ $t('dashboard.topPaths') }}</div>
            </template>
            <div class="rows">
              <div v-for="(it, idx) in topPaths" :key="idx" class="row">
                <div class="k">{{ it.item }}</div>
                <div class="v">{{ it.count }}</div>
              </div>
              <el-empty v-if="topPaths.length===0" :description="$t('dashboard.noData')" :image-size="60" />
            </div>
          </el-card>

          <el-card class="table" shadow="never">
            <template #header>
              <div class="table-title">{{ $t('dashboard.topRouteErrors') }}</div>
            </template>
            <div class="rows">
              <div v-for="(it, idx) in topRouteErr" :key="idx" class="row">
                <div class="k">{{ it.key }}</div>
                <div class="v">{{ it.value }}</div>
              </div>
              <el-empty v-if="topRouteErr.length===0" :description="$t('dashboard.noData')" :image-size="60" />
            </div>
          </el-card>

          <el-card class="table" shadow="never">
            <template #header>
              <div class="table-title">{{ $t('dashboard.topUpstreamErrors') }}</div>
            </template>
            <div class="rows">
              <div v-for="(it, idx) in topUpstreamErrors" :key="idx" class="row">
                <div class="k">{{ it.item }}</div>
                <div class="v">{{ it.count }}</div>
              </div>
              <el-empty v-if="topUpstreamErrors.length===0" :description="$t('dashboard.noData')" :image-size="60" />
            </div>
          </el-card>
        </div>
      </el-card>

      <el-card class="panel panel--rate" shadow="hover">
        <template #header>
          <div class="panel-header">
            <div class="panel-title">{{ $t('dashboard.errorSuccessRateTrend') }}</div>
            <el-button text size="small" class="panel-preview-btn" @click="openChartPreview($t('dashboard.errorSuccessRateTrend'), 'rate')">
              {{ $t('common.fullscreenPreview') }}
            </el-button>
          </div>
        </template>
        <v-chart v-if="isActive" :option="rateOption" class="chart" autoresize />
      </el-card>

      <el-card class="panel panel--pie" shadow="hover">
        <template #header>
          <div class="panel-header">
            <div class="panel-title">{{ $t('dashboard.statusDistributionPie') }}</div>
            <el-button text size="small" class="panel-preview-btn" @click="openChartPreview($t('dashboard.statusDistributionPie'), 'statusPie')">
              {{ $t('common.fullscreenPreview') }}
            </el-button>
          </div>
        </template>
        <v-chart v-if="isActive" :option="statusPieOption" class="chart" autoresize />
      </el-card>

      <el-card class="panel panel--throughput" shadow="hover">
        <template #header>
          <div class="panel-header">
            <div class="panel-title">{{ $t('dashboard.throughputTrendCumulative') }}</div>
            <el-button text size="small" class="panel-preview-btn" @click="openChartPreview($t('dashboard.throughputTrendCumulative'), 'throughput')">
              {{ $t('common.fullscreenPreview') }}
            </el-button>
          </div>
        </template>
        <v-chart v-if="isActive" :option="throughputOption" class="chart" autoresize />
      </el-card>

      <el-card class="panel panel--latency-dist" shadow="hover">
        <template #header>
          <div class="panel-header">
            <div class="panel-title">{{ $t('dashboard.latencyDistributionComparison') }}</div>
            <el-button text size="small" class="panel-preview-btn" @click="openChartPreview($t('dashboard.latencyDistributionComparison'), 'latencyDist')">
              {{ $t('common.fullscreenPreview') }}
            </el-button>
          </div>
        </template>
        <v-chart v-if="isActive" :option="latencyDistOption" class="chart" autoresize />
      </el-card>

      <el-card class="panel panel--top-routes" shadow="hover">
        <template #header>
          <div class="panel-header">
            <div class="panel-header-left">
              <div class="panel-title">{{ $t('dashboard.topRouteRequestDistribution') }}</div>
              <el-tooltip :content="$t('dashboard.topRouteHint')" placement="top">
                <el-icon class="header-icon"><InfoFilled /></el-icon>
              </el-tooltip>
            </div>
            <el-button text size="small" class="panel-preview-btn" @click="openChartPreview($t('dashboard.topRouteRequestDistribution'), 'topRoutes')">
              {{ $t('common.fullscreenPreview') }}
            </el-button>
          </div>
        </template>
        <div v-if="!isActive" class="chart-placeholder">
          <el-skeleton :rows="5" animated />
        </div>
        <template v-else>
          <div v-if="topRoutes.length > 0" class="top-routes-chart">
            <v-chart :option="topRoutesOption" class="chart" autoresize />
          </div>
          <el-empty
            v-else
            :description="historicalData ? $t('dashboard.noRouteData') : $t('dashboard.realtimeNoDbHint')"
            :image-size="60"
          />
        </template>
      </el-card>
    </div>

  </div>
</template>

<script setup lang="ts">
import { computed, onBeforeUnmount, onMounted, ref, watch } from 'vue'
import { ElMessage, ElConfigProvider } from 'element-plus'
import { InfoFilled } from '@element-plus/icons-vue'
import zhCn from 'element-plus/dist/locale/zh-cn.mjs'
import enUs from 'element-plus/dist/locale/en.mjs'
import { EventsOn, EventsOff } from '../api'
import { GetListenAddrs, GetMetrics, QueryHistoricalMetrics, GetDashboardStats, GetConfig, OpenChartPreviewWindow } from '../api'
import type { EChartsOption } from 'echarts'
import { createLazyVChart } from '../composables/lazyEcharts'
import { useI18n } from 'vue-i18n'
import { useDateShortcuts } from '../composables/useDateShortcuts'
import { emitTo, listen } from '@tauri-apps/api/event'

const { t, locale } = useI18n()
const { dateShortcuts } = useDateShortcuts()
const datePickerLocale = computed(() => (locale.value === 'en-US' ? enUs : zhCn))

const VChart = createLazyVChart('dashboard-echarts', ({ use, renderers, charts, components }) => {
  use([
    renderers.CanvasRenderer,
    charts.LineChart,
    charts.BarChart,
    charts.PieChart,
    components.GridComponent,
    components.TooltipComponent,
    components.LegendComponent,
    components.GraphicComponent,
    components.DataZoomComponent,
  ])
})

const props = defineProps<{ isActive: boolean }>()
const windowVisible = ref<boolean>(typeof document === 'undefined' ? true : !document.hidden)
const previewSyncActive = ref(false)
const PREVIEW_SYNC_KEEPALIVE_MS = 120000

type KV = { key: string; value: number }

// 临时类型定义
type MetricsSeries = {
  timestamps: number[]
  counts: number[]
  s2xx: number[]
  s3xx: number[]
  s4xx: number[]
  s5xx: number[]
  s0: number[]
  avgLatencyMs: number[]
  maxLatencyMs: number[]

  p50?: number[]
  p95?: number[]
  p99?: number[]

  upstreamDist?: KV[]
  topRouteErr?: KV[]
  topUpErr?: KV[]
  latencyDist?: KV[]
}

type PhaseMetricStats = {
  avg_ms?: number
  p95_ms?: number
  p99_ms?: number
}

type PhaseTimingStats = {
  guard?: PhaseMetricStats
  prepare?: PhaseMetricStats
  upstream?: PhaseMetricStats
}

type DashboardStatsResponse = {
  top_routes?: Array<{ item: string; count: number }>
  top_route_errors?: Array<{ item: string; count: number }>
  top_paths?: Array<{ item: string; count: number }>
  top_ips?: Array<{ item: string; count: number }>
  top_upstream_errors?: Array<{ item: string; count: number }>
  phase_timing?: PhaseTimingStats
}

type MetricsPayload = {
  windowSeconds: number
  listenAddrs: string[]
  byListenAddr: Record<string, MetricsSeries>

  minuteWindowSeconds?: number
  byListenMinute?: Record<string, MetricsSeries>

  topRoutes?: Array<{ item: string; count: number }>
  topPaths?: Array<{ item: string; count: number }>
  topClientIps?: Array<{ item: string; count: number }>
  topUpstreamErrors?: Array<{ item: string; count: number }>
}

const GLOBAL_LISTEN_ADDR = '全局'
const isGlobalListen = (listenAddr: string) => listenAddr === GLOBAL_LISTEN_ADDR
const getListenAddrLabel = (listenAddr: string) => (isGlobalListen(listenAddr) ? t('dashboard.globalListen') : listenAddr)

const listenAddrs = ref<string[]>([GLOBAL_LISTEN_ADDR])
const selectedListen = ref<string>(GLOBAL_LISTEN_ADDR)
const selectedWindow = ref<number>(900)

const latest = ref<MetricsPayload | null>(null)

// 历史数据相关
const dateRange = ref<[number, number] | null>(null)
const loadingHistorical = ref(false)
const historicalData = ref<MetricsSeries | null>(null)
const previewTitle = ref('')
const previewChartKey = ref<
  | 'qps'
  | 'status'
  | 'latency'
  | 'percentile'
  | 'upstream'
  | 'rate'
  | 'statusPie'
  | 'throughput'
  | 'latencyDist'
  | 'topRoutes'
  | ''
>('')

const openChartPreview = (title: string, key: typeof previewChartKey.value) => {
  previewTitle.value = title
  previewChartKey.value = key
  void openPreviewInNewWindow()
}

const getPreviewOptionByKey = (key: typeof previewChartKey.value): EChartsOption => {
  switch (key) {
    case 'qps': return qpsOption.value
    case 'status': return statusOption.value
    case 'latency': return latencyOption.value
    case 'percentile': return pOption.value
    case 'upstream': return upDistOption.value
    case 'rate': return rateOption.value
    case 'statusPie': return statusPieOption.value
    case 'throughput': return throughputOption.value
    case 'latencyDist': return latencyDistOption.value
    case 'topRoutes': return topRoutesOption.value
    default: return {}
  }
}

const applyPreviewZoom = (option: EChartsOption): EChartsOption => {
  const base = option || {}
  const xAxis = (base as any).xAxis
  if (xAxis === undefined) return base

  return {
    ...(base as any),
    dataZoom: [
      {
        type: 'inside',
        xAxisIndex: 0,
        start: 0,
        end: 100,
        zoomOnMouseWheel: true,
        moveOnMouseMove: true,
      },
      {
        type: 'slider',
        xAxisIndex: 0,
        height: 18,
        bottom: 6,
        start: 0,
        end: 100,
      },
    ],
  }
}

const formatAxisDateTime = (tsSec: number) => {
  const d = new Date(tsSec * 1000)
  const yyyy = d.getFullYear()
  const mm = String(d.getMonth() + 1).padStart(2, '0')
  const dd = String(d.getDate()).padStart(2, '0')
  const hh = String(d.getHours()).padStart(2, '0')
  const mi = String(d.getMinutes()).padStart(2, '0')
  const ss = String(d.getSeconds()).padStart(2, '0')
  return `${yyyy}-${mm}-${dd} ${hh}:${mi}:${ss}`
}

const isPreviewTimeSeriesChart = (key: typeof previewChartKey.value) =>
  key === 'qps'
  || key === 'status'
  || key === 'latency'
  || key === 'percentile'
  || key === 'rate'
  || key === 'throughput'

const applyPreviewDateLabels = (option: EChartsOption, labels: string[]): EChartsOption => {
  if (!labels.length) return option
  const base = (option || {}) as any
  const xAxis = base.xAxis
  if (!xAxis) return base

  const patchAxis = (axis: any) => {
    if (!axis || typeof axis !== 'object') return axis
    if (axis.type && axis.type !== 'category') return axis
    if (Array.isArray(axis.data) && axis.data.length > 0 && axis.data.length !== labels.length) {
      return axis
    }
    return {
      ...axis,
      data: labels,
    }
  }

  if (Array.isArray(xAxis)) {
    return {
      ...base,
      xAxis: xAxis.map((axis: any, idx: number) => (idx === 0 ? patchAxis(axis) : axis)),
    }
  }

  return {
    ...base,
    xAxis: patchAxis(xAxis),
  }
}

const getPreviewOptionPayloadByKey = (key: typeof previewChartKey.value): any | null => {
  const previewDateLabels = isPreviewTimeSeriesChart(key)
    ? ((alignedView.value?.series?.timestamps || []).map((ts) => formatAxisDateTime(ts)))
    : []
  const option = applyPreviewDateLabels(
    applyPreviewZoom(getPreviewOptionByKey(key)),
    previewDateLabels,
  ) as any
  if (!option || typeof option !== 'object') {
    return null
  }
  // 通过 JSON 深拷贝去掉函数，保证可序列化到 localStorage
  try {
    return JSON.parse(JSON.stringify(option))
  } catch {
    return null
  }
}

const openPreviewInNewWindow = async () => {
  const chartKey = previewChartKey.value
  const optionPayload = getPreviewOptionPayloadByKey(chartKey)
  if (!optionPayload) {
    ElMessage.warning(t('common.previewUnavailable'))
    return
  }
  const title = previewTitle.value
  const payloadKey = `chart-preview:${Date.now()}:${Math.random().toString(36).slice(2)}`
  localStorage.setItem(
    payloadKey,
    JSON.stringify({
      title,
      option: optionPayload,
      source: 'dashboard',
      chartKey,
      createdAt: Date.now(),
    }),
  )
  const previewUrl = new URL('/index.html', window.location.origin)
  previewUrl.searchParams.set('chart_preview', '1')
  previewUrl.searchParams.set('key', payloadKey)

  try {
    if ((window as any).__TAURI_INTERNALS__) {
      await OpenChartPreviewWindow(title, payloadKey, `dashboard-${chartKey}`)
      return
    }
  } catch (e) {
    console.error('open preview window failed:', e)
    ElMessage.error(t('common.previewUnavailable'))
    return
  }

  const popupName = `chart-preview-dashboard-${chartKey}`
  const popup = window.open(previewUrl.toString(), popupName, 'width=1400,height=900')
  if (!popup) {
    ElMessage.warning(t('common.previewUnavailable'))
    return
  }
  popup.focus()
}

let previewSyncUnlisten: (() => void) | null = null
let previewSyncIdleTimer: number | null = null

const onPreviewSyncRequest = async (event: any) => {
  const payload = event?.payload as any
  if (!payload || payload.source !== 'dashboard') return

  previewSyncActive.value = true
  if (previewSyncIdleTimer) {
    clearTimeout(previewSyncIdleTimer)
  }
  previewSyncIdleTimer = window.setTimeout(() => {
    previewSyncActive.value = false
    previewSyncIdleTimer = null
  }, PREVIEW_SYNC_KEEPALIVE_MS)

  const requestId = String(payload.requestId || '').trim()
  const chartKey = String(payload.chartKey || '').trim() as typeof previewChartKey.value
  const responseEvent = 'chart-preview-sync-response'
  const target = String(payload.requesterLabel || '').trim()
  if (!requestId) return

  const option = getPreviewOptionPayloadByKey(chartKey)
  if (!option) {
    if (target) {
      await emitTo(target, responseEvent, { requestId, ok: false, error: 'preview option unavailable' })
    }
    return
  }
  if (target) {
    await emitTo(target, responseEvent, {
      requestId,
      ok: true,
      option,
      title: String(payload.title || previewTitle.value || ''),
      updatedAt: Date.now(),
    })
  }
}

const maxPoints = 1200

const formatTime = (tsSec: number) => {
  const d = new Date(tsSec * 1000)
  const hh = String(d.getHours()).padStart(2, '0')
  const mm = String(d.getMinutes()).padStart(2, '0')
  const ss = String(d.getSeconds()).padStart(2, '0')
  return `${hh}:${mm}:${ss}`
}

const sliceTail = <T,>(arr: T[], n: number): T[] => {
  if (!Array.isArray(arr)) return []
  if (n <= 0) return []
  return arr.length <= n ? arr : arr.slice(arr.length - n)
}

const getRawWindowSeries = () => {
  const p = latest.value
  if (!p) return null

  // 1小时及以上（>= 3600秒）使用分钟级数据
  const useMinute = selectedWindow.value >= 3600
  const dict = useMinute ? (p.byListenMinute || {}) : p.byListenAddr
  const maxWin = useMinute ? (p.minuteWindowSeconds || p.windowSeconds || 0) : (p.windowSeconds || 0)

  const all = dict?.[selectedListen.value]
  if (!all) return null

  const winSec = maxWin > 0 ? Math.min(selectedWindow.value, maxWin) : selectedWindow.value
  const pointWin = useMinute ? Math.ceil(winSec / 60) : winSec

  const tsTail = sliceTail(all.timestamps || [], pointWin)
  const len = tsTail.length
  if (len === 0) {
    return {
      timestamps: [] as number[],
      counts: [] as number[],
      s2xx: [] as number[],
      s3xx: [] as number[],
      s4xx: [] as number[],
      s5xx: [] as number[],
      s0: [] as number[],
      avgLatencyMs: [] as number[],
      upstreamDist: all.upstreamDist || [],
      topRouteErr: all.topRouteErr || [],
      topUpErr: all.topUpErr || [],
    }
  }

  const cut = <T,>(arr: T[] | undefined, fallback: T) => {
    const tail = sliceTail(arr || [], pointWin)
    if (tail.length === len) return tail
    const padded = new Array(len).fill(fallback) as T[]
    const startAt = padded.length - tail.length
    for (let i = 0; i < tail.length; i++) padded[startAt + i] = tail[i]
    return padded
  }

  return {
    timestamps: tsTail,
    counts: cut(all.counts, 0),
    s2xx: cut(all.s2xx, 0),
    s3xx: cut(all.s3xx, 0),
    s4xx: cut(all.s4xx, 0),
    s5xx: cut(all.s5xx, 0),
    s0: cut(all.s0, 0),
    avgLatencyMs: cut(all.avgLatencyMs, 0),
    upstreamDist: all.upstreamDist || [],
    topRouteErr: all.topRouteErr || [],
    topUpErr: all.topUpErr || [],
  }
}

// 保存上一次有效的数据视图，避免切换窗口时闪烁
// 同时保存窗口大小和监听地址，只有当这些匹配时才使用缓存
let lastValidView: { x: string[]; series: MetricsSeries; window: number; listen: string } | null = null

const buildAlignedView = () => {
  const p = latest.value
  if (!p) {
    // 只有当窗口大小和监听地址匹配时才返回缓存视图
    if (lastValidView && lastValidView.window === selectedWindow.value && lastValidView.listen === selectedListen.value) {
      return lastValidView
    }
    return null
  }

  // 1小时及以上（>= 3600秒）使用分钟级数据
  const useMinute = selectedWindow.value >= 3600
  const dict = useMinute ? (p.byListenMinute || {}) : p.byListenAddr
  const maxWin = useMinute ? (p.minuteWindowSeconds || p.windowSeconds || 0) : (p.windowSeconds || 0)

  const all = dict?.[selectedListen.value]
  if (!all) {
    // 只有当窗口大小和监听地址匹配时才返回缓存视图
    if (lastValidView && lastValidView.window === selectedWindow.value && lastValidView.listen === selectedListen.value) {
      return lastValidView
    }
    return null
  }

  const winSec = maxWin > 0 ? Math.min(selectedWindow.value, maxWin) : selectedWindow.value
  const pointWin = useMinute ? Math.ceil(winSec / 60) : winSec

  const tsTail = sliceTail(all.timestamps || [], pointWin)
  if (tsTail.length === 0) {
    // 只有当窗口大小和监听地址匹配时才返回缓存视图
    if (lastValidView && lastValidView.window === selectedWindow.value && lastValidView.listen === selectedListen.value) {
      return lastValidView
    }
    return null
  }

  const step = tsTail.length > maxPoints ? Math.ceil(tsTail.length / maxPoints) : 1
  const idx: number[] = []
  for (let i = 0; i < tsTail.length; i += step) idx.push(i)

  const pick = <T,>(arr: T[] | undefined, fallback: T): T[] => {
    const tail = sliceTail(arr || [], pointWin)
    if (tail.length !== tsTail.length) {
      const padded = new Array(tsTail.length).fill(fallback) as T[]
      const startAt = padded.length - tail.length
      for (let i = 0; i < tail.length; i++) padded[startAt + i] = tail[i]
      return idx.map(i => padded[i])
    }
    return idx.map(i => tail[i])
  }

  const x = idx.map(i => formatTime(tsTail[i]))

  const view: MetricsSeries = {
    timestamps: idx.map(i => tsTail[i]),
    counts: pick(all.counts, 0),
    s2xx: pick(all.s2xx, 0),
    s3xx: pick(all.s3xx, 0),
    s4xx: pick(all.s4xx, 0),
    s5xx: pick(all.s5xx, 0),
    s0: pick(all.s0, 0),
    avgLatencyMs: pick(all.avgLatencyMs, 0),
    maxLatencyMs: pick(all.maxLatencyMs, 0),
    p50: pick(all.p50, 0),
    p95: pick(all.p95, 0),
    p99: pick(all.p99, 0),

    upstreamDist: all.upstreamDist || [],
    topRouteErr: all.topRouteErr || [],
    topUpErr: all.topUpErr || [],
  }

  const result = { 
    x, 
    series: view,
    window: selectedWindow.value,
    listen: selectedListen.value
  }
  lastValidView = result // 保存有效视图（包含窗口大小和监听地址）
  return result
}

const sum = (arr: number[]) => arr.reduce((a, b) => a + (Number.isFinite(b) ? b : 0), 0)

const totalReq = computed(() => {
  // 如果有历史数据，使用历史数据
  if (historicalData.value) {
    return sum(historicalData.value.counts || [])
  }
  const raw = getRawWindowSeries()
  if (!raw) return 0
  return sum(raw.counts)
})

const successRate = computed(() => {
  // 如果有历史数据，使用历史数据
  if (historicalData.value) {
    const t = sum(historicalData.value.counts || [])
    if (t <= 0) return '0%'
    return `${((sum(historicalData.value.s2xx || []) / t) * 100).toFixed(2)}%`
  }
  const raw = getRawWindowSeries()
  if (!raw) return '0%'
  const t = sum(raw.counts)
  if (t <= 0) return '0%'
  return `${((sum(raw.s2xx) / t) * 100).toFixed(2)}%`
})

const errorRate = computed(() => {
  // 如果有历史数据，使用历史数据
  if (historicalData.value) {
    const t = sum(historicalData.value.counts || [])
    if (t <= 0) return '0%'
    return `${(((sum(historicalData.value.s5xx || []) + sum(historicalData.value.s0 || [])) / t) * 100).toFixed(2)}%`
  }
  const raw = getRawWindowSeries()
  if (!raw) return '0%'
  const t = sum(raw.counts)
  if (t <= 0) return '0%'
  return `${(((sum(raw.s5xx) + sum(raw.s0)) / t) * 100).toFixed(2)}%`
})

const avgLatency = computed(() => {
  // 如果有历史数据，使用历史数据
  if (historicalData.value && historicalData.value.avgLatencyMs.length > 0) {
    return Math.round(sum(historicalData.value.avgLatencyMs) / historicalData.value.avgLatencyMs.length)
  }
  const raw = getRawWindowSeries()
  if (!raw || raw.avgLatencyMs.length === 0) return 0
  return Number((sum(raw.avgLatencyMs) / raw.avgLatencyMs.length).toFixed(4))
})

const topRouteErr = computed(() => {
  // 如果有历史数据，只使用历史数据
  if (historicalData.value && historicalData.value.topRouteErr && historicalData.value.topRouteErr.length > 0) {
    return historicalData.value.topRouteErr
  }
  const raw = getRawWindowSeries()
  return raw?.topRouteErr || []
})
const topUpErr = computed(() => {
  // 如果有历史数据，只使用历史数据
  if (historicalData.value && historicalData.value.topUpErr && historicalData.value.topUpErr.length > 0) {
    return historicalData.value.topUpErr
  }
  const raw = getRawWindowSeries()
  return raw?.topUpErr || []
})

// Top Routes（matched_route_id）来源：后端 get_dashboard_stats
const topRoutes = ref<Array<{ item: string; count: number }>>([])
const topPaths = ref<Array<{ item: string; count: number }>>([])
const topClientIps = ref<Array<{ item: string; count: number }>>([])
const topUpstreamErrors = ref<Array<{ item: string; count: number }>>([])
const phaseTiming = ref<PhaseTimingStats | null>(null)

const formatPhaseMs = (v?: number) => Number.isFinite(v as number) ? Number(v as number).toFixed(2) : '0.00'

const fetchTopRoutes = async () => {
  if (!props.isActive) return

  // 仅在“载入历史数据”模式下查询数据库（get_dashboard_stats 走 DB）
  if (!historicalData.value || !dateRange.value || dateRange.value.length !== 2) {
    topRoutes.value = []
    return
  }

  const listenAddr = isGlobalListen(selectedListen.value) ? '' : selectedListen.value

  const [startMs, endMs] = dateRange.value
  const start = Math.floor(startMs / 1000)
  const end = Math.floor(endMs / 1000)

  try {
    // @ts-ignore
    const res: DashboardStatsResponse = await GetDashboardStats({
      start_time: start,
      end_time: end,
      listen_addr: listenAddr,
      granularity_secs: Math.max(1, Math.floor((end - start) / 60)),
    })

    const list = Array.isArray(res?.top_routes) ? res.top_routes : []
    topRoutes.value = list
      .filter((it) => it && typeof it.item === 'string' && it.item.trim() !== '')
      .map((it) => ({ item: it.item, count: Number(it.count) || 0 }))

    // 历史模式下的 Top（来自 get_dashboard_stats）
    topPaths.value = Array.isArray(res?.top_paths)
      ? res.top_paths.map((it) => ({ item: String(it.item || ''), count: Number(it.count) || 0 }))
      : []

    topClientIps.value = Array.isArray(res?.top_ips)
      ? res.top_ips.map((it) => ({ item: String(it.item || ''), count: Number(it.count) || 0 }))
      : []

    topUpstreamErrors.value = Array.isArray(res?.top_upstream_errors)
      ? res.top_upstream_errors.map((it) => ({ item: String(it.item || ''), count: Number(it.count) || 0 }))
      : []

    phaseTiming.value = res?.phase_timing || null
  } catch (e) {
    topRoutes.value = []
    phaseTiming.value = null
  }
}

// 加载历史数据
const loadHistoricalData = async () => {
  if (!dateRange.value || dateRange.value.length !== 2) {
    ElMessage.warning(t('dashboard.selectDateRange'))
    return
  }

  const [startTime, endTime] = dateRange.value
  if (startTime >= endTime) {
    ElMessage.warning(t('dashboard.startTimeMustBeLess'))
    return
  }

  // 转换为秒级时间戳
  const startSec = Math.floor(startTime / 1000)
  const endSec = Math.floor(endTime / 1000)

  loadingHistorical.value = true
  try {
    const listenAddr = isGlobalListen(selectedListen.value) ? '' : selectedListen.value
    
    // @ts-ignore
    const response = await QueryHistoricalMetrics({
      start_time: startSec,
      end_time: endSec,
      listen_addr: listenAddr,
    })

    if (response && response.series) {
      const series: MetricsSeries = {
        timestamps: response.series.timestamps || [],
        counts: response.series.counts || [],
        s2xx: response.series.s2xx || [],
        s3xx: response.series.s3xx || [],
        s4xx: response.series.s4xx || [],
        s5xx: response.series.s5xx || [],
        s0: response.series.s0 || [],
        avgLatencyMs: response.series.avgLatencyMs || [],
        maxLatencyMs: response.series.maxLatencyMs || [],
        p50: response.series.p50 || [],
        p95: response.series.p95 || [],
        p99: response.series.p99 || [],
        upstreamDist: (response.series.upstreamDist || []).map((kv: any) => ({ key: kv.key || kv.Key || '', value: kv.value || kv.Value || 0 })),
        topRouteErr: (response.series.topRouteErr || []).map((kv: any) => ({ key: kv.key || kv.Key || '', value: kv.value || kv.Value || 0 })),
        topUpErr: (response.series.topUpErr || []).map((kv: any) => ({ key: kv.key || kv.Key || '', value: kv.value || kv.Value || 0 })),
        latencyDist: (response.series.latencyDist || []).map((kv: any) => ({ key: kv.key || kv.Key || '', value: kv.value || kv.Value || 0 })), 
      }
      historicalData.value = series
      // 载入历史数据后，Top Routes 才需要查询数据库
      fetchTopRoutes()
      ElMessage.success(t('dashboard.loadHistoricalSuccess', { count: series.timestamps.length }))
    } else {
      ElMessage.warning(t('dashboard.noHistoricalData'))
      historicalData.value = null
    }
  } catch (error: any) {
    console.error('加载历史数据失败:', error)
    ElMessage.error(t('dashboard.loadHistoricalFailed', { error: error.message || String(error) }))
    historicalData.value = null
  } finally {
    loadingHistorical.value = false
  }
}

// 清除历史数据
const clearHistoricalData = () => {
  historicalData.value = null
  // 清除历史数据时同步清空 Top（避免误以为还在历史模式）
  topRoutes.value = []
  topPaths.value = []
  topClientIps.value = []
  topUpstreamErrors.value = []
  phaseTiming.value = null
  ElMessage.info(t('dashboard.historicalDataCleared'))
}

const chartColors = ref({
  text: 'var(--text)',
  textMuted: 'var(--text-muted)',
  border: 'var(--border)',
  primary: 'var(--primary)',
  primaryLight: 'var(--primary-light)',
  success: 'var(--success)',
  warning: 'var(--warning)',
  danger: 'var(--danger)',
  info: '#0ea5e9',
  gray: '#6b7280',
  purple: '#8b5cf6',
  pink: '#ec4899',
  orange: '#f97316',
});

const updateChartColors = () => {
  const style = getComputedStyle(document.documentElement);
  chartColors.value = {
    text: style.getPropertyValue('--text').trim(),
    textMuted: style.getPropertyValue('--text-muted').trim(),
    border: style.getPropertyValue('--border').trim(),
    primary: style.getPropertyValue('--primary').trim(),
    primaryLight: style.getPropertyValue('--primary-light').trim(),
    success: style.getPropertyValue('--success').trim(),
    warning: style.getPropertyValue('--warning').trim(),
    danger: style.getPropertyValue('--danger').trim(),
    info: '#0ea5e9', // Assuming this is static
    gray: '#6b7280', // Assuming this is static
    purple: '#8b5cf6', // Assuming this is static
    pink: '#ec4899', // Assuming this is static
    orange: '#f97316', // Assuming this is static
  };
};

const baseOption = computed<EChartsOption>(() => ({
  backgroundColor: 'transparent',
  textStyle: { color: chartColors.value.text },
  animation: false,
  renderer: 'canvas',
}));

const commonAxis = computed(() => ({
  axisLabel: { color: chartColors.value.textMuted },
  axisLine: { lineStyle: { color: chartColors.value.border } },
  splitLine: { lineStyle: { color: chartColors.value.border, type: 'dashed', opacity: 0.5 } },
}));

// 获取对齐后的视图数据（历史数据优先，如果有历史数据则不显示实时数据）
const alignedView = computed(() => {
  if (!shouldRunRealtime.value) return null
  
  // 如果有历史数据，只显示历史数据
  if (historicalData.value) {
    const histSeries = historicalData.value
    if (histSeries.timestamps.length === 0) {
      return null
    }
    
    const step = histSeries.timestamps.length > maxPoints ? Math.ceil(histSeries.timestamps.length / maxPoints) : 1
    const idx: number[] = []
    for (let i = 0; i < histSeries.timestamps.length; i += step) idx.push(i)

    const x = idx.map(i => formatTime(histSeries.timestamps[i]))
    const view: MetricsSeries = {
      timestamps: idx.map(i => histSeries.timestamps[i]),
      counts: idx.map(i => histSeries.counts[i] || 0),
      s2xx: idx.map(i => histSeries.s2xx[i] || 0),
      s3xx: idx.map(i => histSeries.s3xx[i] || 0),
      s4xx: idx.map(i => histSeries.s4xx[i] || 0),
      s5xx: idx.map(i => histSeries.s5xx[i] || 0),
      s0: idx.map(i => histSeries.s0[i] || 0),
      avgLatencyMs: idx.map(i => histSeries.avgLatencyMs[i] || 0),
      maxLatencyMs: idx.map(i => histSeries.maxLatencyMs[i] || 0),
      p50: idx.map(i => histSeries.p50?.[i] || 0),
      p95: idx.map(i => histSeries.p95?.[i] || 0),
      p99: idx.map(i => histSeries.p99?.[i] || 0),
      upstreamDist: histSeries.upstreamDist || [],
      topRouteErr: histSeries.topRouteErr || [],
      topUpErr: histSeries.topUpErr || [],
    }

    return {
      x,
      series: view,
      window: selectedWindow.value,
      listen: selectedListen.value,
    }
  }
  
  // 没有历史数据时，显示实时数据
  return buildAlignedView()
})

const qpsOption = computed<EChartsOption>(() => {
  const v = alignedView.value
  if (!v) {
    return {
      ...baseOption.value,
      xAxis: { type: 'category', data: [] },
      yAxis: { type: 'value' },
      series: [],
    }
  }
  return {
    ...baseOption.value,
    tooltip: { trigger: 'axis', axisPointer: { type: 'line' } },
    grid: { left: 44, right: 20, top: 30, bottom: 30 },
    xAxis: { type: 'category', data: v.x, boundaryGap: false, ...commonAxis.value },
    yAxis: { type: 'value', ...commonAxis.value },
    series: [
      { 
        name: 'QPS', 
        type: 'line', 
        smooth: false,
        showSymbol: false, 
        large: true,
        largeThreshold: 200,
        lineStyle: { width: 2, color: chartColors.value.primary }, 
        areaStyle: { opacity: 0.18, color: chartColors.value.primaryLight }, 
        data: v.series.counts || [],
        sampling: 'lttb',
      },
    ],
  }
})

const statusOption = computed<EChartsOption>(() => {
  const v = alignedView.value
  if (!v) {
    return {
      ...baseOption.value,
      xAxis: { type: 'category', data: [] },
      yAxis: { type: 'value' },
      series: [],
    }
  }
  const s = v.series
  return {
    ...baseOption.value,
    tooltip: { trigger: 'axis', axisPointer: { type: 'line' } },
    legend: { top: 0, textStyle: { color: chartColors.value.textMuted } },
    grid: { left: 44, right: 20, top: 44, bottom: 30 },
    xAxis: { type: 'category', data: v.x, boundaryGap: false, ...commonAxis.value },
    yAxis: { type: 'value', ...commonAxis.value },
    series: [
      { name: '2xx', type: 'line', smooth: false, showSymbol: false, large: true, largeThreshold: 200, data: s.s2xx, lineStyle: { width: 2, color: chartColors.value.success }, sampling: 'lttb' },
      { name: '3xx', type: 'line', smooth: false, showSymbol: false, large: true, largeThreshold: 200, data: s.s3xx, lineStyle: { width: 2, color: chartColors.value.info }, sampling: 'lttb' },
      { name: '4xx', type: 'line', smooth: false, showSymbol: false, large: true, largeThreshold: 200, data: s.s4xx, lineStyle: { width: 2, color: chartColors.value.warning }, sampling: 'lttb' },
      { name: '5xx', type: 'line', smooth: false, showSymbol: false, large: true, largeThreshold: 200, data: s.s5xx, lineStyle: { width: 2, color: chartColors.value.danger }, sampling: 'lttb' },
      { name: 'err', type: 'line', smooth: false, showSymbol: false, large: true, largeThreshold: 200, data: s.s0, lineStyle: { width: 2, color: chartColors.value.gray }, sampling: 'lttb' },
    ],
  }
})

const latencyOption = computed<EChartsOption>(() => {
  const v = alignedView.value
  if (!v) {
    return {
      ...baseOption.value,
      xAxis: { type: 'category', data: [] },
      yAxis: { type: 'value' },
      series: [],
    }
  }
  const s = v.series
  return {
    ...baseOption.value,
    tooltip: { trigger: 'axis', axisPointer: { type: 'line' } },
    legend: { top: 0, textStyle: { color: chartColors.value.textMuted } },
    grid: { left: 44, right: 20, top: 44, bottom: 30 },
    xAxis: { type: 'category', data: v.x, boundaryGap: false, ...commonAxis.value },
    yAxis: { type: 'value', ...commonAxis.value },
    series: [
      { name: 'avg', type: 'line', smooth: false, showSymbol: false, large: true, largeThreshold: 200, data: s.avgLatencyMs, lineStyle: { width: 2, color: chartColors.value.purple }, sampling: 'lttb' },
      { name: 'max', type: 'line', smooth: false, showSymbol: false, large: true, largeThreshold: 200, data: s.maxLatencyMs, lineStyle: { width: 2, color: chartColors.value.pink }, sampling: 'lttb' },
    ],
  }
})

const pOption = computed<EChartsOption>(() => {
  const v = alignedView.value
  if (!v) {
    return {
      ...baseOption.value,
      xAxis: { type: 'category', data: [] },
      yAxis: { type: 'value' },
      series: [],
    }
  }
  const s = v.series
  return {
    ...baseOption.value,
    tooltip: { trigger: 'axis', axisPointer: { type: 'line' } },
    legend: { top: 0, textStyle: { color: chartColors.value.textMuted } },
    grid: { left: 44, right: 20, top: 44, bottom: 30 },
    xAxis: { type: 'category', data: v.x, boundaryGap: false, ...commonAxis.value },
    yAxis: { type: 'value', ...commonAxis.value },
    series: [
      { name: 'p50', type: 'line', smooth: false, showSymbol: false, large: true, largeThreshold: 200, data: s.p50 || [], lineStyle: { width: 2, color: chartColors.value.primary }, sampling: 'lttb' },
      { name: 'p95', type: 'line', smooth: false, showSymbol: false, large: true, largeThreshold: 200, data: s.p95 || [], lineStyle: { width: 2, color: chartColors.value.orange }, sampling: 'lttb' },
      { name: 'p99', type: 'line', smooth: false, showSymbol: false, large: true, largeThreshold: 200, data: s.p99 || [], lineStyle: { width: 2, color: chartColors.value.danger }, sampling: 'lttb' },
    ],
  }
})

const upDistOption = computed<EChartsOption>(() => {
  const v = alignedView.value
  if (!v) {
    return {
      ...baseOption.value,
      xAxis: { type: 'value' },
      yAxis: { type: 'category', data: [] },
      series: [],
    }
  }
  const upstreamDist = historicalData.value?.upstreamDist || v.series.upstreamDist || []
  const data = upstreamDist.map(it => ({ name: it.key, value: it.value })).sort((a, b) => a.value - b.value)
  return {
    ...baseOption.value,
    tooltip: { trigger: 'axis', axisPointer: { type: 'shadow' } },
    grid: { left: 20, right: 40, top: 20, bottom: 20, containLabel: true },
    xAxis: { type: 'value', ...commonAxis.value },
    yAxis: { type: 'category', data: data.map(d => d.name), axisLabel: { color: chartColors.value.textMuted, fontSize: 10 } },
    series: [{ name: t('dashboard.requestCount'), type: 'bar', data: data.map(d => d.value), itemStyle: { color: chartColors.value.primary, borderRadius: [0, 4, 4, 0] } }],
  }
})

const rateOption = computed<EChartsOption>(() => {
  const v = alignedView.value
  if (!v) {
    return {
      ...baseOption.value,
      xAxis: { type: 'category', data: [] },
      yAxis: { type: 'value' },
      series: [],
    }
  }
  const s = v.series
  const total = s.counts.map((c) => (Number.isFinite(c) ? c : 0))
  const err = s.s5xx.map((v, i) => (Number.isFinite(v) ? v : 0) + (Number.isFinite(s.s0[i]) ? s.s0[i] : 0))
  const ok = s.s2xx.map((v) => (Number.isFinite(v) ? v : 0))
  const errRate = total.map((t, i) => (t > 0 ? (err[i] / t) * 100 : 0))
  const okRate = total.map((t, i) => (t > 0 ? (ok[i] / t) * 100 : 0))
  return {
    ...baseOption.value,
    tooltip: { trigger: 'axis', axisPointer: { type: 'line' }, valueFormatter: (v: any) => `${Number(v).toFixed(2)}%` },
    legend: { top: 0, textStyle: { color: chartColors.value.textMuted } },
    grid: { left: 50, right: 20, top: 44, bottom: 30 },
    xAxis: { type: 'category', data: v.x, boundaryGap: false, ...commonAxis.value },
    yAxis: { type: 'value', min: 0, max: 100, axisLabel: { color: chartColors.value.textMuted, formatter: (v: number) => `${v}%` }, ...commonAxis.value },
    series: [
      { name: t('dashboard.successRate'), type: 'line', smooth: false, showSymbol: false, large: true, largeThreshold: 200, data: okRate, lineStyle: { width: 2, color: chartColors.value.success }, sampling: 'lttb' },
      { name: t('dashboard.errorRate'), type: 'line', smooth: false, showSymbol: false, large: true, largeThreshold: 200, data: errRate, lineStyle: { width: 2, color: chartColors.value.danger }, sampling: 'lttb' },
    ],
  }
})

const statusPieOption = computed<EChartsOption>(() => {
  let raw = historicalData.value ? {
    s2xx: historicalData.value.s2xx || [],
    s3xx: historicalData.value.s3xx || [],
    s4xx: historicalData.value.s4xx || [],
    s5xx: historicalData.value.s5xx || [],
    s0: historicalData.value.s0 || [],
  } : getRawWindowSeries()
  if (!raw) {
    return {
      ...baseOption.value,
      graphic: {
        type: 'text',
        left: 'center',
        top: 'middle',
        style: {
          text: t('dashboard.noDataText'),
          fontSize: 14,
          fill: chartColors.value.textMuted,
        },
      },
      series: [{ type: 'pie', data: [] }],
    }
  }
  const total2xx = sum(raw.s2xx || [])
  const total3xx = sum(raw.s3xx || [])
  const total4xx = sum(raw.s4xx || [])
  const total5xx = sum(raw.s5xx || [])
  const total0 = sum(raw.s0 || [])
  const total = total2xx + total3xx + total4xx + total5xx + total0
  
  if (total === 0) {
    return {
      ...baseOption.value,
      graphic: {
        type: 'text',
        left: 'center',
        top: 'middle',
        style: {
          text: t('dashboard.noDataText'),
          fontSize: 14,
          fill: chartColors.value.textMuted,
        },
      },
      series: [{ type: 'pie', data: [] }],
    }
  }
  
  const data: Array<{ name: string; value: number; itemStyle: { color: string } }> = []
  if (total2xx > 0) data.push({ name: '2xx', value: total2xx, itemStyle: { color: chartColors.value.success } })
  if (total3xx > 0) data.push({ name: '3xx', value: total3xx, itemStyle: { color: chartColors.value.info } })
  if (total4xx > 0) data.push({ name: '4xx', value: total4xx, itemStyle: { color: chartColors.value.warning } })
  if (total5xx > 0) data.push({ name: '5xx', value: total5xx, itemStyle: { color: chartColors.value.danger } })
  if (total0 > 0) data.push({ name: t('dashboard.errorLabel'), value: total0, itemStyle: { color: chartColors.value.gray } })
  
  return {
    ...baseOption.value,
    tooltip: {
      trigger: 'item',
      formatter: (params: any) => {
        if (!params || !params.value) return ''
        const percent = total > 0 ? ((params.value / total) * 100).toFixed(2) : '0.00'
        return `${params.name}<br/>${params.value} ${t('dashboard.timesUnit')} (${percent}%)`
      },
    },
    legend: {
      orient: 'vertical',
      left: 'left',
      top: 'middle',
      textStyle: { color: chartColors.value.textMuted },
    },
    series: [
      {
        name: t('dashboard.statusCode'),
        type: 'pie',
        radius: ['40%', '70%'],
        center: ['60%', '50%'],
        avoidLabelOverlap: false,
        itemStyle: {
          borderRadius: 8,
          borderColor: 'var(--card-bg)',
          borderWidth: 4,
        },
        label: {
          show: true,
          formatter: (params: any) => {
            if (!params || !params.value) return ''
            const percent = total > 0 ? ((params.value / total) * 100).toFixed(1) : '0.0'
            return `${params.name}\n${percent}%`
          },
        },
        emphasis: {
          label: {
            show: true,
            fontSize: 14,
            fontWeight: 'bold',
          },
        },
        data,
      },
    ],
  }
})

const throughputOption = computed<EChartsOption>(() => {
  const v = alignedView.value
  if (!v) {
    return {
      ...baseOption.value,
      xAxis: { type: 'category', data: [] },
      yAxis: { type: 'value' },
      series: [],
    }
  }
  const s = v.series
  let cumulative = 0
  const cumulativeData = s.counts.map((c) => {
    cumulative += Number.isFinite(c) ? c : 0
    return cumulative
  })
  
  return {
    ...baseOption.value,
    tooltip: { trigger: 'axis', axisPointer: { type: 'line' } },
    grid: { left: 44, right: 20, top: 30, bottom: 30 },
    xAxis: { type: 'category', data: v.x, boundaryGap: false, ...commonAxis.value },
    yAxis: { type: 'value', ...commonAxis.value },
    series: [
      {
        name: t('dashboard.cumulativeRequests'),
        type: 'line',
        smooth: false,
        showSymbol: false,
        large: true,
        largeThreshold: 200,
        data: cumulativeData,
        lineStyle: { width: 2, color: chartColors.value.purple },
        areaStyle: { opacity: 0.2, color: chartColors.value.purple },
        sampling: 'lttb',
      },
    ],
  }
})

const latencyDistOption = computed<EChartsOption>(() => {
  const v = alignedView.value
  if (!v) {
    return {
      ...baseOption.value,
      xAxis: { type: 'category', data: [] },
      yAxis: { type: 'value' },
      series: [],
    }
  }
  const dist = (historicalData.value?.latencyDist || v.series.latencyDist || [])
  const names = dist.map(d => d.key)
  const vals = dist.map(d => d.value)

  return {
    ...baseOption.value,
    tooltip: { trigger: 'axis', axisPointer: { type: 'shadow' } },
    grid: { left: 44, right: 20, top: 30, bottom: 50 },
    xAxis: { type: 'category', data: names, ...commonAxis.value, axisLabel: { ...commonAxis.value.axisLabel, rotate: 30 } },
    yAxis: { type: 'value', ...commonAxis.value },
    series: [
      { name: t('dashboard.requestCount'), type: 'bar', data: vals, itemStyle: { color: chartColors.value.purple }, barWidth: '60%' },
    ],
  }
})

// matched_route_id -> 更直观的显示名（upstreams + path）
const routeLabelMap = ref<Record<string, string>>({})

const refreshRouteLabelMap = async () => {
  try {
    const cfg = (await GetConfig()) as any
    const map: Record<string, string> = {}

    const rules = Array.isArray(cfg?.rules) ? cfg.rules : []
    for (const rule of rules) {
      const routes = Array.isArray(rule?.routes) ? rule.routes : []
      for (const rt of routes) {
        const id = String(rt?.id || '').trim()
        if (!id) continue

        const path = String(rt?.path || '/').trim() || '/'
        const ups = Array.isArray(rt?.upstreams) ? rt.upstreams : []
        const upsText = ups
          .map((u: any) => String(u?.url || '').trim())
          .filter((s: string) => !!s)
          .join(',')

        const staticDir = String(rt?.static_dir || '').trim()

        // 更直观显示：
        // - upstreams 非空：显示 upstreams + path
        // - upstreams 为空且 static_dir 非空：显示 static_dir + path（静态资源）
        // - 否则回退到 id
        const label = upsText
          ? `${upsText} ${path}`.trim()
          : (staticDir ? `${staticDir} ${path}`.trim() : id)

        map[id] = label || id
      }
    }

    routeLabelMap.value = map
  } catch {
    routeLabelMap.value = {}
  }
}

const displayRouteName = (routeId: string) => {
  const id = String(routeId || '').trim()
  if (!id) return ''
  return routeLabelMap.value[id] || id
}

const topRoutesOption = computed<EChartsOption>(() => {
  const rows = topRoutes.value || []
  if (!rows.length) {
    return {
      ...baseOption.value,
      xAxis: { type: 'value', ...commonAxis.value },
      yAxis: { type: 'category', data: [] },
      series: [],
    }
  }

  // 为了更好看：从小到大排序（横向条形图从下往上）
  const data = rows
    .map((r) => ({ name: displayRouteName(r.item), value: Number(r.count) || 0 }))
    .sort((a, b) => a.value - b.value)

  return {
    ...baseOption.value,
    tooltip: { trigger: 'axis', axisPointer: { type: 'shadow' } },
    grid: { left: 20, right: 40, top: 20, bottom: 20, containLabel: true },
    xAxis: { type: 'value', ...commonAxis.value },
    yAxis: {
      type: 'category',
      data: data.map((d) => d.name),
      axisLabel: { color: chartColors.value.textMuted, fontSize: 10 },
    },
    series: [
      {
        name: t('dashboard.requestCount'),
        type: 'bar',
        data: data.map((d) => d.value),
        itemStyle: { color: chartColors.value.primary, borderRadius: [0, 4, 4, 0] },
      },
    ],
  }
})

// ---- 订阅/轮询策略（已按后端推送优化） ----
let subscribed = false
let metricsUnlisten: (() => void) | null = null
let pollingTimer: number | null = null
let heartbeatCleanup: (() => void) | null = null
let pollingEnabled = false

let lastEventTime = 0
let eventEverReceived = false

const POLLING_INTERVAL = 3000 // 轮询兜底间隔
const EVENT_TIMEOUT = 6000 // 6 秒收不到事件则启动轮询兜底

const shouldRunRealtime = computed(
  () => (props.isActive && windowVisible.value) || previewSyncActive.value,
)

const syncWindowVisibility = () => {
  windowVisible.value = !document.hidden
}

const processMetricsPayload = (payload: MetricsPayload) => {
  if (!shouldRunRealtime.value) return
  latest.value = payload
  lastEventTime = Date.now()
  eventEverReceived = true

  // 实时 Top Routes：来自 get_metrics（内存聚合，不查库）
  if (!historicalData.value) {
    topRoutes.value = Array.isArray(payload.topRoutes)
      ? payload.topRoutes.map((it) => ({ item: String(it.item || ''), count: Number(it.count) || 0 }))
      : []
    topPaths.value = Array.isArray(payload.topPaths)
      ? payload.topPaths.map((it) => ({ item: String(it.item || ''), count: Number(it.count) || 0 }))
      : []
    topClientIps.value = Array.isArray(payload.topClientIps)
      ? payload.topClientIps.map((it) => ({ item: String(it.item || ''), count: Number(it.count) || 0 }))
      : []
    topUpstreamErrors.value = Array.isArray(payload.topUpstreamErrors)
      ? payload.topUpstreamErrors.map((it) => ({ item: String(it.item || ''), count: Number(it.count) || 0 }))
      : []
  }

  if (!isGlobalListen(selectedListen.value) && !listenAddrs.value.includes(selectedListen.value)) {
    selectedListen.value = GLOBAL_LISTEN_ADDR
  }

  const maxWin = selectedWindow.value >= 3600
    ? (payload.minuteWindowSeconds || payload.windowSeconds)
    : payload.windowSeconds
  if (maxWin && selectedWindow.value > maxWin) {
    selectedWindow.value = maxWin
  }
}

const onMetrics = (payload: MetricsPayload) => {
  processMetricsPayload(payload)
  // 收到事件立即停轮询（事件为主）
  stopPolling()

  // Top Routes：实时模式不查库（仅载入历史数据后才查询）
}

const normalizeMetricsPayload = (payload: any): MetricsPayload => ({
  windowSeconds: Number(payload?.windowSeconds) || 0,
  listenAddrs: Array.isArray(payload?.listenAddrs) ? payload.listenAddrs : [],
  byListenAddr:
    payload?.byListenAddr && typeof payload.byListenAddr === 'object'
      ? payload.byListenAddr
      : {},
  minuteWindowSeconds:
    payload?.minuteWindowSeconds == null ? undefined : Number(payload.minuteWindowSeconds) || 0,
  byListenMinute:
    payload?.byListenMinute && typeof payload.byListenMinute === 'object'
      ? payload.byListenMinute
      : undefined,
  topRoutes: Array.isArray(payload?.topRoutes) ? payload.topRoutes : undefined,
  topPaths: Array.isArray(payload?.topPaths) ? payload.topPaths : undefined,
  topClientIps: Array.isArray(payload?.topClientIps) ? payload.topClientIps : undefined,
  topUpstreamErrors: Array.isArray(payload?.topUpstreamErrors) ? payload.topUpstreamErrors : undefined,
})

const startPolling = () => {
  if (pollingTimer) return
  pollingEnabled = true
  
  const poll = async () => {
    if (!shouldRunRealtime.value || !pollingEnabled) {
      stopPolling()
      return
    }
    
    try {
      const payload = await GetMetrics()
      processMetricsPayload(normalizeMetricsPayload(payload))

      // Top Routes：实时模式不查库（仅载入历史数据后才查询）
    } catch (err) {
      console.error('轮询获取 metrics 失败:', err)
    }

    if (!pollingEnabled || !shouldRunRealtime.value) {
      return
    }
    pollingTimer = window.setTimeout(poll, POLLING_INTERVAL)
  }
  
  poll()
}

const stopPolling = () => {
  pollingEnabled = false
  if (pollingTimer) {
    clearTimeout(pollingTimer)
    pollingTimer = null
  }
}

const startHeartbeat = () => {
  const heartbeatInterval = setInterval(() => {
    if (!shouldRunRealtime.value) return
    
    const now = Date.now()

    // 已经收过事件，但长时间收不到：启用轮询兜底
    if (subscribed && eventEverReceived && now - lastEventTime > EVENT_TIMEOUT) {
      startPolling()
    }
    
    // 从未收到事件：给一点时间等待后端首次推送，再启用轮询兜底
    if (subscribed && !eventEverReceived && now - lastEventTime > EVENT_TIMEOUT) {
      startPolling()
    }
  }, 2000)

  return () => clearInterval(heartbeatInterval)
  }

const refreshListenAddrs = async () => {
  try {
    const addrs = await GetListenAddrs()
    const list = [GLOBAL_LISTEN_ADDR, ...(Array.isArray(addrs) ? addrs : [])]
    const uniq = Array.from(new Set(list))
    listenAddrs.value = uniq

    if (!listenAddrs.value.includes(selectedListen.value)) {
      selectedListen.value = GLOBAL_LISTEN_ADDR
    }
  } catch (err) {
    console.error('获取监听地址列表失败:', err)
    listenAddrs.value = [GLOBAL_LISTEN_ADDR]
    selectedListen.value = GLOBAL_LISTEN_ADDR
  }
}

const startSubscription = () => {
  if (subscribed) return

  lastEventTime = Date.now()
  eventEverReceived = false

  EventsOn('metrics', onMetrics)
    .then((unlisten) => {
          metricsUnlisten = unlisten
          subscribed = true
    })
    .catch((err) => {
      console.error('EventsOn 订阅失败，启用轮询兜底:', err)
          subscribed = false
      startPolling()
        })
}

const stopSubscription = () => {
  if (metricsUnlisten) {
    try {
      EventsOff(metricsUnlisten)
      } catch (err) {
      console.error('EventsOff 失败:', err)
    }
    metricsUnlisten = null
  }
        subscribed = false
      }

watch(selectedListen, () => {
  if (!shouldRunRealtime.value) return
  if (lastValidView && lastValidView.listen !== selectedListen.value) {
    lastValidView = null
  }

  // Top Routes 仅在历史模式下查询数据库
  if (historicalData.value) {
    fetchTopRoutes()
  }
})

watch(selectedWindow, () => {
  if (!shouldRunRealtime.value) return
  if (lastValidView && lastValidView.window !== selectedWindow.value) {
    lastValidView = null
  }
})

watch(shouldRunRealtime, (active) => {
  if (active) {
    refreshListenAddrs()
    startSubscription()
    refreshRouteLabelMap()

    // Top Routes 仅在历史模式查询数据库；实时模式不查询（历史数据载入成功后会触发）

    if (!heartbeatCleanup) {
      heartbeatCleanup = startHeartbeat()
    }
    
    // 首次激活：等事件；若超时 heartbeat 会自动启动轮询兜底
  } else {
    stopPolling()
    stopSubscription()
    
    if (heartbeatCleanup) {
      heartbeatCleanup()
      heartbeatCleanup = null
    }
  }
}, { immediate: true })

let themeObserver: MutationObserver | null = null;

onMounted(() => {
  syncWindowVisibility()
  document.addEventListener('visibilitychange', syncWindowVisibility)
  updateChartColors();

  themeObserver = new MutationObserver((mutations) => {
    mutations.forEach((mutation) => {
      if (mutation.type === 'attributes' && mutation.attributeName === 'class') {
        updateChartColors();
      }
    });
  });

  themeObserver.observe(document.documentElement, {
    attributes: true,
  });

  if ((window as any).__TAURI_INTERNALS__) {
    listen('chart-preview-sync-request', onPreviewSyncRequest)
      .then((unlisten) => {
        previewSyncUnlisten = unlisten
      })
      .catch((err) => {
        console.error('listen chart-preview-sync-request failed:', err)
      })
  }
})

onBeforeUnmount(() => {
  stopPolling()
  stopSubscription()
  document.removeEventListener('visibilitychange', syncWindowVisibility)
  
  if (heartbeatCleanup) {
    heartbeatCleanup()
    heartbeatCleanup = null
  }
  if (previewSyncIdleTimer) {
    clearTimeout(previewSyncIdleTimer)
    previewSyncIdleTimer = null
  }
  if (themeObserver) {
    themeObserver.disconnect();
  }
  if (previewSyncUnlisten) {
    previewSyncUnlisten()
    previewSyncUnlisten = null
  }
})
</script>

<style scoped>
.config-card {
  background: transparent;
  border-radius: 0;
  padding: 0;
  box-shadow: none;
  border: none;
}

.config-page {
  height: 100%;
  overflow-y: auto;
  padding: 16px;
}

.header {
  display: flex;
  justify-content: space-between;
  align-items: flex-end;
  gap: 16px;
  margin-bottom: 24px;
  flex-wrap: wrap;
}

h3 {
  font-size: 24px;
  font-weight: 700;
  margin: 0;
  color: var(--text);
  background: linear-gradient(135deg, var(--primary), var(--primary-hover));
  -webkit-background-clip: text;
  -webkit-text-fill-color: transparent;
  background-clip: text;
  letter-spacing: -0.5px;
}

.hint {
  color: var(--text-muted);
  font-size: 13px;
}

.controls {
  display: flex;
  gap: 12px;
  flex-wrap: wrap;
  align-items: center;
  padding: 10px 12px;
  background: var(--input-bg);
  border: 1px solid var(--border);
  border-radius: var(--radius-md);
}

.grid {
  display: grid;
  grid-template-columns: repeat(12, 1fr);
  gap: 16px;
}

.panel {
  border: 1px solid var(--border);
  border-radius: var(--radius-lg);
  background: var(--card-bg);
  backdrop-filter: blur(12px);
  transition: all 0.3s cubic-bezier(0.4, 0, 0.2, 1);
  grid-column: span 12;
}

.panel:hover {
  box-shadow: var(--shadow-glow);
  border-color: var(--border-hover);
}

.panel--stats {
  grid-column: span 12;
}

.panel--qps {
  grid-column: span 6;
}

.panel--status {
  grid-column: span 6;
}

.panel--latency {
  grid-column: span 8;
}

.panel--percentile {
  grid-column: span 4;
}

.panel--upstream {
  grid-column: span 12;
}

.panel--errors {
  grid-column: span 12;
}

.panel--rate {
  grid-column: span 6;
}

.panel--pie {
  grid-column: span 6;
}

.panel--throughput {
  grid-column: span 6;
}

.panel--latency-dist {
  grid-column: span 6;
}

.panel--top-routes {
  grid-column: span 6;
}


.panel :deep(.el-card__header) {
  border-bottom: 1px solid var(--border);
  padding: 14px 18px;
}

.panel :deep(.el-card__body) {
  padding: 16px 18px;
}

.panel-title {
  font-size: 16px;
  font-weight: 600;
  color: var(--text);
}

.panel-header {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 8px;
}

.panel-header-left {
  display: flex;
  align-items: center;
  gap: 6px;
}

.panel-preview-btn {
  font-size: 12px;
}

.chart {
  height: 300px;
  width: 100%;
}

.stats {
  display: grid;
  grid-template-columns: repeat(auto-fit, minmax(170px, 1fr));
  gap: 14px;
}

.stat {
  text-align: center;
  padding: 10px 8px;
  border-radius: var(--radius-sm);
  background: var(--input-bg);
  border: 1px solid var(--border);
}

.stat-label {
  font-size: 14px;
  color: var(--text-muted);
  margin-bottom: 8px;
}

.phase-stats {
  margin-top: 14px;
  border: 1px solid var(--border);
  border-radius: var(--radius-sm);
  padding: 12px;
  background: var(--card-bg);
}

.phase-title {
  font-size: 13px;
  color: var(--text-muted);
  margin-bottom: 10px;
}

.phase-grid {
  display: grid;
  grid-template-columns: repeat(3, minmax(0, 1fr));
  gap: 10px;
}

.phase-item {
  border: 1px solid var(--border);
  border-radius: var(--radius-sm);
  background: var(--input-bg);
  padding: 10px;
}

.phase-name {
  font-size: 13px;
  font-weight: 600;
  margin-bottom: 8px;
}

.phase-line {
  font-size: 12px;
  color: var(--text-muted);
  line-height: 1.6;
}

.tables {
  display: grid;
  grid-template-columns: repeat(2, minmax(0, 1fr));
  gap: 14px;
}

.table :deep(.el-card__header) {
  padding: 12px 16px;
  border-bottom: 1px solid var(--border);
}

.table-title {
  font-size: 14px;
  font-weight: 600;
  color: var(--text);
}

.rows {
  display: flex;
  flex-direction: column;
  gap: 10px;
  max-height: 240px;
  overflow: auto;
}

.row {
  display: flex;
  justify-content: space-between;
  align-items: center;
  padding: 8px 12px;
  border-radius: var(--radius-sm);
  background: var(--input-bg);
  transition: background-color 0.2s;
}

.row:hover {
  background: var(--input-focus);
}

.row .k {
  font-family: 'JetBrains Mono', 'Consolas', monospace;
  font-size: 13px;
  color: var(--text-muted);
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.row .v {
  font-weight: 600;
  color: var(--text);
  font-size: 14px;
}

/* Responsive Grid */
@media (max-width: 1200px) {
  .panel--qps,
  .panel--status,
  .panel--rate,
  .panel--pie,
  .panel--throughput,
  .panel--latency-dist {
    grid-column: span 12;
  }

  .panel--latency {
    grid-column: span 7;
  }

  .panel--percentile {
    grid-column: span 5;
  }
}

@media (max-width: 768px) {
  .config-page { padding: 8px; }
  .grid { gap: 16px; }
  .header { margin-bottom: 16px; }
  h3 { font-size: 20px; }
  .chart { height: 250px; }
  .stats { grid-template-columns: repeat(2, 1fr); }
  .tables { grid-template-columns: 1fr; }
  .phase-grid { grid-template-columns: 1fr; }
  
  .panel--latency,
  .panel--percentile {
    grid-column: span 12;
  }
}

@media (max-width: 480px) {
  .config-page { padding: 4px; }
  .grid { gap: 12px; }
  .header {
    flex-direction: column;
    align-items: flex-start;
    gap: 12px;
  }
  h3 { font-size: 18px; }
  .controls {
    flex-direction: column;
    align-items: stretch;
    width: 100%;
    padding: 8px;
  }
  .controls .el-form-item {
    width: 100%;
  }
  .stats { grid-template-columns: repeat(2, 1fr); }
  .stat-value { font-size: 22px; }
  .panel :deep(.el-card__body) {
    padding: 12px;
  }
}
</style>
