import * as echarts from 'echarts'
import { emit, listen } from '@tauri-apps/api/event'

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
const refreshStatusEl = document.getElementById('refresh-status') as HTMLElement | null
const autoRefreshToggle = document.getElementById('auto-refresh-toggle') as HTMLInputElement | null
const autoRefreshSeconds = document.getElementById('auto-refresh-seconds') as HTMLInputElement | null

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
  showEmpty('Preview data not found or expired.')
} else if (!chartEl || !payloadKey) {
  // no-op
} else {
  if (titleEl && currentPayload.title) {
    titleEl.textContent = String(currentPayload.title)
    document.title = String(currentPayload.title)
  }

  chartEl.className = ''
  chartEl.textContent = ''

  const chart = echarts.init(chartEl, undefined, { renderer: 'canvas' })
  chart.setOption(currentPayload.option, { notMerge: true, lazyUpdate: false })

  const resize = () => chart.resize()
  window.addEventListener('resize', resize)
  setTimeout(resize, 50)
  let autoRefreshTimer: number | null = null
  let syncing = false

  const applyPayload = (payload: PreviewPayload) => {
    if (!payload || !payload.option) return false
    currentPayload = payload
    chart.setOption(payload.option, { notMerge: true, lazyUpdate: false })
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
      setRefreshStatus('未找到预览数据')
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
      setRefreshStatus(ok ? `已刷新 ${new Date().toLocaleTimeString()}` : '刷新失败')
      syncing = false
      return
    }

    const requestId = `${Date.now()}-${Math.random().toString(36).slice(2)}`
    const responseEvent = 'chart-preview-sync-response'
    setRefreshStatus('同步中...')
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
        localStorage.setItem(payloadKey, JSON.stringify(nextPayload))
        applyPayload(nextPayload)
        setRefreshStatus(`已更新 ${new Date().toLocaleTimeString()}`)
      } else {
        setRefreshStatus('刷新失败')
      }
    } catch (e: any) {
      const msg = String(e?.message || e || '')
      if (msg) {
        setRefreshStatus(`同步失败: ${msg}`)
      } else {
        setRefreshStatus('同步超时')
      }
    } finally {
      if (refreshBtn) refreshBtn.disabled = false
      syncing = false
    }
  }

  const getRefreshIntervalMs = () => {
    const raw = Number(autoRefreshSeconds?.value || 5)
    const sec = Number.isFinite(raw) ? Math.max(1, Math.min(3600, Math.floor(raw))) : 5
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

  if (autoRefreshToggle) {
    autoRefreshToggle.addEventListener('change', () => {
      resetAutoRefreshTimer()
      if (autoRefreshToggle.checked) {
        setRefreshStatus('已开启自动刷新')
        void requestLatestFromSource()
      } else {
        setRefreshStatus('已关闭自动刷新')
      }
    })
  }

  if (autoRefreshSeconds) {
    autoRefreshSeconds.addEventListener('change', () => {
      if (autoRefreshToggle?.checked) {
        resetAutoRefreshTimer()
        setRefreshStatus(`自动刷新: ${autoRefreshSeconds.value || '5'} 秒`)
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
