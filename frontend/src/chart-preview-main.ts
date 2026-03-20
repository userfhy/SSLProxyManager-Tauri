import * as echarts from 'echarts'
import { emit, listen } from '@tauri-apps/api/event'
import { getCurrentWindow } from '@tauri-apps/api/window'
import { SaveChartPngWithDialog } from './api'

type PreviewPayload = {
  title?: string
  option?: any
  createdAt?: number
  source?: 'dashboard' | 'systemMetrics'
  chartKey?: string
}

const titleEl = document.getElementById('title') as HTMLElement | null
const chartEl = document.getElementById('chart') as HTMLDivElement | null
const refreshBtn = document.getElementById('refresh-btn') as HTMLButtonElement | null
const exportPngBtn = document.getElementById('export-png-btn') as HTMLButtonElement | null
const refreshStatusEl = document.getElementById('refresh-status') as HTMLElement | null
const autoRefreshToggle = document.getElementById('auto-refresh-toggle') as HTMLInputElement | null
const autoRefreshSeconds = document.getElementById('auto-refresh-seconds') as HTMLInputElement | null
const autoRefreshLabel = document.getElementById('auto-refresh-label') as HTMLElement | null
const autoRefreshSecondsLabel = document.getElementById('auto-refresh-seconds-label') as HTMLElement | null

const localeRaw = localStorage.getItem('locale') || 'zh-CN'
const locale: 'zh-CN' | 'en-US' = localeRaw === 'en-US' ? 'en-US' : 'zh-CN'

const shouldUseDarkMode = (): boolean => {
  const hour = new Date().getHours()
  return hour >= 18 || hour < 6
}

const applyWindowTheme = () => {
  const autoThemeEnabled = localStorage.getItem('autoThemeEnabled') !== 'false'
  const theme = localStorage.getItem('theme') || 'dark'
  let isDark = true
  if (autoThemeEnabled || theme === 'auto') {
    isDark = shouldUseDarkMode()
  } else {
    isDark = theme !== 'light'
  }
  document.documentElement.classList.toggle('light-mode', !isDark)
}

applyWindowTheme()
window.setInterval(() => {
  applyWindowTheme()
}, 60000)

type PreviewI18n = Record<string, string>
const fallbackI18n: PreviewI18n = {
  chartPreview: 'Chart Preview',
  refresh: 'Refresh Latest',
  exportPng: 'Export PNG',
  autoRefresh: 'Auto Refresh',
  seconds: 'sec',
  loadingPreview: 'Loading preview...',
  previewNotFound: 'Preview data not found or expired.',
  noPreviewData: 'Preview data unavailable',
  refreshedAt: 'Refreshed {time}',
  syncInProgress: 'Syncing...',
  updatedAt: 'Updated {time}',
  refreshFailed: 'Refresh failed',
  syncFailed: 'Sync failed: {error}',
  syncTimeout: 'Sync timeout',
  autoOn: 'Auto refresh enabled',
  autoOff: 'Auto refresh disabled',
  autoInterval: 'Auto refresh: {seconds} sec',
  exportSuccess: 'PNG exported',
  exportFailed: 'Export failed',
}
let i18nText: PreviewI18n = fallbackI18n

const loadPreviewI18n = async () => {
  try {
    const mod = locale === 'en-US'
      ? await import('./i18n/chart-preview/en-US.json')
      : await import('./i18n/chart-preview/zh-CN.json')
    const loaded = (mod?.default || {}) as PreviewI18n
    i18nText = { ...fallbackI18n, ...loaded }
  } catch {
    i18nText = fallbackI18n
  }
}

const t = (key: string, params?: Record<string, string | number>) => {
  const tpl = i18nText[key] || key
  if (!params) return tpl
  return tpl.replace(/\{(\w+)\}/g, (_, p1) => String(params[p1] ?? ''))
}

const applyStaticI18nText = () => {
  if (refreshBtn) refreshBtn.textContent = t('refresh')
  if (exportPngBtn) exportPngBtn.textContent = t('exportPng')
  if (autoRefreshLabel) autoRefreshLabel.textContent = t('autoRefresh')
  if (autoRefreshSecondsLabel) autoRefreshSecondsLabel.textContent = t('seconds')
  if (titleEl && !titleEl.textContent?.trim()) {
    titleEl.textContent = t('chartPreview')
  }
  if (chartEl && chartEl.classList.contains('empty') && chartEl.textContent?.includes('Loading')) {
    chartEl.textContent = t('loadingPreview')
  }
}

void loadPreviewI18n().finally(() => {
  applyStaticI18nText()
})

const showEmpty = (msg: string) => {
  if (!chartEl) return
  chartEl.className = 'empty'
  chartEl.textContent = msg
}

const setRefreshStatus = (msg: string) => {
  if (!refreshStatusEl) return
  refreshStatusEl.textContent = msg
}

const params = new URLSearchParams(window.location.search)
const payloadKey = params.get('key') || ''

const readPayloadByKey = (key: string): PreviewPayload | null => {
  if (!key) return null
  const raw = localStorage.getItem(key)
  if (!raw) return null

  try {
    return JSON.parse(raw) as PreviewPayload
  } catch {
    return null
  }
}

let currentPayload = readPayloadByKey(payloadKey)
if (!currentPayload || !currentPayload.option) {
  showEmpty(t('previewNotFound'))
} else if (!chartEl || !payloadKey) {
  // no-op
} else {
  const normalizeOptionForPreview = (option: any) => {
    if (!option || typeof option !== 'object') return option
    const next = JSON.parse(JSON.stringify(option))
    if (next.tooltip && typeof next.tooltip === 'object' && !Array.isArray(next.tooltip)) {
      next.tooltip.renderMode = 'richText'
      next.tooltip.confine = true
    }
    return next
  }

  const formatLocalTimestamp = (d: Date) => {
    const pad = (n: number) => String(n).padStart(2, '0')
    const yyyy = d.getFullYear()
    const mm = pad(d.getMonth() + 1)
    const dd = pad(d.getDate())
    const hh = pad(d.getHours())
    const mi = pad(d.getMinutes())
    const ss = pad(d.getSeconds())
    return `${yyyy}${mm}${dd}-${hh}${mi}${ss}`
  }

  const waitNextFrame = () => new Promise<void>((resolve) => {
    requestAnimationFrame(() => resolve())
  })

  if (titleEl && currentPayload.title) {
    titleEl.textContent = String(currentPayload.title)
    document.title = String(currentPayload.title)
  }

  chartEl.className = ''
  chartEl.textContent = ''

  const chart = echarts.init(chartEl, undefined, { renderer: 'canvas' })
  chart.setOption(normalizeOptionForPreview(currentPayload.option), { notMerge: true, lazyUpdate: false })
  let pinnedTip: { seriesIndex: number; dataIndex: number } | null = null

  const applyPinnedTip = () => {
    if (!pinnedTip) return
    chart.dispatchAction({
      type: 'showTip',
      seriesIndex: pinnedTip.seriesIndex,
      dataIndex: pinnedTip.dataIndex,
    } as any)
  }

  const resize = () => chart.resize()
  window.addEventListener('resize', resize)
  setTimeout(resize, 50)
  let autoRefreshTimer: number | null = null
  let syncing = false

  const applyPayload = (payload: PreviewPayload) => {
    if (!payload || !payload.option) return false
    currentPayload = payload
    chart.setOption(normalizeOptionForPreview(payload.option), { notMerge: true, lazyUpdate: false })
    applyPinnedTip()
    if (titleEl && payload.title) {
      titleEl.textContent = String(payload.title)
      document.title = String(payload.title)
    }
    requestAnimationFrame(() => chart.resize())
    return true
  }

  const reloadFromLocalStorage = () => {
    const next = readPayloadByKey(payloadKey)
    if (!next || !next.option) return false
    return applyPayload(next)
  }

  const requestLatestFromSource = async () => {
    if (syncing) {
      return
    }
    syncing = true

    if (!currentPayload) {
      setRefreshStatus(t('noPreviewData'))
      syncing = false
      return
    }

    // 浏览器模式或无来源信息：回退到本地快照刷新
    if (
      !(window as any).__TAURI_INTERNALS__
      || !currentPayload.source
      || !currentPayload.chartKey
    ) {
      const ok = reloadFromLocalStorage()
      setRefreshStatus(ok ? t('refreshedAt', { time: new Date().toLocaleTimeString() }) : t('refreshFailed'))
      syncing = false
      return
    }

    const requestId = `${Date.now()}-${Math.random().toString(36).slice(2)}`
    const responseEvent = 'chart-preview-sync-response'
    setRefreshStatus(t('syncInProgress'))
    if (refreshBtn) refreshBtn.disabled = true

    try {
      const response: any = await new Promise((resolve, reject) => {
        let resolved = false
        let off: (() => void) | null = null
        const timeout = window.setTimeout(() => {
          if (resolved) return
          resolved = true
          if (off) off()
          reject(new Error('sync timeout'))
        }, 4000)

        listen(responseEvent, (event) => {
          const payload = event.payload as any
          if (!payload || payload.requestId !== requestId) {
            return
          }
          if (resolved) return
          resolved = true
          clearTimeout(timeout)
          if (off) off()
          resolve(payload)
        })
          .then((unlisten) => {
            off = unlisten
            emit('chart-preview-sync-request', {
              requestId,
              source: currentPayload?.source,
              chartKey: currentPayload?.chartKey,
              title: currentPayload?.title || '',
              requesterLabel: getCurrentWindow().label,
            }).catch((err) => {
              if (resolved) return
              resolved = true
              clearTimeout(timeout)
              off?.()
              reject(err)
            })
          })
          .catch((err) => {
            if (resolved) return
            resolved = true
            clearTimeout(timeout)
            reject(err)
          })
      })

      if (response?.ok && response?.option) {
        const nextPayload: PreviewPayload = {
          ...currentPayload,
          title: String(response.title || currentPayload.title || ''),
          option: response.option,
          createdAt: Date.now(),
        }
        applyPayload(nextPayload)
        setRefreshStatus(t('updatedAt', { time: new Date().toLocaleTimeString() }))
      } else {
        setRefreshStatus(t('refreshFailed'))
      }
    } catch (e: any) {
      const msg = String(e?.message || e || '')
      if (msg) {
        setRefreshStatus(t('syncFailed', { error: msg }))
      } else {
        setRefreshStatus(t('syncTimeout'))
      }
    } finally {
      if (refreshBtn) refreshBtn.disabled = false
      syncing = false
    }
  }

  const getRefreshIntervalMs = () => {
    const raw = Number(autoRefreshSeconds?.value || 10)
    const sec = Number.isFinite(raw) ? Math.max(1, Math.min(3600, Math.floor(raw))) : 10
    if (autoRefreshSeconds) {
      autoRefreshSeconds.value = String(sec)
    }
    return sec * 1000
  }

  const resetAutoRefreshTimer = () => {
    if (autoRefreshTimer) {
      window.clearInterval(autoRefreshTimer)
      autoRefreshTimer = null
    }
    if (!autoRefreshToggle?.checked) {
      return
    }
    const intervalMs = getRefreshIntervalMs()
    autoRefreshTimer = window.setInterval(() => {
      void requestLatestFromSource()
    }, intervalMs)
  }

  if (refreshBtn) {
    refreshBtn.addEventListener('click', () => {
      void requestLatestFromSource()
    })
  }

  chart.on('click', (params: any) => {
    if (params?.componentType !== 'series') return
    const seriesIndex = Number(params?.seriesIndex)
    const dataIndex = Number(params?.dataIndex)
    if (!Number.isFinite(seriesIndex) || !Number.isFinite(dataIndex)) return

    if (
      pinnedTip
      && pinnedTip.seriesIndex === seriesIndex
      && pinnedTip.dataIndex === dataIndex
    ) {
      pinnedTip = null
      chart.dispatchAction({ type: 'hideTip' } as any)
      setRefreshStatus(t('tooltipUnpinned'))
      return
    }

    pinnedTip = { seriesIndex, dataIndex }
    applyPinnedTip()
    setRefreshStatus(t('tooltipPinned'))
  })

  chart.getZr().on('mousemove', () => {
    applyPinnedTip()
  })

  chart.on('globalout', () => {
    applyPinnedTip()
  })

  window.addEventListener('keydown', (event) => {
    if (event.key !== 'Escape') return
    if (!pinnedTip) return
    pinnedTip = null
    chart.dispatchAction({ type: 'hideTip' } as any)
    setRefreshStatus(t('tooltipUnpinned'))
  })

  if (exportPngBtn) {
    exportPngBtn.addEventListener('click', async () => {
      try {
        if (pinnedTip) {
          applyPinnedTip()
          await waitNextFrame()
        }
        let dataUrl = ''
        if (typeof chart.getDataURL === 'function') {
          dataUrl = chart.getDataURL({
            type: 'png',
            pixelRatio: 2,
            backgroundColor: '#ffffff',
          })
        }
        if (!dataUrl) {
          const canvas = chartEl.querySelector('canvas') as HTMLCanvasElement | null
          dataUrl = canvas?.toDataURL('image/png') || ''
        }
        if (!dataUrl) {
          setRefreshStatus(t('exportFailed'))
          return
        }

        const safeTitle = String(currentPayload?.title || 'chart-preview')
          .replace(/[\\/:*?"<>|]+/g, '-')
          .replace(/\s+/g, '_')
          .slice(0, 64)
        const ts = formatLocalTimestamp(new Date())
        const fileName = `${safeTitle || 'chart-preview'}-${ts}.png`

        if ((window as any).__TAURI_INTERNALS__) {
          const savedPath = await SaveChartPngWithDialog(fileName, dataUrl)
          if (savedPath) {
            setRefreshStatus(t('exportSuccess'))
          } else {
            setRefreshStatus(t('exportFailed'))
          }
        } else {
          const a = document.createElement('a')
          a.href = dataUrl
          a.download = fileName
          document.body.appendChild(a)
          a.click()
          a.remove()
          setRefreshStatus(t('exportSuccess'))
        }
      } catch {
        setRefreshStatus(t('exportFailed'))
      }
    })
  }

  if (autoRefreshToggle) {
    autoRefreshToggle.addEventListener('change', () => {
      resetAutoRefreshTimer()
      if (autoRefreshToggle.checked) {
        setRefreshStatus(t('autoOn'))
        void requestLatestFromSource()
      } else {
        setRefreshStatus(t('autoOff'))
      }
    })
  }

  if (autoRefreshSeconds) {
    autoRefreshSeconds.addEventListener('change', () => {
      if (autoRefreshToggle?.checked) {
        resetAutoRefreshTimer()
        setRefreshStatus(t('autoInterval', { seconds: autoRefreshSeconds.value || '10' }))
      }
    })
  }

  window.addEventListener('beforeunload', () => {
    if (autoRefreshTimer) {
      window.clearInterval(autoRefreshTimer)
      autoRefreshTimer = null
    }
  })
}
