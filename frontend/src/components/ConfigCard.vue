<!-- frontend/src/components/ConfigCard.vue -->
<template>
  <el-card class="config-card config-page" shadow="hover">
    <template #header>
      <h3>代理配置</h3>
    </template>

    <!-- 规则配置 -->
    <div class="rules-section">
      <el-card 
        v-for="(rule, ruleIndex) in rules" 
        :key="ruleIndex" 
        class="rule-card"
        shadow="hover"
      >
        <template #header>
          <div class="rule-header">
            <h4>规则 {{ ruleIndex + 1 }}</h4>
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

        <el-form :model="rule" label-width="120px" class="form-grid">
          <el-form-item label="监听地址">
            <el-input v-model="rule.ListenAddr" placeholder="0.0.0.0:8888" style="width: 260px;"/>
          </el-form-item>

          <el-form-item label="路由规则" required>
            <div class="routes-section">
              <el-card 
                v-for="(rt, routeIndex) in rule.Routes" 
                :key="routeIndex"
                class="route-card"
                shadow="never"
              >
                <template #header>
                  <div class="route-header">
                    <div class="route-title">路由 {{ routeIndex + 1 }}</div>
                    <el-button
                      @click="removeRoute(ruleIndex, routeIndex)"
                      type="danger"
                      size="small"
                      :disabled="rule.Routes.length <= 1"
                    >
                      删除路由
                    </el-button>
                  </div>
                </template>

                <el-form :model="rt" label-width="160px" class="route-form">
                  <el-row :gutter="20" class="route-match">
                    <el-col :span="10">
                      <el-form-item label="Host（可选）">
                        <el-input v-model="rt.Host" placeholder="api.example.com（留空表示任意）" />
                      </el-form-item>
                    </el-col>
                    <el-col :span="10">
                      <el-form-item label="Path 前缀（location）">
                        <el-input v-model="rt.Path" placeholder="/ 或 /api" />
                      </el-form-item>
                    </el-col>
                  </el-row>

                  <el-row :gutter="22">
                    <el-col :span="10">
                      <el-form-item label="proxy_pass_path（可选）">
                        <el-input v-model="rt.ProxyPassPath" placeholder="/v1 （留空表示不重写）" />
                        <el-text type="info" size="small" class="mini-hint">
                          等价 nginx: proxy_pass http://upstream&lt;这里&gt;;
                        </el-text>
                      </el-form-item>
                    </el-col>
                    <el-col :span="10">
                      <el-form-item label="静态文件目录（可选）">
                        <div class="file-selector">
                          <el-input v-model="rt.StaticDir" placeholder="./frontend/dist 或绝对路径" readonly />
                        </div>
                        <el-text type="info" size="small" class="mini-hint">
                          优先提供静态文件，不存在时回退到上游服务器
                        </el-text>
                      </el-form-item>
                    </el-col>
                    <el-col :span="2">
                      <el-button @click="selectDirectory(ruleIndex, routeIndex)" size="small" type="primary" :icon="Folder">
                            选择目录
                          </el-button>
                    </el-col>
                  </el-row>

                  <el-row :gutter="20">
                    <el-col :span="10">
                      <el-form-item>
                        <el-checkbox v-model="rt.ExcludeBasicAuth">
                          排除 Basic Auth 验证
                        </el-checkbox>
                        <el-text type="info" size="small" class="mini-hint">
                          勾选后，此路由将跳过 Basic Auth 验证
                        </el-text>
                      </el-form-item>
                    </el-col>
                  </el-row>
                </el-form>

                <el-card class="headers-section" shadow="never">
                  <template #header>
                    <div class="headers-title">proxy_set_header（可选）</div>
                  </template>
                  <el-text type="info" size="small" class="headers-hint">
                    支持变量：$remote_addr / $proxy_add_x_forwarded_for / $scheme
                  </el-text>
                  <div class="headers-actions">
                    <el-button @click="applyCommonHeaders(rt)" type="primary" size="small">
                      <el-icon><MagicStick /></el-icon> 快速应用常用 Nginx Headers
                    </el-button>
                  </div>
                  <div v-for="(kv, hIndex) in (rt.SetHeadersList || [])" :key="hIndex" class="header-item">
                    <el-input v-model="kv.Key" placeholder="Header-Key (如 Host)" class="header-key" />
                    <el-input v-model="kv.Value" placeholder="Header-Value (如 $remote_addr)" class="header-value" />
                    <el-button @click="(rt.SetHeadersList || []).splice(hIndex, 1)" type="danger" size="small">删除</el-button>
                  </div>
                  <el-button @click="(rt.SetHeadersList ||= []).push({ Key: '', Value: '' })" type="primary" size="small">
                    <el-icon><Plus /></el-icon> 添加自定义 Header
                  </el-button>
                </el-card>

                <el-card class="upstreams-section" shadow="never">
                  <template #header>
                    <span>上游服务器</span>
                  </template>
                  <div v-for="(upstream, index) in rt.Upstreams" :key="index" class="upstream-item">
                    <el-row :gutter="10" class="upstream-inputs">
                      <el-col :span="12">
                        <el-input
                          v-model="upstream.URL"
                          :placeholder="index === 0 ? 'http://127.0.0.1:8080' : 'https://example.com'"
                          class="upstream-url"
                        />
                      </el-col>
                      <el-col :span="4">
                        <el-input-number
                          v-model="upstream.Weight"
                          :min="1"
                          placeholder="权重"
                          class="upstream-weight"
                        />
                      </el-col>
                      <el-col :span="2">
                        <el-button
                          @click="removeUpstream(ruleIndex, routeIndex, index)"
                          type="danger"
                          size="small"
                          :disabled="rt.Upstreams.length <= 1"
                        >
                          删除
                        </el-button>
                      </el-col>
                    </el-row>
                  </div>
                  <el-button @click="addUpstream(ruleIndex, routeIndex)" type="primary">
                    <el-icon><Plus /></el-icon> 添加新的上游服务器
                  </el-button>
                </el-card>
              </el-card>

              <el-button @click="addRoute(ruleIndex)" type="primary" style="margin-top: 10px;">
                <el-icon><Plus /></el-icon> 添加新的路由规则
              </el-button>
            </div>
          </el-form-item>

          <el-form-item>
            <el-checkbox v-model="rule.SSLEnable">该规则启用 SSL/HTTPS</el-checkbox>
          </el-form-item>

          <template v-if="rule.SSLEnable">
            <el-form-item label="证书文件 (cert)">
              <div class="file-selector">
                <el-input v-model="rule.CertFile" placeholder="ssl/server.crt" readonly />
                <el-button @click="selectCertFile(ruleIndex)" type="primary" :icon="Folder">
                  选择文件
                </el-button>
              </div>
            </el-form-item>
            <el-form-item label="私钥文件 (key)">
              <div class="file-selector">
                <el-input v-model="rule.KeyFile" placeholder="ssl/server.key" readonly />
                <el-button @click="selectKeyFile(ruleIndex)" type="primary" :icon="Folder">
                  选择文件
                </el-button>
              </div>
            </el-form-item>
          </template>

          <el-form-item>
            <el-checkbox v-model="rule.BasicAuthEnable">启用 Basic Auth 认证</el-checkbox>
          </el-form-item>

          <template v-if="rule.BasicAuthEnable">
            <el-form-item label="用户名">
              <el-input v-model="rule.BasicAuthUsername" placeholder="admin" />
            </el-form-item>
            <el-form-item label="密码">
              <el-input v-model="rule.BasicAuthPassword" type="password" placeholder="password" show-password />
            </el-form-item>
            <el-form-item>
              <el-checkbox v-model="rule.BasicAuthForwardHeader">
                将 Basic Auth 头转发到上游服务器
              </el-checkbox>
              <el-text type="info" size="small" class="mini-hint">
                默认不转发，避免影响后端 API 的认证（如 JWT、OAuth 等）
              </el-text>
            </el-form-item>
          </template>
        </el-form>
      </el-card>

      <el-button @click="addRule" type="primary" style="margin-top: 10px;">
        <el-icon><Plus /></el-icon> 添加新的监听规则
      </el-button>
    </div>
  </el-card>
</template>

<script setup lang="ts">
import { ref, onMounted } from 'vue'
import { GetConfig, OpenCertFileDialog, OpenKeyFileDialog, OpenDirectoryDialog } from '../api'
import { Plus, MagicStick, Folder } from '@element-plus/icons-vue'
import { ElMessage } from 'element-plus'

interface Upstream {
  URL: string
  Weight: number
}

interface HeaderKV {
  Key: string
  Value: string
}

interface Route {
  ID?: string
  Host: string
  Path: string

  ProxyPassPath?: string
  SetHeaders?: Record<string, string>
  SetHeadersList?: HeaderKV[]

  StaticDir?: string
  ExcludeBasicAuth?: boolean

  Upstreams: Upstream[]
}

interface ListenRule {
  ID?: string
  ListenAddr: string
  SSLEnable: boolean
  CertFile: string
  KeyFile: string
  BasicAuthEnable?: boolean
  BasicAuthUsername?: string
  BasicAuthPassword?: string
  BasicAuthForwardHeader?: boolean
  Routes: Route[]
}

interface Config {
  Rules?: ListenRule[]

  ListenAddr?: string
  SSLEnable?: boolean
  CertFile?: string
  KeyFile?: string
  Upstreams?: Upstream[]
  Upstream?: string
}

const cfg = ref<Config>({
  Rules: [],
})

const rules = ref<ListenRule[]>([
  {
    ListenAddr: '0.0.0.0:8888',
    SSLEnable: false,
    CertFile: '',
    KeyFile: '',
    BasicAuthEnable: false,
    BasicAuthUsername: '',
    BasicAuthPassword: '',
    Routes: [
      {
        Host: '',
        Path: '/',
        ProxyPassPath: '',
        SetHeaders: {},
        SetHeadersList: [],
        StaticDir: '',
        ExcludeBasicAuth: false,
        Upstreams: [{ URL: '', Weight: 1 }],
      },
    ],
  },
])

onMounted(async () => {
  const configData = (await GetConfig()) as any;

  if (Array.isArray(configData.rules) && configData.rules.length > 0) {
    rules.value = configData.rules.map((rule: any) => {
      const routes = (rule.routes || []).map((rt: any) => ({
        ID: rt.id || '',
        Host: rt.host || '',
        Path: rt.path || '/',
        ProxyPassPath: rt.proxy_pass_path || '',
        SetHeaders: rt.set_headers || {},
        SetHeadersList: Object.entries(rt.set_headers || {}).map(([Key, Value]) => ({
          Key,
          Value: String(Value ?? ''),
        })),
        StaticDir: rt.static_dir || '',
        ExcludeBasicAuth: !!rt.exclude_basic_auth,
        Upstreams: (rt.upstreams || []).map((u: any) => ({
          URL: u.url || '',
          Weight: u.weight || 1,
        })),
      }));

      return {
        ID: rule.id || '',
        ListenAddr: rule.listen_addr || '0.0.0.0:8888',
        SSLEnable: !!rule.ssl_enable,
        CertFile: rule.cert_file || '',
        KeyFile: rule.key_file || '',
        BasicAuthEnable: !!rule.basic_auth_enable,
        BasicAuthUsername: rule.basic_auth_username || '',
        BasicAuthPassword: rule.basic_auth_password || '',
        BasicAuthForwardHeader: !!rule.basic_auth_forward_header,
        Routes: routes.length > 0 ? routes : [{
          Host: '',
          Path: '/',
          ProxyPassPath: '',
          SetHeaders: {},
          SetHeadersList: [],
          StaticDir: '',
          ExcludeBasicAuth: false,
          Upstreams: [{ URL: '', Weight: 1 }],
        }],
      } as ListenRule;
    });
  } else {
    // 如果没有规则，则使用默认的空规则
    rules.value = [
      {
        ListenAddr: '0.0.0.0:8888',
        SSLEnable: false,
        CertFile: '',
        KeyFile: '',
        BasicAuthEnable: false,
        BasicAuthUsername: '',
        BasicAuthPassword: '',
        Routes: [
          {
            Host: '',
            Path: '/',
            ProxyPassPath: '',
            SetHeaders: {},
            SetHeadersList: [],
            StaticDir: '',
            ExcludeBasicAuth: false,
            Upstreams: [{ URL: '', Weight: 1 }],
          },
        ],
      },
    ];
  }
})

const addRule = () => {
  rules.value.push({
    ID: '',
    ListenAddr: '0.0.0.0:8888',
    SSLEnable: false,
    CertFile: '',
    KeyFile: '',
    BasicAuthEnable: false,
    BasicAuthUsername: '',
    BasicAuthPassword: '',
    BasicAuthForwardHeader: false,
    Routes: [
      {
        Host: '',
        Path: '/',
        ProxyPassPath: '',
        SetHeaders: {},
        SetHeadersList: [],
        StaticDir: '',
        ExcludeBasicAuth: false,
        Upstreams: [{ URL: '', Weight: 1 }],
      },
    ],
  })
}

const removeRule = (index: number) => {
  if (rules.value.length > 1) {
    rules.value.splice(index, 1)
  }
}

const addUpstream = (ruleIndex: number, routeIndex: number) => {
  rules.value[ruleIndex].Routes[routeIndex].Upstreams.push({ URL: '', Weight: 1 })
}

const removeUpstream = (ruleIndex: number, routeIndex: number, upstreamIndex: number) => {
  const list = rules.value[ruleIndex].Routes[routeIndex].Upstreams
  if (list.length > 1) {
    list.splice(upstreamIndex, 1)
  }
}

const applyCommonHeaders = (rt: Route) => {
  rt.SetHeadersList ||= []

  const common: HeaderKV[] = [
    { Key: 'Host', Value: '' },
    { Key: 'X-Real-IP', Value: '$remote_addr' },
    { Key: 'X-Forwarded-For', Value: '$proxy_add_x_forwarded_for' },
    { Key: 'X-Forwarded-Proto', Value: '$scheme' },
  ]

  const hostVal = (rt.Host || '').trim()

  const existing = new Map<string, number>()
  for (let i = 0; i < rt.SetHeadersList.length; i++) {
    const k = (rt.SetHeadersList[i].Key || '').trim().toLowerCase()
    if (!k) continue
    existing.set(k, i)
  }

  for (const kv of common) {
    const keyLower = kv.Key.toLowerCase()
    const value = kv.Key === 'Host' ? (hostVal || kv.Value) : kv.Value

    if (existing.has(keyLower)) {
      const idx = existing.get(keyLower)!
      if (!rt.SetHeadersList[idx].Value || rt.SetHeadersList[idx].Value.trim() === '') {
        rt.SetHeadersList[idx].Value = value
      }
    } else {
      rt.SetHeadersList.push({ Key: kv.Key, Value: value })
      existing.set(keyLower, rt.SetHeadersList.length - 1)
    }
  }
}

const addRoute = (ruleIndex: number) => {
  rules.value[ruleIndex].Routes.push({
    Host: '',
    Path: '/',
    ProxyPassPath: '',
    SetHeaders: {},
    SetHeadersList: [],
    StaticDir: '',
    ExcludeBasicAuth: false,
    Upstreams: [{ URL: '', Weight: 1 }],
  })
}

const removeRoute = (ruleIndex: number, routeIndex: number) => {
  const list = rules.value[ruleIndex].Routes
  if (list.length > 1) {
    list.splice(routeIndex, 1)
  }
}

const selectCertFile = async (ruleIndex: number) => {
  try {
    const filePath = await OpenCertFileDialog()
    if (filePath) {
      rules.value[ruleIndex].CertFile = filePath
    }
  } catch (error: any) {
    ElMessage.error(`选择证书文件失败: ${error.message || error}`)
  }
}

const selectKeyFile = async (ruleIndex: number) => {
  try {
    const filePath = await OpenKeyFileDialog()
    if (filePath) {
      rules.value[ruleIndex].KeyFile = filePath
    }
  } catch (error: any) {
    ElMessage.error(`选择私钥文件失败: ${error.message || error}`)
  }
}

const selectDirectory = async (ruleIndex: number, routeIndex: number) => {
  try {
    const dirPath = await OpenDirectoryDialog()
    if (dirPath) {
      rules.value[ruleIndex].Routes[routeIndex].StaticDir = dirPath
    }
  } catch (error: any) {
    ElMessage.error(`选择目录失败: ${error.message || error}`)
  }
}

const normalizePath = (p: string) => {
  const v = (p || '').trim()
  if (!v) return '/'
  return v.startsWith('/') ? v : '/' + v
}

// 获取配置（供父组件调用）
const getConfig = () => {
  const currentRules = [...rules.value]

  const cleanedRules: ListenRule[] = currentRules.map((rule) => ({
    ID: (rule.ID || '').trim(),
    ListenAddr: rule.ListenAddr.trim(),
    SSLEnable: !!rule.SSLEnable,
    CertFile: rule.CertFile || '',
    KeyFile: rule.KeyFile || '',
    BasicAuthEnable: !!rule.BasicAuthEnable,
    BasicAuthUsername: (rule.BasicAuthUsername || '').trim(),
    BasicAuthPassword: (rule.BasicAuthPassword || '').trim(),
    BasicAuthForwardHeader: !!rule.BasicAuthForwardHeader,
    Routes: rule.Routes.map((rt) => {
      const list = Array.isArray(rt.SetHeadersList) ? rt.SetHeadersList : []
      const setHeaders: Record<string, string> = {}
      for (const kv of list) {
        const k = (kv.Key || '').trim()
        if (!k) continue
        setHeaders[k] = (kv.Value || '').trim()
      }
      return {
        ID: (rt.ID || '').trim(),
        Host: (rt.Host || '').trim(),
        Path: normalizePath(rt.Path),
        ProxyPassPath: rt.ProxyPassPath ? normalizePath(rt.ProxyPassPath) : '',
        SetHeaders: setHeaders,
        StaticDir: (rt.StaticDir || '').trim(),
        ExcludeBasicAuth: !!rt.ExcludeBasicAuth,
        Upstreams: rt.Upstreams.filter((u) => u.URL.trim() !== '').map((u) => ({
          URL: u.URL.trim(),
          Weight: u.Weight > 0 ? u.Weight : 1,
        })),
      }
    }),
  }))

  for (let i = 0; i < cleanedRules.length; i++) {
    const rule = cleanedRules[i]
    if (!rule.ListenAddr) {
      throw new Error(`规则 ${i + 1}：监听地址不能为空`)
    }
    if (!rule.Routes || rule.Routes.length === 0) {
      throw new Error(`规则 ${i + 1}：请至少配置一个路由（routes）`)
    }

    for (let j = 0; j < rule.Routes.length; j++) {
      const rt: any = rule.Routes[j]
      if (!rt.Path) {
        throw new Error(`规则 ${i + 1} / 路由 ${j + 1}：Path 不能为空（至少为 /）`)
      }
      const hasUpstreams = rt.Upstreams && rt.Upstreams.length > 0
      const hasStaticDir = rt.StaticDir && rt.StaticDir.trim() !== ''
      if (!hasUpstreams && !hasStaticDir) {
        throw new Error(`规则 ${i + 1} / 路由 ${j + 1}：请至少配置一个上游服务器地址或静态文件目录`)
      }
    }

    if (rule.SSLEnable && (!rule.CertFile || !rule.KeyFile)) {
      throw new Error(`规则 ${i + 1}：启用SSL时证书文件和私钥文件不能为空`)
    }
    if (rule.BasicAuthEnable && (!rule.BasicAuthUsername || !rule.BasicAuthPassword)) {
      throw new Error(`规则 ${i + 1}：启用Basic Auth时用户名和密码不能为空`)
    }
  }

  // 关键：输出为 Rust 后端需要的 snake_case 结构
  const mappedRules = cleanedRules.map((r: any) => ({
    id: r.ID || undefined,
    listen_addr: r.ListenAddr,
    ssl_enable: !!r.SSLEnable,
    cert_file: r.CertFile,
    key_file: r.KeyFile,
    basic_auth_enable: !!r.BasicAuthEnable,
    basic_auth_username: r.BasicAuthUsername || '',
    basic_auth_password: r.BasicAuthPassword || '',
    basic_auth_forward_header: !!r.BasicAuthForwardHeader,
    routes: (r.Routes || []).map((rt: any) => ({
      id: rt.ID || undefined,
      host: rt.Host || undefined,
      path: rt.Path,
      proxy_pass_path: rt.ProxyPassPath || undefined,
      set_headers: rt.SetHeaders || {},
      static_dir: rt.StaticDir || undefined,
      exclude_basic_auth: !!rt.ExcludeBasicAuth,
      upstreams: (rt.Upstreams || []).map((u: any) => ({
        url: u.URL,
        weight: u.Weight,
      })),
    })),
  }))

  return {
    rules: mappedRules,
  }
}

defineExpose({
  getConfig,
})
</script>

<style scoped>
/* 样式保持不变（省略） */
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

.form-grid {
  margin-bottom: 24px;
}

.form-grid :deep(.el-form-item) {
  margin-bottom: 20px;
}

.mini-hint {
  display: block;
  margin-top: 6px;
  font-size: 12px;
  color: var(--text-muted);
}

.rules-section {
  display: flex;
  flex-direction: column;
  gap: 18px;
  margin-bottom: 18px;
}

.rule-card {
  margin-bottom: 18px;
  border-radius: 14px;
}

.rule-card :deep(.el-card__header) {
  padding: 12px 16px;
  border-bottom: 1px solid var(--border);
}

.rule-header {
  display: flex;
  justify-content: space-between;
  align-items: center;
}

.rule-header h4 {
  margin: 0;
  font-size: 16px;
  font-weight: 700;
  color: var(--text);
}

.route-card {
  margin-bottom: 16px;
  border-radius: 12px;
  background: var(--card-hover);
  border: 1px solid var(--border);
  transition: all 0.3s;
}

.route-card:hover {
  border-color: var(--border-hover);
  box-shadow: var(--shadow-sm);
}

.route-card :deep(.el-card__header) {
  padding: 16px;
  border-bottom: 1px solid var(--border);
}

.route-card :deep(.el-card__body) {
  padding: 20px;
}

.route-header {
  display: flex;
  justify-content: space-between;
  align-items: center;
}

.route-title {
  font-size: 15px;
  font-weight: 600;
  color: var(--text);
}

.route-form {
  width: 100%;
}

.route-form :deep(.el-form-item) {
  margin-bottom: 18px;
}

.headers-section {
  margin-top: 20px;
  border-radius: 12px;
}

.headers-section :deep(.el-card__header) {
  padding: 14px 16px;
  border-bottom: 1px solid var(--border);
}

.headers-section :deep(.el-card__body) {
  padding: 16px;
}

.headers-title {
  font-size: 13px;
  font-weight: 800;
  color: var(--text);
}

.upstreams-section {
  margin-top: 20px;
  border-radius: 12px;
}

.upstreams-section :deep(.el-card__header) {
  padding: 14px 16px;
  border-bottom: 1px solid var(--border);
}

.upstreams-section :deep(.el-card__body) {
  padding: 16px;
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

.file-selector .el-button {
  flex-shrink: 0;
}

.header-item {
  display: grid;
  grid-template-columns: 1fr 1fr auto;
  gap: 8px;
  align-items: center;
  margin-bottom: 12px;
}

.header-key,
.header-value {
  width: 100%;
}

@media (max-width: 768px) {
  .header-item {
    grid-template-columns: 1fr;
  }
}
</style>
