<!-- frontend/src/components/BaseConfig.vue -->
<template>
  <el-card class="config-card config-page" shadow="hover">
    <template #header>
      <div class="header-row">
        <h3>基础配置</h3>
        <el-button type="warning" size="small" plain @click="resetToDefaults">恢复默认设置</el-button>
      </div>
    </template>
    <el-form label-width="190px">
      <el-form-item label="开机自启">
        <el-switch v-model="autoStart" />
        <el-text type="info" size="small" class="mini-hint" style="margin-left: 10px;">
          启用后，应用将在您登录系统时自动启动。
        </el-text>
      </el-form-item>

      <el-form-item label="显示实时日志">
        <el-switch v-model="showRealtimeLogs" />
        <el-text type="info" size="small" class="mini-hint" style="margin-left: 10px;">
          关闭后不会实时推送日志到界面（仍会在后台缓存，且可手动查看）。
        </el-text>
      </el-form-item>

      <el-form-item v-if="showRealtimeLogs" label="仅显示错误日志">
        <el-switch v-model="realtimeLogsOnlyErrors" />
        <el-text type="info" size="small" class="mini-hint" style="margin-left: 10px;">
          开启后仅实时推送错误相关日志，降低高并发下的 UI/日志开销。
        </el-text>
      </el-form-item>

        <el-form-item label="代理流式转发">
        <el-switch v-model="streamProxy" active-text="开启" inactive-text="关闭" />
        <el-text type="info" size="small" class="mini-hint" style="margin-left: 10px;">
          关闭后，请求/响应将在内存中整块读取，可能占用更多内存。
        </el-text>
      </el-form-item>

      <el-form-item v-if="!streamProxy" label="最大Body大小(MB)">
        <el-input-number v-model="maxBodySizeMB" :min="1" :max="1024" :step="1" controls-position="right" />
        <el-text type="info" size="small" class="mini-hint" style="margin-left: 10px;">
          仅在关闭流式转发时生效；超过该大小将拒绝读取。
        </el-text>
      </el-form-item>

      <el-form-item v-if="!streamProxy" label="最大响应Body大小(MB)">
        <el-input-number v-model="maxResponseBodySizeMB" :min="1" :max="1024" :step="1" controls-position="right" />
        <el-text type="info" size="small" class="mini-hint" style="margin-left: 10px;">
          仅在关闭流式转发时生效；超过该大小将拒绝读取。
        </el-text>
      </el-form-item>

      <el-form-item label="上游连接超时(ms)">
        <el-input-number v-model="upstreamConnectTimeoutMs" :min="100" :max="600000" :step="100" controls-position="right" />
      </el-form-item>

      <el-form-item label="上游读取超时(ms)">
        <el-input-number v-model="upstreamReadTimeoutMs" :min="100" :max="600000" :step="100" controls-position="right" />
      </el-form-item>

      <el-form-item label="上游连接池最大空闲">
        <el-input-number v-model="upstreamPoolMaxIdle" :min="0" :max="1024" :step="1" controls-position="right" />
      </el-form-item>

      <el-form-item label="HTTP/2">
        <el-switch v-model="enableHttp2" active-text="开启" inactive-text="关闭" />
        <el-text type="info" size="small" class="mini-hint" style="margin-left: 10px;">
          关闭后，上游请求将强制使用 HTTP/1.1。
        </el-text>
      </el-form-item>

      <el-form-item label="上游空闲连接超时(秒)">
        <el-input-number v-model="upstreamPoolIdleTimeoutSec" :min="0" :max="3600" :step="1" controls-position="right" />
      </el-form-item>

      <el-divider />

      <el-form-item label="响应压缩">
        <el-switch v-model="compressionEnabled" active-text="开启" inactive-text="关闭" />
        <el-text type="info" size="small" class="mini-hint" style="margin-left: 10px;">
          启用后，将对符合条件的响应进行压缩，减少传输数据量。
        </el-text>
      </el-form-item>

      <template v-if="compressionEnabled">
        <el-form-item label="Gzip 压缩">
          <el-switch v-model="compressionGzip" active-text="开启" inactive-text="关闭" />
          <el-text type="info" size="small" class="mini-hint" style="margin-left: 10px;">
            启用 Gzip 压缩算法。
          </el-text>
        </el-form-item>

        <el-form-item v-if="compressionGzip" label="Gzip 压缩等级">
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
            等级越高压缩率越大，但 CPU 消耗也越大。推荐值：6（平衡）。
          </el-text>
        </el-form-item>

        <el-form-item label="Brotli 压缩">
          <el-switch v-model="compressionBrotli" active-text="开启" inactive-text="关闭" />
          <el-text type="info" size="small" class="mini-hint" style="margin-left: 10px;">
            启用 Brotli 压缩算法（压缩率更高，但 CPU 消耗更大）。
          </el-text>
        </el-form-item>

        <el-form-item v-if="compressionBrotli" label="Brotli 压缩等级">
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
            等级越高压缩率越大，但 CPU 消耗也越大。推荐值：6（平衡）。
          </el-text>
        </el-form-item>
      </template>

    </el-form>
  </el-card>
</template>

<script setup lang="ts">
import { ref, onMounted, watch } from 'vue'
import { ElMessageBox, ElMessage } from 'element-plus'
import { GetConfig } from '../api'

const resetToDefaults = async () => {
  try {
    await ElMessageBox.confirm(
      '确定要恢复基础配置默认值吗？\n\n此操作只会重置当前页面的输入项，不会立即保存。',
      '恢复默认设置',
      {
        confirmButtonText: '恢复',
        cancelButtonText: '取消',
        type: 'warning',
      }
    )

    // 基础设置
    autoStart.value = false
    showRealtimeLogs.value = true
    realtimeLogsOnlyErrors.value = false

    // 代理设置
    streamProxy.value = true
    enableHttp2.value = DEFAULT_ENABLE_HTTP2

    // 请求体大小限制
    maxBodySizeMB.value = DEFAULT_MAX_BODY_SIZE_MB
    maxResponseBodySizeMB.value = DEFAULT_MAX_RESPONSE_BODY_SIZE_MB

    // 上游连接设置
    upstreamConnectTimeoutMs.value = DEFAULT_CONNECT_TIMEOUT_MS
    upstreamReadTimeoutMs.value = DEFAULT_READ_TIMEOUT_MS
    upstreamPoolMaxIdle.value = DEFAULT_POOL_MAX_IDLE
    upstreamPoolIdleTimeoutSec.value = DEFAULT_POOL_IDLE_TIMEOUT_SEC

    // 压缩设置
    compressionEnabled.value = DEFAULT_COMPRESSION_ENABLED
    compressionGzip.value = DEFAULT_COMPRESSION_GZIP
    compressionBrotli.value = DEFAULT_COMPRESSION_BROTLI
    compressionGzipLevel.value = DEFAULT_COMPRESSION_GZIP_LEVEL
    compressionBrotliLevel.value = DEFAULT_COMPRESSION_BROTLI_LEVEL

    // 触发实时日志事件
    window.dispatchEvent(new CustomEvent('toggle-realtime-logs', { detail: showRealtimeLogs.value }))

    ElMessage.success('已恢复所有默认值（未保存）')
  } catch {
    // 用户取消
  }
}

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
  } catch {
    // ignore
  }
})

watch(showRealtimeLogs, (v) => {
  if (!v) {
    realtimeLogsOnlyErrors.value = false
  }
  window.dispatchEvent(new CustomEvent('toggle-realtime-logs', { detail: v }))
})

const getConfig = () => {
  return {
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
  }
}

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

.header-row {
  display: flex;
  align-items: center;
  justify-content: space-between;
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

:deep(.el-form-item__content) {
  display: flex;
  align-items: center;
  gap: 12px;
}

.mini-hint {
  font-size: 12px;
  color: var(--text-muted);
  line-height: 1.4;
}
</style>
