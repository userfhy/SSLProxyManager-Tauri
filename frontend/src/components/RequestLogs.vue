<!-- frontend/src/components/RequestLogs.vue -->
<template>
  <el-card ref="configCardRef" class="config-page" shadow="never">
    <template #header>
      <h3>{{ $t('requestLogs.title') }}</h3>
    </template>

    <!-- 查询条件 -->
    <el-card class="search-card" shadow="never">
      <el-form :model="searchForm" label-width="120px" :inline="true">
        <el-form-item :label="$t('requestLogs.timeRange')">
          <el-config-provider :locale="zhCn">
            <el-date-picker
              v-model="dateRange"
              type="datetimerange"
              :range-separator="$t('requestLogs.to')"
              :start-placeholder="$t('requestLogs.startTime')"
              :end-placeholder="$t('requestLogs.endTime')"
              format="YYYY-MM-DD HH:mm:ss"
              value-format="x"
              :shortcuts="dateShortcuts"
              style="width: 380px;"
            />
          </el-config-provider>
        </el-form-item>

        <el-form-item :label="$t('requestLogs.listenAddr')">
          <el-select v-model="searchForm.listenAddr" :placeholder="$t('requestLogs.all')" style="width: 200px;" clearable filterable>
            <el-option :label="$t('requestLogs.all')" value="" />
            <el-option v-for="a in listenAddrs" :key="a" :label="a" :value="a" />
          </el-select>
        </el-form-item>

        <el-form-item :label="$t('requestLogs.upstream')">
          <el-input v-model="searchForm.upstream" :placeholder="$t('requestLogs.fuzzyMatch')" style="width: 200px;" clearable />
        </el-form-item>

        <el-form-item :label="$t('requestLogs.requestPath')">
          <el-input v-model="searchForm.requestPath" :placeholder="$t('requestLogs.fuzzyMatch')" style="width: 200px;" clearable />
        </el-form-item>

        <el-form-item :label="$t('requestLogs.clientIP')">
          <el-input v-model="searchForm.clientIP" :placeholder="$t('requestLogs.fuzzyMatch')" style="width: 200px;" clearable />
        </el-form-item>

        <el-form-item :label="$t('requestLogs.statusCode')">
          <el-input-number v-model="searchForm.statusCode" :min="0" :max="999" :placeholder="$t('requestLogs.allStatusCodes')" style="width: 150px;" />
        </el-form-item>

        <el-form-item>
          <el-button type="primary" @click="handleSearch" :loading="loading">{{ $t('requestLogs.search') }}</el-button>
          <el-button @click="handleReset">{{ $t('requestLogs.reset') }}</el-button>
        </el-form-item>
      </el-form>
    </el-card>

    <!-- 数据表格 -->
    <el-table
      :data="logs" 
      v-loading="loading" 
      stripe 
      border 
      style="width: 100%; margin-top: 20px;"
      @sort-change="handleSortChange"
    >
      <el-table-column prop="timestamp" :label="$t('requestLogs.time')" width="180" sortable="custom">
        <template #default="{ row }">
          {{ formatTime(row.timestamp) }}
        </template>
      </el-table-column>
      <el-table-column prop="listenAddr" :label="$t('requestLogs.listenAddr')" width="120" sortable="custom" />
      <el-table-column prop="clientIP" :label="$t('requestLogs.clientIP')" width="130" sortable="custom" />
      <el-table-column prop="remoteIP" :label="$t('requestLogs.remoteIP')" width="130" sortable="custom" />
      <el-table-column prop="method" :label="$t('requestLogs.method')" width="80" sortable="custom" />
      <el-table-column prop="requestPath" :label="$t('requestLogs.requestPath')" min-width="200" show-overflow-tooltip sortable="custom" />
      <el-table-column prop="requestHost" :label="$t('requestLogs.host')" width="150" show-overflow-tooltip sortable="custom" />
      <el-table-column prop="statusCode" :label="$t('requestLogs.statusCode')" width="100" sortable="custom">
        <template #default="{ row }">
          <el-tag :type="getStatusTagType(row.statusCode)" size="small">
            {{ row.statusCode }}
          </el-tag>
        </template>
      </el-table-column>
      <el-table-column prop="upstream" :label="$t('requestLogs.upstream')" width="200" show-overflow-tooltip sortable="custom">
        <template #default="{ row }">
          {{ formatUpstreamHost(row.upstream) }}
        </template>
      </el-table-column>
      <el-table-column prop="latencyMs" :label="$t('requestLogs.latency')" width="100" sortable="custom">
        <template #default="{ row }">
          {{ row.latencyMs.toFixed(2) }}
        </template>
      </el-table-column>
      <el-table-column prop="userAgent" :label="$t('requestLogs.userAgent')" min-width="200" show-overflow-tooltip />
      <el-table-column :label="$t('requestLogs.actions')" width="120" fixed="right">
        <template #default="{ row }">
          <el-button 
            type="danger" 
            size="small" 
            @click="handleBlacklistIP(row.clientIP)"
            :icon="Lock"
          >
            {{ $t('requestLogs.blacklist') }}
          </el-button>
        </template>
      </el-table-column>
    </el-table>

    <!-- 分页 -->
    <div class="pagination">
      <el-config-provider :locale="zhCn">
        <el-pagination
          v-model:current-page="pagination.page"
          v-model:page-size="pagination.pageSize"
          :page-sizes="[10, 20, 50, 100]"
          :total="pagination.total"
          layout="total, sizes, prev, pager, next, jumper"
          @size-change="handleSizeChange"
          @current-change="handlePageChange"
        />
      </el-config-provider>
    </div>
  </el-card>
</template>

<script setup lang="ts">
import { ref, onMounted, onBeforeUnmount, nextTick, watch } from 'vue'
import { ElMessage, ElConfigProvider, ElMessageBox } from 'element-plus'
import { Lock } from '@element-plus/icons-vue'
import zhCn from 'element-plus/dist/locale/zh-cn.mjs'
// @ts-ignore
import { GetListenAddrs, QueryRequestLogs, AddBlacklistEntry } from '../api'
import { useI18n } from 'vue-i18n'
import { useDateShortcuts } from '../composables/useDateShortcuts'

const { t } = useI18n()
const { dateShortcuts } = useDateShortcuts()

interface RequestLog {
  id: number
  timestamp: number
  listenAddr: string
  clientIP: string
  remoteIP: string
  method: string
  requestURL: string
  requestPath: string
  requestHost: string
  statusCode: number
  upstream: string
  routeKey: string
  latencyMs: number
  userAgent: string
  referer: string
}

const dateRange = ref<[number, number] | null>(null)
const listenAddrs = ref<string[]>([])

const searchForm = ref({
  listenAddr: '',
  upstream: '',
  requestPath: '',
  clientIP: '',
  statusCode: 0,
})

const logs = ref<RequestLog[]>([])
const loading = ref(false)
const pagination = ref({
  page: 1,
  pageSize: 20,
  total: 0,
  totalPage: 0,
})
const sortConfig = ref<{ prop?: string; order?: string }>({})
const AUTO_SEARCH_DEBOUNCE_MS = 500
let autoSearchTimer: number | null = null
let searchSeq = 0

const toServerSortBy = (prop?: string): string => {
  switch (prop) {
    case 'timestamp': return 'timestamp'
    case 'listenAddr': return 'listen_addr'
    case 'clientIP': return 'client_ip'
    case 'remoteIP': return 'remote_ip'
    case 'method': return 'method'
    case 'requestPath': return 'request_path'
    case 'requestHost': return 'request_host'
    case 'statusCode': return 'status_code'
    case 'upstream': return 'upstream'
    case 'latencyMs': return 'latency_ms'
    default: return 'timestamp'
  }
}

const formatTime = (timestamp: number) => {
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

// 从完整 URL 中提取域名（不包含协议和路径）
const formatUpstreamHost = (upstream: string): string => {
  if (!upstream) return ''
  try {
    // 处理没有协议的 URL（如直接是 "example.com:8080"）
    const urlStr = upstream.includes('://') ? upstream : `http://${upstream}`
    const url = new URL(urlStr)
    return url.host || url.pathname.split('/')[0] || upstream
  } catch (e) {
    // 解析失败时返回原值
    return upstream
  }
}

const getStatusTagType = (statusCode: number) => {
  if (statusCode >= 200 && statusCode < 300) return 'success'
  if (statusCode >= 300 && statusCode < 400) return 'info'
  if (statusCode >= 400 && statusCode < 500) return 'warning'
  if (statusCode >= 500) return 'danger'
  return ''
}

const handleSearch = async (opts?: { silent?: boolean }) => {
  const silent = !!opts?.silent
  if (!dateRange.value || dateRange.value.length !== 2) {
    if (!silent) ElMessage.warning(t('requestLogs.selectTimeRange'))
    return
  }

  const [startTime, endTime] = dateRange.value
  if (startTime >= endTime) {
    if (!silent) ElMessage.warning(t('requestLogs.startTimeMustBeLess'))
    return
  }

  const reqSeq = ++searchSeq
  loading.value = true
  try {
    const startSec = Math.floor(startTime / 1000)
    const endSec = Math.floor(endTime / 1000)

    // @ts-ignore
    const response = await QueryRequestLogs({
      start_time: startSec,
      end_time: endSec,
      listen_addr: searchForm.value.listenAddr || '',
      upstream: searchForm.value.upstream || '',
      request_path: searchForm.value.requestPath || '',
      client_ip: searchForm.value.clientIP || '',
      status_code: searchForm.value.statusCode || 0,
      page: pagination.value.page,
      page_size: pagination.value.pageSize,
      sort_by: toServerSortBy(sortConfig.value.prop),
      sort_order: sortConfig.value.order || 'descending',
    })

    if (reqSeq !== searchSeq) return

    if (response) {
      const list = Array.isArray(response.logs) ? response.logs : []
      logs.value = list.map((r: any) => ({
        id: r.id,
        timestamp: r.timestamp,
        listenAddr: r.listen_addr ?? r.listenAddr,
        clientIP: r.client_ip ?? r.clientIP,
        remoteIP: r.remote_ip ?? r.remoteIP,
        method: r.method,
        requestURL: r.request_url ?? r.requestURL ?? '',
        requestPath: r.request_path ?? r.requestPath,
        requestHost: r.request_host ?? r.requestHost,
        statusCode: r.status_code ?? r.statusCode,
        upstream: r.upstream,
        routeKey: r.route_key ?? r.routeKey ?? '',
        latencyMs: r.latency_ms ?? r.latencyMs,
        userAgent: r.user_agent ?? r.userAgent,
        referer: r.referer,
      }))
      pagination.value.total = response.total || 0
      pagination.value.totalPage = response.total_page ?? response.totalPage ?? 0
      if (!silent) ElMessage.success(t('requestLogs.searchSuccess', { total: response.total }))
    } else {
      logs.value = []
      pagination.value.total = 0
      if (!silent) ElMessage.warning(t('requestLogs.noDataFound'))
    }
  } catch (error: any) {
    if (reqSeq !== searchSeq) return
    console.error('查询失败:', error)
    if (!silent) ElMessage.error(t('requestLogs.searchFailed', { error: error.message || String(error) }))
    logs.value = []
    pagination.value.total = 0
  } finally {
    if (reqSeq === searchSeq) {
      loading.value = false
    }
  }
}

const scheduleAutoSearch = () => {
  if (autoSearchTimer) {
    clearTimeout(autoSearchTimer)
    autoSearchTimer = null
  }
  autoSearchTimer = window.setTimeout(() => {
    autoSearchTimer = null
    pagination.value.page = 1
    void handleSearch({ silent: true })
  }, AUTO_SEARCH_DEBOUNCE_MS)
}

const handleReset = () => {
  dateRange.value = null
  searchForm.value = {
    listenAddr: '',
    upstream: '',
    requestPath: '',
    clientIP: '',
    statusCode: 0,
  }
  pagination.value.page = 1
  logs.value = []
  pagination.value.total = 0
  searchSeq += 1
}

const handleSizeChange = (size: number) => {
  pagination.value.pageSize = size
  pagination.value.page = 1
  handleSearch()
}

const handlePageChange = (page: number) => {
  pagination.value.page = page
  handleSearch()
}

const handleSortChange = ({ prop, order }: { prop?: string; order?: string }) => {
  sortConfig.value = { prop, order }
  pagination.value.page = 1
  handleSearch()
}

const handleBlacklistIP = async (ip: string) => {
  try {
    await ElMessageBox.confirm(
      t('requestLogs.blacklistConfirm', { ip }),
      t('requestLogs.blacklistTitle'),
      {
        confirmButtonText: t('common.confirm'),
        cancelButtonText: t('common.cancel'),
        type: 'warning',
      }
    )

    // @ts-ignore
    await AddBlacklistEntry(ip, t('requestLogs.blacklistReason'), 0) // 0表示永久
    ElMessage.success(t('requestLogs.blacklistSuccess', { ip }))
  } catch (error: any) {
    if (error !== 'cancel') {
      console.error('拉黑IP失败:', error)
      ElMessage.error(t('requestLogs.blacklistFailed', { error: error.message || String(error) }))
    }
  }
}

const configCardRef = ref<InstanceType<typeof import('element-plus').ElCard> | null>(null)

let dragEventHandlers: {
  element: HTMLElement
  preventDrag: (e: DragEvent) => boolean
} | null = null

onMounted(() => {
  // 默认查询最近1小时的数据
  const endTime = new Date()
  const startTime = new Date(endTime.getTime() - 60 * 60 * 1000)
  dateRange.value = [startTime.getTime(), endTime.getTime()]
  
  // 加载监听地址列表（用于下拉框）
  GetListenAddrs()
    .then((addrs: any) => {
      listenAddrs.value = Array.isArray(addrs) ? addrs : []
    })
    .catch((err: any) => {
      console.error('获取监听地址列表失败:', err)
      listenAddrs.value = []
    })

  // 禁止拖动选中的文本
  nextTick(() => {
    const cardElement = configCardRef.value?.$el as HTMLElement
    if (cardElement) {
      const preventDrag = (e: DragEvent) => {
        e.preventDefault()
        return false
      }
      cardElement.addEventListener('dragstart', preventDrag)
      cardElement.addEventListener('drag', preventDrag)
      cardElement.addEventListener('dragend', preventDrag)
      dragEventHandlers = { element: cardElement, preventDrag }
    }
  })
})

watch(
  [
    () => dateRange.value?.[0],
    () => dateRange.value?.[1],
    () => searchForm.value.listenAddr,
    () => searchForm.value.upstream,
    () => searchForm.value.requestPath,
    () => searchForm.value.clientIP,
    () => searchForm.value.statusCode,
  ],
  () => {
    if (!dateRange.value || dateRange.value.length !== 2) return
    const [startTime, endTime] = dateRange.value
    if (startTime >= endTime) return
    scheduleAutoSearch()
  },
)

onBeforeUnmount(() => {
  if (autoSearchTimer) {
    clearTimeout(autoSearchTimer)
    autoSearchTimer = null
  }
  searchSeq += 1
  // 清理拖动事件监听器
  if (dragEventHandlers) {
    dragEventHandlers.element.removeEventListener('dragstart', dragEventHandlers.preventDrag)
    dragEventHandlers.element.removeEventListener('drag', dragEventHandlers.preventDrag)
    dragEventHandlers.element.removeEventListener('dragend', dragEventHandlers.preventDrag)
    dragEventHandlers = null
  }
})
</script>

<style scoped>
.config-page {
  height: 100%;
  display: flex;
  flex-direction: column;
}

.config-page :deep(.el-card__header) {
  border-bottom: 1px solid var(--border);
  padding: 16px 20px;
  flex-shrink: 0;
}

.config-page :deep(.el-card__body) {
  padding: 20px;
  flex: 1;
  min-height: 0;
  display: flex;
  flex-direction: column;
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

.search-card {
  margin-bottom: 20px;
  border-radius: var(--radius-md);
  background: var(--input-bg);
  border: 1px solid var(--border);
  padding: 4px;
}

.search-card :deep(.el-form) {
  display: grid;
  grid-template-columns: repeat(12, minmax(0, 1fr));
  gap: 12px 14px;
  align-items: end;
}

.search-card :deep(.el-form-item) {
  margin-bottom: 0;
  grid-column: span 3;
}

.search-card :deep(.el-form-item:nth-child(1)) {
  grid-column: span 6;
}

.search-card :deep(.el-form-item:last-child) {
  grid-column: span 12;
}

.search-card :deep(.el-form-item__content) {
  width: 100%;
}

.search-card :deep(.el-input),
.search-card :deep(.el-input-number),
.search-card :deep(.el-select),
.search-card :deep(.el-date-editor) {
  width: 100% !important;
}

.el-table {
  flex: 1;
  min-height: 0;
}

.pagination {
  margin-top: 20px;
  display: flex;
  justify-content: flex-end;
  flex-shrink: 0;
}

@media (max-width: 1200px) {
  .search-card :deep(.el-form-item) {
    grid-column: span 4;
  }
}

@media (max-width: 900px) {
  .search-card :deep(.el-form-item),
  .search-card :deep(.el-form-item:nth-child(1)) {
    grid-column: span 6;
  }
}

@media (max-width: 640px) {
  .search-card :deep(.el-form-item),
  .search-card :deep(.el-form-item:nth-child(1)) {
    grid-column: span 12;
  }

  .pagination {
    justify-content: center;
  }
}
</style>
