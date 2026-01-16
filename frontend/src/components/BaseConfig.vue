<!-- frontend/src/components/BaseConfig.vue -->
<template>
  <el-card class="config-card config-page" shadow="hover">
    <template #header>
      <h3>基础配置</h3>
    </template>
    <el-form label-width="120px">
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

    </el-form>
  </el-card>
</template>

<script setup lang="ts">
import { ref, onMounted, watch } from 'vue'
import { GetConfig } from '../api'

const autoStart = ref(false)
const showRealtimeLogs = ref(true)
const realtimeLogsOnlyErrors = ref(false)
const streamProxy = ref(true)

onMounted(async () => {
  try {
    const configData = (await GetConfig()) as any
    autoStart.value = !!configData.auto_start
    showRealtimeLogs.value = configData.show_realtime_logs !== false
    realtimeLogsOnlyErrors.value = !!configData.realtime_logs_only_errors
    streamProxy.value = configData.stream_proxy !== false
  } catch {
    // ignore
  }
})

watch(showRealtimeLogs, (v) => {
  if (!v) {
    realtimeLogsOnlyErrors.value = false
  }
})

const getConfig = () => {
  return {
    auto_start: !!autoStart.value,
    show_realtime_logs: !!showRealtimeLogs.value,
    realtime_logs_only_errors: !!realtimeLogsOnlyErrors.value,
    stream_proxy: !!streamProxy.value,
  }
}

defineExpose({
  getConfig,
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

.mini-hint {
  display: block;
  margin-top: 6px;
  font-size: 12px;
  color: var(--text-muted);
}
</style>
