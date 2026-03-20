<template>
  <el-card class="config-card config-page" shadow="hover">
    <template #header>
      <div class="header-row">
        <h3>{{ $t('wsProxy.title') }}</h3>
        <div class="header-actions">
          <el-switch v-model="wsProxyEnabled" :active-text="$t('wsProxy.enable')" :inactive-text="$t('wsProxy.disable')" />
        </div>
      </div>
    </template>

    <el-form label-width="140px">
      <TransitionGroup name="list" tag="div" class="rules-section">
        <el-card
          v-for="(rule, ruleIndex) in rules"
          :key="rule.id || ruleIndex"
          class="rule-card"
          shadow="hover"
        >
          <template #header>
            <div class="rule-header">
              <h4>{{ $t('wsProxy.wsListenRule') }} {{ ruleIndex + 1 }}</h4>
              <el-button
                @click="removeRule(ruleIndex)"
                type="danger"
                size="small"
                :disabled="rules.length <= 1"
              >
                {{ $t('wsProxy.deleteRule') }}
              </el-button>
            </div>
          </template>

          <el-form-item :label="$t('wsProxy.enabled')">
            <el-switch v-model="rule.enabled" />
          </el-form-item>

          <el-form-item :label="$t('wsProxy.listenAddr')">
            <el-input v-model="rule.listen_addr" placeholder="0.0.0.0:9001" style="width: 260px;" />
          </el-form-item>

          <el-form-item :label="$t('wsProxy.enableTLS')">
            <el-switch v-model="rule.ssl_enable" />
          </el-form-item>

          <template v-if="rule.ssl_enable">
            <el-form-item :label="$t('wsProxy.certFile')">
              <div class="file-selector">
                <el-input v-model="rule.cert_file" placeholder="ssl/server.crt" readonly />
                <el-button @click="selectCertFile(ruleIndex)" type="primary" :icon="Folder">
                  {{ $t('wsProxy.selectFile') }}
                </el-button>
              </div>
            </el-form-item>
            <el-form-item :label="$t('wsProxy.keyFile')">
              <div class="file-selector">
                <el-input v-model="rule.key_file" placeholder="ssl/server.key" readonly />
                <el-button @click="selectKeyFile(ruleIndex)" type="primary" :icon="Folder">
                  {{ $t('wsProxy.selectFile') }}
                </el-button>
              </div>
            </el-form-item>
          </template>

          <el-card class="routes-card" shadow="never">
            <template #header>
              <div class="route-header">
                <span>{{ $t('wsProxy.wsRoutes') }}</span>
              </div>
            </template>

            <TransitionGroup name="list" tag="div">
              <div
                v-for="(rt, routeIndex) in rule.routes"
                :key="rt.id || routeIndex"
                class="route-item"
              >
                <div class="route-item-header">
                  <span>{{ $t('wsProxy.route') }} {{ routeIndex + 1 }}</span>
                  <el-button
                    @click="removeRoute(ruleIndex, routeIndex)"
                    type="danger"
                    size="small"
                    :disabled="rule.routes.length <= 1"
                  >
                    {{ $t('wsProxy.deleteRoute') }}
                  </el-button>
                </div>

                <el-form-item :label="$t('wsProxy.pathPrefix')">
                  <el-input v-model="rt.path" placeholder="/ws" />
                </el-form-item>

                <el-form-item :label="$t('wsProxy.upstreamUrl')">
                  <el-input v-model="rt.upstream_url" placeholder="ws://127.0.0.1:9000 或 wss://example.com/ws" />
                </el-form-item>
              </div>
            </TransitionGroup>

            <el-button @click="addRoute(ruleIndex)" type="primary" style="margin-top: 10px;">
              <el-icon><Plus /></el-icon> {{ $t('wsProxy.addRoute') }}
            </el-button>
          </el-card>
        </el-card>
      </TransitionGroup>

        <el-button @click="addRule" type="primary" style="margin-top: 10px;">
          <el-icon><Plus /></el-icon> {{ $t('wsProxy.addListenRule') }}
        </el-button>
    </el-form>
  </el-card>
</template>

<script setup lang="ts">
import { ref, onMounted } from 'vue'
import { GetConfig, OpenCertFileDialog, OpenKeyFileDialog } from '../api'
import { Plus, Folder } from '@element-plus/icons-vue'
import { ElMessage } from 'element-plus'
import { useI18n } from 'vue-i18n'

const { t } = useI18n()

interface WsRoute {
  id?: string
  path: string
  upstream_url: string
}

interface WsListenRule {
  id?: string
  enabled: boolean
  listen_addr: string
  ssl_enable: boolean
  cert_file: string
  key_file: string
  routes: WsRoute[]
}

const wsProxyEnabled = ref(true)

const rules = ref<WsListenRule[]>([
  {
    enabled: false,
    listen_addr: '0.0.0.0:9001',
    ssl_enable: false,
    cert_file: '',
    key_file: '',
    routes: [{ path: '/ws', upstream_url: 'ws://127.0.0.1:9000' }],
  },
])

onMounted(async () => {
  try {
    const cfg: any = await GetConfig()
    wsProxyEnabled.value = cfg?.ws_proxy_enabled !== false

    const ws = cfg?.ws_proxy
    if (Array.isArray(ws) && ws.length > 0) {
      rules.value = ws.map((r: any) => ({
        enabled: !!r.enabled,
        listen_addr: r.listen_addr || '0.0.0.0:9001',
        ssl_enable: !!r.ssl_enable,
        cert_file: r.cert_file || '',
        key_file: r.key_file || '',
        routes: Array.isArray(r.routes) && r.routes.length > 0
          ? r.routes.map((rt: any) => ({
              path: rt.path || '/',
              upstream_url: rt.upstream_url || '',
            }))
          : [{ path: '/ws', upstream_url: '' }],
      }))
    }
  } catch {
    // ignore
  }
})

const addRule = () => {
  rules.value.push({
    id: `new-rule-${Date.now()}`,
    enabled: true,
    listen_addr: '0.0.0.0:9001',
    ssl_enable: false,
    cert_file: '',
    key_file: '',
    routes: [{ id: `new-route-${Date.now()}`, path: '/ws', upstream_url: 'ws://127.0.0.1:9000' }],
  })
}

const removeRule = (index: number) => {
  if (rules.value.length <= 1) return
  rules.value.splice(index, 1)
}

const addRoute = (ruleIndex: number) => {
  rules.value[ruleIndex].routes.push({
    id: `new-route-${Date.now()}`,
    path: '/ws',
    upstream_url: '',
  })
}

const removeRoute = (ruleIndex: number, routeIndex: number) => {
  const list = rules.value[ruleIndex].routes
  if (list.length <= 1) return
  list.splice(routeIndex, 1)
}

const selectCertFile = async (ruleIndex: number) => {
  try {
    const filePath = await OpenCertFileDialog()
    if (filePath) {
      rules.value[ruleIndex].cert_file = String(filePath)
    }
  } catch (error: any) {
    ElMessage.error(t('wsProxy.selectCertFileFailed', { error: error.message || error }))
  }
}

const selectKeyFile = async (ruleIndex: number) => {
  try {
    const filePath = await OpenKeyFileDialog()
    if (filePath) {
      rules.value[ruleIndex].key_file = String(filePath)
    }
  } catch (error: any) {
    ElMessage.error(t('wsProxy.selectKeyFileFailed', { error: error.message || error }))
  }
}

const normalizePath = (p: string) => {
  const v = (p || '').trim()
  if (!v) return '/'
  return v.startsWith('/') ? v : '/' + v
}

const getConfig = () => {
  const cleaned = rules.value.map((r) => ({
    enabled: !!r.enabled,
    listen_addr: (r.listen_addr || '').trim(),
    ssl_enable: !!r.ssl_enable,
    cert_file: r.cert_file || '',
    key_file: r.key_file || '',
    routes: (r.routes || []).map((rt) => ({
      path: normalizePath(rt.path),
      upstream_url: (rt.upstream_url || '').trim(),
    })),
  }))

  for (let i = 0; i < cleaned.length; i++) {
    const r = cleaned[i]
    if (!r.listen_addr) {
      throw new Error(`WS 规则 ${i + 1}：监听地址不能为空`)
    }
    if (r.ssl_enable && (!r.cert_file || !r.key_file)) {
      throw new Error(`WS 规则 ${i + 1}：启用 TLS 时证书/私钥不能为空`)
    }
    if (!r.routes || r.routes.length === 0) {
      throw new Error(`WS 规则 ${i + 1}：请至少配置一个 WS 路由`)
    }
    for (let j = 0; j < r.routes.length; j++) {
      const rt = r.routes[j]
      if (!rt.path) {
        throw new Error(`WS 规则 ${i + 1} / 路由 ${j + 1}：Path 不能为空`)
      }
      if (!rt.upstream_url) {
        throw new Error(`WS 规则 ${i + 1} / 路由 ${j + 1}：上游地址不能为空`)
      }
    }
  }

  return {
    ws_proxy_enabled: !!wsProxyEnabled.value,
    ws_proxy: cleaned,
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
  padding: 20px;
}

.header-row {
  display: flex;
  align-items: center;
  justify-content: space-between;
  width: 100%;
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

.rules-section {
  display: flex;
  flex-direction: column;
  gap: 24px;
}

.rule-card {
  border-radius: var(--radius-lg);
  border: 1px solid var(--border);
  background: var(--card-bg);
}

.rule-header {
  display: flex;
  justify-content: space-between;
  align-items: center;
}

.rule-header h4 {
  margin: 0;
  font-size: 18px;
  font-weight: 600;
  color: var(--text);
}

.routes-card {
  margin-top: 14px;
  border-radius: var(--radius-md);
  background: var(--input-bg);
  border: 1px solid var(--border);
}

.route-item {
  margin-bottom: 12px;
  border-radius: var(--radius-sm);
  border: 1px solid var(--border);
  padding: 16px;
  background: var(--card-bg);
}

.route-item-header {
  display: flex;
  justify-content: space-between;
  align-items: center;
  margin-bottom: 16px;
  padding-bottom: 12px;
  border-bottom: 1px solid var(--border);
}

.route-item-header span {
  font-size: 16px;
  font-weight: 600;
  color: var(--text);
}

.file-selector {
  display: flex;
  gap: 8px;
  align-items: center;
  width: 100%;
}

@media (max-width: 980px) {
  .config-page :deep(.el-form-item__label) {
    width: 120px !important;
  }

  .file-selector {
    flex-direction: column;
    align-items: stretch;
  }
}

/* Transition styles */
.list-enter-active,
.list-leave-active {
  transition: all 0.5s cubic-bezier(0.55, 0, 0.1, 1);
}
.list-enter-from,
.list-leave-to {
  opacity: 0;
  transform: scaleY(0.01) translate(30px, 0);
}

.list-leave-active {
  position: absolute;
}
</style>
