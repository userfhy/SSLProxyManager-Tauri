<!-- frontend/src/components/RequestLogs.vue -->
<template>
  <el-card ref="configCardRef" class="config-card config-page" shadow="hover">
    <template #header>
      <h3>请求记录查询</h3>
    </template>

    <!-- 查询条件 -->
    <el-card class="search-card" shadow="never">
      <el-form :model="searchForm" label-width="120px" :inline="true">
        <el-form-item label="时间范围">
          <el-config-provider :locale="zhCn">
            <el-date-picker
              v-model="dateRange"
              type="datetimerange"
              range-separator="至"
              start-placeholder="开始时间"
              end-placeholder="结束时间"
              format="YYYY-MM-DD HH:mm:ss"
              value-format="x"
              :shortcuts="dateShortcuts"
              style="width: 380px;"
            />
          </el-config-provider>
        </el-form-item>

        <el-form-item label="监听地址">
          <el-select v-model="searchForm.listenAddr" placeholder="全部" style="width: 200px;" clearable filterable>
            <el-option label="全部" value="" />
            <el-option v-for="a in listenAddrs" :key="a" :label="a" :value="a" />
          </el-select>
        </el-form-item>

        <el-form-item label="上游地址">
          <el-input v-model="searchForm.upstream" placeholder="模糊匹配" style="width: 200px;" clearable />
        </el-form-item>

        <el-form-item label="请求路径">
          <el-input v-model="searchForm.requestPath" placeholder="模糊匹配" style="width: 200px;" clearable />
        </el-form-item>

        <el-form-item label="客户端IP">
          <el-input v-model="searchForm.clientIP" placeholder="模糊匹配" style="width: 200px;" clearable />
        </el-form-item>

        <el-form-item label="状态码">
          <el-input-number v-model="searchForm.statusCode" :min="0" :max="999" placeholder="0表示全部" style="width: 150px;" />
        </el-form-item>

        <el-form-item>
          <el-button type="primary" @click="handleSearch" :loading="loading">查询</el-button>
          <el-button @click="handleReset">重置</el-button>
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
      <el-table-column prop="timestamp" label="时间" width="180" sortable="custom">
        <template #default="{ row }">
          {{ formatTime(row.timestamp) }}
        </template>
      </el-table-column>
      <el-table-column prop="listenAddr" label="监听地址" width="120" sortable="custom" />
      <el-table-column prop="clientIP" label="客户端IP" width="130" sortable="custom" />
      <el-table-column prop="remoteIP" label="RemoteIP" width="130" sortable="custom" />
      <el-table-column prop="method" label="方法" width="80" sortable="custom" />
      <el-table-column prop="requestPath" label="请求路径" min-width="200" show-overflow-tooltip sortable="custom" />
      <el-table-column prop="requestHost" label="Host" width="150" show-overflow-tooltip sortable="custom" />
      <el-table-column prop="statusCode" label="状态码" width="100" sortable="custom">
        <template #default="{ row }">
          <el-tag :type="getStatusTagType(row.statusCode)" size="small">
            {{ row.statusCode }}
          </el-tag>
        </template>
      </el-table-column>
      <el-table-column prop="upstream" label="上游地址" width="200" show-overflow-tooltip sortable="custom">
        <template #default="{ row }">
          {{ formatUpstreamHost(row.upstream) }}
        </template>
      </el-table-column>
      <el-table-column prop="latencyMs" label="延迟(ms)" width="100" sortable="custom">
        <template #default="{ row }">
          {{ row.latencyMs.toFixed(2) }}
        </template>
      </el-table-column>
      <el-table-column prop="userAgent" label="User-Agent" min-width="200" show-overflow-tooltip />
      <el-table-column label="操作" width="120" fixed="right">
        <template #default="{ row }">
          <el-button 
            type="danger" 
            size="small" 
            @click="handleBlacklistIP(row.clientIP)"
            :icon="Lock"
          >
            拉黑
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
import { ref, onMounted, onBeforeUnmount, nextTick } from 'vue'
import { ElMessage, ElConfigProvider, ElMessageBox } from 'element-plus'
import { Lock } from '@element-plus/icons-vue'
import zhCn from 'element-plus/dist/locale/zh-cn.mjs'
// @ts-ignore
import { GetListenAddrs, QueryRequestLogs, AddBlacklistEntry } from '../api'

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

// 日期快捷选项
const dateShortcuts = [
  {
    text: '今天',
    value: () => {
      const now = new Date()
      const start = new Date(now.getFullYear(), now.getMonth(), now.getDate(), 0, 0, 0)
      const end = new Date(now.getFullYear(), now.getMonth(), now.getDate(), 23, 59, 59)
      return [start.getTime(), end.getTime()]
    }
  },
  {
    text: '昨天',
    value: () => {
      const now = new Date()
      const yesterday = new Date(now)
      yesterday.setDate(yesterday.getDate() - 1)
      const start = new Date(yesterday.getFullYear(), yesterday.getMonth(), yesterday.getDate(), 0, 0, 0)
      const end = new Date(yesterday.getFullYear(), yesterday.getMonth(), yesterday.getDate(), 23, 59, 59)
      return [start.getTime(), end.getTime()]
    }
  },
  {
    text: '最近7天',
    value: () => {
      const now = new Date()
      const start = new Date(now)
      start.setDate(start.getDate() - 6)
      start.setHours(0, 0, 0, 0)
      const end = new Date(now)
      end.setHours(23, 59, 59, 999)
      return [start.getTime(), end.getTime()]
    }
  },
  {
    text: '最近30天',
    value: () => {
      const now = new Date()
      const start = new Date(now)
      start.setDate(start.getDate() - 29)
      start.setHours(0, 0, 0, 0)
      const end = new Date(now)
      end.setHours(23, 59, 59, 999)
      return [start.getTime(), end.getTime()]
    }
  },
  {
    text: '本月',
    value: () => {
      const now = new Date()
      const start = new Date(now.getFullYear(), now.getMonth(), 1, 0, 0, 0)
      const end = new Date(now.getFullYear(), now.getMonth() + 1, 0, 23, 59, 59)
      return [start.getTime(), end.getTime()]
    }
  },
  {
    text: '上一月',
    value: () => {
      const now = new Date()
      const start = new Date(now.getFullYear(), now.getMonth() - 1, 1, 0, 0, 0)
      const end = new Date(now.getFullYear(), now.getMonth(), 0, 23, 59, 59)
      return [start.getTime(), end.getTime()]
    }
  },
  {
    text: '最近半年',
    value: () => {
      const now = new Date()
      const start = new Date(now)
      start.setMonth(start.getMonth() - 6)
      start.setDate(1)
      start.setHours(0, 0, 0, 0)
      const end = new Date(now)
      end.setHours(23, 59, 59, 999)
      return [start.getTime(), end.getTime()]
    }
  },
  {
    text: '去年',
    value: () => {
      const now = new Date()
      const start = new Date(now.getFullYear() - 1, 0, 1, 0, 0, 0)
      const end = new Date(now.getFullYear() - 1, 11, 31, 23, 59, 59)
      return [start.getTime(), end.getTime()]
    }
  },
  {
    text: '今年',
    value: () => {
      const now = new Date()
      const start = new Date(now.getFullYear(), 0, 1, 0, 0, 0)
      const end = new Date(now.getFullYear(), 11, 31, 23, 59, 59)
      return [start.getTime(), end.getTime()]
    }
  }
]

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

const handleSearch = async () => {
  if (!dateRange.value || dateRange.value.length !== 2) {
    ElMessage.warning('请选择时间范围')
    return
  }

  const [startTime, endTime] = dateRange.value
  if (startTime >= endTime) {
    ElMessage.warning('开始时间必须小于结束时间')
    return
  }

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
    })

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
      ElMessage.success(`查询成功，共 ${response.total} 条记录`)
    } else {
      logs.value = []
      pagination.value.total = 0
      ElMessage.warning('未找到数据')
    }
  } catch (error: any) {
    console.error('查询失败:', error)
    ElMessage.error('查询失败: ' + (error.message || String(error)))
    logs.value = []
    pagination.value.total = 0
  } finally {
    loading.value = false
  }
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
  // 前端排序
  if (prop && order) {
    const sortedLogs = [...logs.value]
    sortedLogs.sort((a: any, b: any) => {
      let aVal = a[prop]
      let bVal = b[prop]
      
      // 处理数字类型
      if (prop === 'timestamp' || prop === 'statusCode' || prop === 'latencyMs') {
        aVal = Number(aVal)
        bVal = Number(bVal)
      } else {
        // 字符串类型
        aVal = String(aVal || '')
        bVal = String(bVal || '')
      }
      
      if (order === 'ascending') {
        return aVal > bVal ? 1 : aVal < bVal ? -1 : 0
      } else {
        return aVal < bVal ? 1 : aVal > bVal ? -1 : 0
      }
    })
    logs.value = sortedLogs
  } else {
    // 恢复原始顺序，重新查询
    handleSearch()
  }
}

const handleBlacklistIP = async (ip: string) => {
  try {
    await ElMessageBox.confirm(
      `确定要拉黑IP "${ip}" 吗？\n将使用默认设置：永久拉黑`,
      '拉黑IP',
      {
        confirmButtonText: '确定',
        cancelButtonText: '取消',
        type: 'warning',
      }
    )

    // @ts-ignore
    await AddBlacklistEntry(ip, '从请求记录中拉黑', 0) // 0表示永久
    ElMessage.success(`IP ${ip} 已添加到黑名单（永久）`)
  } catch (error: any) {
    if (error !== 'cancel') {
      console.error('拉黑IP失败:', error)
      ElMessage.error('拉黑IP失败: ' + (error.message || String(error)))
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

onBeforeUnmount(() => {
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
.config-card {
  height: 100%;
  overflow-y: auto;
  border-radius: 20px;
  backdrop-filter: blur(10px);
}

.config-card :deep(.el-card__header) {
  border-bottom: 1px solid var(--border);
  padding: 20px;
}

.config-card :deep(.el-card__body) {
  padding: 24px;
}

.config-card h3 {
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
  border-radius: 14px;
}

.pagination {
  margin-top: 20px;
  display: flex;
  justify-content: flex-end;
}
</style>
