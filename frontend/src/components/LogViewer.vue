<template>
  <el-card class="log-page" shadow="never">
    <template #header>
      <div class="log-header">
        <h3 class="log-title">{{ $t('logViewer.title') }}</h3>
        <div class="log-actions">
          <el-text type="info" size="small" class="log-count">
            {{ $t('logViewer.totalLogs', { total: totalLogCount, display: displayLogs.length }) }}
          </el-text>
          <el-button 
            v-if="!realtimeEnabled"
            @click="refreshLogs"
            type="primary"
            size="small"
          >
            {{ $t('logViewer.refresh') }}
          </el-button>
          <el-button 
            @click="clearLogs" 
            type="danger"
            size="small"
            :disabled="displayLogs.length === 0"
          >
            {{ $t('logViewer.clearLogs') }}
          </el-button>
        </div>
      </div>
    </template>
    <div class="log-content text-selectable" ref="logBox">
      <div 
        v-for="(line, index) in displayLogs" 
        :key="index" 
        class="log-line"
      >
        {{ line }}
      </div>
      <el-empty v-if="displayLogs.length === 0" :description="$t('logViewer.noLogs')" />
    </div>
  </el-card>
</template>

<script setup lang="ts">
import { ref, onMounted, onUnmounted, nextTick, computed } from 'vue'
import { ClearLogs, EventsOn, GetConfig, GetLogs } from '../api'
import { useI18n } from 'vue-i18n'

const { t } = useI18n()

// 最大显示的日志条数（前端限制，只保留最近3000条以节省内存）
const MAX_DISPLAY_LOGS = 3000

const allLogs = ref<string[]>([])
const logBox = ref<HTMLElement | null>(null)

let unsubscribeLogLine: (() => void) | null = null
let unsubscribeLogs: (() => void) | null = null
let dragEventHandlers: {
  element: HTMLElement
  preventDrag: (e: DragEvent) => boolean
} | null = null

// 限制显示的日志数量（只显示最近的）
const displayLogs = computed(() => {
  return allLogs.value.slice(-MAX_DISPLAY_LOGS)
})

// 总日志数量（包括未显示的）
const totalLogCount = computed(() => allLogs.value.length)

const realtimeEnabled = ref(true)

const refreshLogs = async () => {
  try {
    const existing = await GetLogs()
    if (Array.isArray(existing)) {
      allLogs.value = existing.slice(-MAX_DISPLAY_LOGS)
    } else {
      allLogs.value = []
    }
    nextTick(() => {
      scrollToBottom()
    })
  } catch {
    // ignore
  }
}

onMounted(async () => {
  // 监听实时日志开关变更
  const toggleHandler = (e: any) => {
    realtimeEnabled.value = !!e.detail
  }
  window.addEventListener('toggle-realtime-logs', toggleHandler)

  // 读取配置：决定是否订阅实时 log-line
  try {
    const cfg = (await GetConfig()) as any
    realtimeEnabled.value = cfg.show_realtime_logs !== false
  } catch {
    realtimeEnabled.value = true
  }

  if (realtimeEnabled.value) {
    // 监听单行日志（实时推送）
  unsubscribeLogLine = await EventsOn('log-line', (line: string) => {
    allLogs.value.push(line)
    
    // 如果日志数量超过限制，删除最旧的
    if (allLogs.value.length > MAX_DISPLAY_LOGS * 2) {
      allLogs.value = allLogs.value.slice(-MAX_DISPLAY_LOGS)
    }
    
    // 如果滚动到底部，自动滚动
    if (logBox.value) {
        const isNearBottom =
          logBox.value.scrollHeight - logBox.value.scrollTop - logBox.value.clientHeight < 100
      if (isNearBottom) {
        nextTick(() => {
          scrollToBottom()
        })
      }
    }
  })

    // 监听全部日志（如果后端有推送）
  unsubscribeLogs = await EventsOn('logs', (data: string[]) => {
    if (Array.isArray(data)) {
      allLogs.value = data.slice(-MAX_DISPLAY_LOGS)
    } else {
      allLogs.value = []
    }
    nextTick(() => {
      scrollToBottom()
    })
  })
  }

  // 初始化拉取一次（关闭实时推送时也可用）
  await refreshLogs()
  
  // 禁止拖动选中的文本
  if (logBox.value) {
    const preventDrag = (e: DragEvent) => {
      e.preventDefault()
      return false
    }
    logBox.value.addEventListener('dragstart', preventDrag)
    logBox.value.addEventListener('drag', preventDrag)
    logBox.value.addEventListener('dragend', preventDrag)
    dragEventHandlers = { element: logBox.value, preventDrag }
  }
})

onUnmounted(() => {
  window.removeEventListener('toggle-realtime-logs', toggleHandler)

  if (unsubscribeLogLine) unsubscribeLogLine()
  if (unsubscribeLogs) unsubscribeLogs()
  
  // 清理拖动事件监听器
  if (dragEventHandlers) {
    dragEventHandlers.element.removeEventListener('dragstart', dragEventHandlers.preventDrag)
    dragEventHandlers.element.removeEventListener('drag', dragEventHandlers.preventDrag)
    dragEventHandlers.element.removeEventListener('dragend', dragEventHandlers.preventDrag)
    dragEventHandlers = null
  }
})

const scrollToBottom = () => {
  if (logBox.value) {
    logBox.value.scrollTop = logBox.value.scrollHeight
  }
}

const clearLogs = () => {
  allLogs.value = []
  ClearLogs()
}
</script>

<style scoped>
.log-page {
  height: 100%;
  display: flex;
  flex-direction: column;
}

.log-page :deep(.el-card__header) {
  border-bottom: 1px solid var(--border);
  padding: 16px 20px;
  flex-shrink: 0;
}

.log-page :deep(.el-card__body) {
  padding: 0;
  flex: 1;
  min-height: 0;
  display: flex;
  flex-direction: column;
}

.log-header {
  display: flex;
  justify-content: space-between;
  align-items: center;
}

.log-title {
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

.log-actions {
  display: flex;
  align-items: center;
  gap: 12px;
}

.log-count {
  font-size: 13px;
  color: var(--text-muted);
}

.log-content {
  flex: 1;
  overflow-y: auto;
  padding: 8px;
  background: var(--input-bg);
  font-family: 'JetBrains Mono', 'Consolas', 'Monaco', monospace;
  font-size: 13px;
  color: var(--text-muted);
  min-height: 0;
  border-radius: var(--radius-md);
  margin: 16px;
  border: 1px solid var(--border);
}

.log-line {
  white-space: pre-wrap;
  word-break: break-all;
  padding: 4px 8px;
  color: var(--text);
  border-radius: 4px;
  transition: background-color 0.2s;
  border-left: 2px solid transparent;
}

.log-line:hover {
  background-color: var(--card-bg);
  border-left-color: rgba(79, 156, 249, 0.7);
}

.text-selectable {
  user-select: text;
}
</style>
