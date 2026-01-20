<template>
  <el-card class="config-card config-page" shadow="hover">
    <template #header>
      <div class="header-row">
        <h3>WebSocket 反向代理配置</h3>
        <div class="header-actions">
          <el-switch v-model="wsProxyEnabled" active-text="启用" inactive-text="禁用" />
        </div>
      </div>
    </template>

    <el-form label-width="140px">
      <div class="rules-section">
        <el-card
          v-for="(rule, ruleIndex) in rules"
          :key="ruleIndex"
          class="rule-card"
          shadow="hover"
        >
          <template #header>
            <div class="rule-header">
              <h4>WS 监听规则 {{ ruleIndex + 1 }}</h4>
              <el-button
                @click="removeRule(ruleIndex)"
                type="danger"
                size="small"
                :disabled="rules.length <= 1"
              >
                删除规则
              </el-button>
            </div>
          </template>

          <el-form-item label="启用">
            <el-switch v-model="rule.enabled" />
          </el-form-item>

          <el-form-item label="监听地址">
            <el-input v-model="rule.listen_addr" placeholder="0.0.0.0:9001" style="width: 260px;" />
          </el-form-item>

          <el-form-item label="启用 TLS（wss）">
            <el-switch v-model="rule.ssl_enable" />
          </el-form-item>

          <template v-if="rule.ssl_enable">
            <el-form-item label="证书文件 (cert)">
              <div class="file-selector">
                <el-input v-model="rule.cert_file" placeholder="ssl/server.crt" readonly />
                <el-button @click="selectCertFile(ruleIndex)" type="primary" :icon="Folder">
                  选择文件
                </el-button>
              </div>
            </el-form-item>
            <el-form-item label="私钥文件 (key)">
              <div class="file-selector">
                <el-input v-model="rule.key_file" placeholder="ssl/server.key" readonly />
                <el-button @click="selectKeyFile(ruleIndex)" type="primary" :icon="Folder">
                  选择文件
                </el-button>
              </div>
            </el-form-item>
          </template>

          <el-card class="routes-card" shadow="never">
            <template #header>
              <div class="route-header">
                <span>WS 路由</span>
              </div>
            </template>

            <el-card
              v-for="(rt, routeIndex) in rule.routes"
              :key="routeIndex"
              class="route-item"
              shadow="never"
            >
              <template #header>
                <div class="route-item-header">
                  <span>路由 {{ routeIndex + 1 }}</span>
                  <el-button
                    @click="removeRoute(ruleIndex, routeIndex)"
                    type="danger"
                    size="small"
                    :disabled="rule.routes.length <= 1"
                  >
                    删除路由
                  </el-button>
                </div>
              </template>

              <el-form-item label="Path 前缀">
                <el-input v-model="rt.path" placeholder="/ws" />
              </el-form-item>

              <el-form-item label="上游地址">
                <el-input v-model="rt.upstream_url" placeholder="ws://127.0.0.1:9000 或 wss://example.com/ws" />
              </el-form-item>
            </el-card>

            <el-button @click="addRoute(ruleIndex)" type="primary" style="margin-top: 10px;">
              <el-icon><Plus /></el-icon> 添加路由
            </el-button>
          </el-card>
        </el-card>

        <el-button @click="addRule" type="primary" style="margin-top: 10px;">
          <el-icon><Plus /></el-icon> 添加 WS 监听规则
        </el-button>
      </div>
    </el-form>
  </el-card>
</template>

<script setup lang="ts">
import { ref, onMounted } from 'vue'
import { GetConfig, OpenCertFileDialog, OpenKeyFileDialog } from '../api'
import { Plus, Folder } from '@element-plus/icons-vue'
import { ElMessage } from 'element-plus'

interface WsRoute {
  path: string
  upstream_url: string
}

interface WsListenRule {
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
    enabled: true,
    listen_addr: '0.0.0.0:9001',
    ssl_enable: false,
    cert_file: '',
    key_file: '',
    routes: [{ path: '/ws', upstream_url: 'ws://127.0.0.1:9000' }],
  })
}

const removeRule = (index: number) => {
  if (rules.value.length <= 1) return
  rules.value.splice(index, 1)
}

const addRoute = (ruleIndex: number) => {
  rules.value[ruleIndex].routes.push({
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
    ElMessage.error(`选择证书文件失败: ${error.message || error}`)
  }
}

const selectKeyFile = async (ruleIndex: number) => {
  try {
    const filePath = await OpenKeyFileDialog()
    if (filePath) {
      rules.value[ruleIndex].key_file = String(filePath)
    }
  } catch (error: any) {
    ElMessage.error(`选择私钥文件失败: ${error.message || error}`)
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
.header-row {
  display: flex;
  align-items: center;
  justify-content: space-between;
  width: 100%;
}

.header-actions {
  display: flex;
  align-items: center;
  justify-content: flex-end;
}

.rules-section {
  display: flex;
  flex-direction: column;
  gap: 18px;
}

.rule-card {
  border-radius: 14px;
}

.rule-header {
  display: flex;
  justify-content: space-between;
  align-items: center;
}

.file-selector {
  display: flex;
  gap: 8px;
  align-items: center;
  width: 100%;
}

.file-selector .el-input {
  flex: 1;
}

.routes-card {
  margin-top: 14px;
  border-radius: 12px;
}

.route-item {
  margin-bottom: 12px;
  border-radius: 12px;
}

.route-item-header {
  display: flex;
  justify-content: space-between;
  align-items: center;
}
</style>
