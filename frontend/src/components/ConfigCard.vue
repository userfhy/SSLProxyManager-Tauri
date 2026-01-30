<!-- frontend/src/components/ConfigCard.vue -->
<template>
  <el-card class="config-card config-page" shadow="hover">
    <template #header>
      <div style="display: flex; align-items: center; justify-content: space-between; gap: 12px;">
        <h3>{{ $t('configCard.title') }}</h3>
        <el-button type="primary" @click="exportConfigToml">
          {{ $t('configCard.exportConfig') }}
        </el-button>
      </div>
    </template>

    <!-- 规则配置 -->
    <TransitionGroup name="list" tag="div" class="rules-section">
      <el-card 
        v-for="(rule, ruleIndex) in rules" 
        :key="rule.ID || ruleIndex"
        class="rule-card"
        shadow="hover"
      >
        <template #header>
          <div class="rule-header">
            <h4>{{ $t('configCard.rule') }} {{ ruleIndex + 1 }}</h4>
            <div style="display: flex; align-items: center; gap: 12px;">
              <el-switch v-model="rule.Enabled" @change="() => onToggleListenRuleEnabled(rule)" />
              <el-button 
                @click="removeRule(ruleIndex)" 
                type="danger"
                size="small"
                :disabled="rules.length <= 1"
              >
                {{ $t('configCard.deleteRule') }}
              </el-button>
            </div>
          </div>
        </template>

        <el-form :model="rule" label-width="120px" class="form-grid">
          <el-form-item :label="$t('configCard.listenAddr')">
            <el-select
              v-model="rule.ListenAddrs"
              multiple
              filterable
              allow-create
              default-first-option
              placeholder="0.0.0.0:8888、[::]:8888、:8888"
              :no-data-text="$t('configCard.listenAddrPlaceholder')"
              :no-match-text="$t('configCard.noMatchText')"
              style="width: 360px;"
              class="listen-addr-input"
            >
            </el-select>
          </el-form-item>

          <el-form-item :label="$t('configCard.routes')" required>
            <TransitionGroup name="list" tag="div" class="routes-section">
              <el-card 
                v-for="(rt, routeIndex) in rule.Routes" 
                :key="rt.ID || routeIndex"
                class="route-card"
                shadow="never"
              >
                <template #header>
                  <div class="route-header">
                    <div class="route-title">{{ $t('configCard.route') }} {{ routeIndex + 1 }}</div>
                    <div style="display: flex; align-items: center; gap: 12px;">
                      <el-switch v-model="rt.Enabled" @change="() => onToggleRouteEnabled(rule, rt)" />
                      <el-button
                        @click="removeRoute(ruleIndex, routeIndex)"
                        type="danger"
                        size="small"
                        :disabled="rule.Routes.length <= 1"
                      >
                        {{ $t('configCard.deleteRoute') }}
                      </el-button>
                    </div>
                  </div>
                </template>

                <el-form :model="rt" label-width="160px" class="route-form">
                  <el-row :gutter="20" class="route-match">
                    <el-col :span="10">
                      <el-form-item :label="$t('configCard.host')">
                        <el-input v-model="rt.Host" :placeholder="$t('configCard.hostPlaceholder')" />
                      </el-form-item>
                    </el-col>
                    <el-col :span="10">
                      <el-form-item :label="$t('configCard.pathPrefix')">
                        <el-input v-model="rt.Path" :placeholder="$t('configCard.pathPlaceholder')" />
                      </el-form-item>
                    </el-col>
                  </el-row>

                  <el-row :gutter="22">
                    <el-col :span="10">
                      <el-form-item :label="$t('configCard.proxyPassPath')">
                        <el-input v-model="rt.ProxyPassPath" :placeholder="$t('configCard.proxyPassPathPlaceholder')" />
                        <el-text type="info" size="small" class="mini-hint">
                          {{ $t('configCard.proxyPassPathHint') }}
                        </el-text>
                      </el-form-item>

                      <el-form-item :label="$t('configCard.followRedirects')" style="margin-top: 10px;">
                        <el-switch v-model="rt.FollowRedirects" />
                        <el-text type="info" size="small" class="mini-hint">
                          {{ $t('configCard.followRedirectsHint') }}
                        </el-text>
                      </el-form-item>
                    </el-col>
                    <el-col :span="10">
                      <el-form-item :label="$t('configCard.staticDir')">
                        <div class="file-selector">
                          <el-input v-model="rt.StaticDir" :placeholder="$t('configCard.staticDirPlaceholder')" readonly />
                        </div>
                        <el-text type="info" size="small" class="mini-hint">
                          {{ $t('configCard.staticDirHint') }}
                        </el-text>
                      </el-form-item>
                    </el-col>
                    <el-col :span="2">
                      <el-button @click="selectDirectory(ruleIndex, routeIndex)" size="small" type="primary" :icon="Folder">
                            {{ $t('configCard.selectDir') }}
                          </el-button>
                    </el-col>
                  </el-row>

                  <el-row :gutter="20">
                    <el-col :span="10">
                      <el-form-item>
                        <el-checkbox v-model="rt.ExcludeBasicAuth">
                          {{ $t('configCard.excludeBasicAuth') }}
                        </el-checkbox>
                        <el-text type="info" size="small" class="mini-hint">
                          {{ $t('configCard.excludeBasicAuthHint') }}
                        </el-text>
                      </el-form-item>
                    </el-col>
                  </el-row>
                </el-form>

                <div class="sub-section">
                  <div class="sub-section-header">{{ $t('configCard.upstreamServers') }}</div>
                  <div class="sub-section-body">
                    <TransitionGroup name="list" tag="div">
                      <div v-for="(upstream, index) in rt.Upstreams" :key="index" class="upstream-item">
                          <el-input
                            v-model="upstream.URL"
                            :placeholder="index === 0 ? 'http://127.0.0.1:8080' : 'https://example.com'"
                          />
                          <el-input-number
                            v-model="upstream.Weight"
                            :min="1"
                            :placeholder="$t('configCard.weight')"
                          />
                          <el-button
                            @click="removeUpstream(ruleIndex, routeIndex, index)"
                            type="danger"
                            size="small"
                            :disabled="rt.Upstreams.length <= 1 && !(rt.StaticDir && rt.StaticDir.trim() !== '')"
                          >
                            {{ $t('configCard.delete') }}
                          </el-button>
                      </div>
                    </TransitionGroup>
                    <el-button @click="addUpstream(ruleIndex, routeIndex)" type="primary" style="margin-top: 12px;">
                      <el-icon><Plus /></el-icon> {{ $t('configCard.addUpstream') }}
                    </el-button>
                  </div>
                </div>

                <div class="sub-section">
                  <div class="sub-section-header">{{ $t('configCard.proxySetHeader') }}</div>
                  <div class="sub-section-body">
                    <el-text type="info" size="small" class="headers-hint">
                      {{ $t('configCard.proxySetHeaderHint') }}
                    </el-text>
                    <div class="headers-actions">
                      <el-button @click="applyCommonHeaders(rt)" type="primary" size="small">
                        <el-icon><MagicStick /></el-icon> {{ $t('configCard.quickApplyHeaders') }}
                      </el-button>
                    </div>
                    <TransitionGroup name="list" tag="div">
                      <div v-for="(kv, hIndex) in (rt.SetHeadersList || [])" :key="hIndex" class="header-item">
                        <el-input v-model="kv.Key" :placeholder="$t('configCard.headerKeyPlaceholder')" />
                        <el-input v-model="kv.Value" :placeholder="$t('configCard.headerValuePlaceholder')" />
                        <el-button @click="(rt.SetHeadersList || []).splice(hIndex, 1)" type="danger" size="small">{{ $t('configCard.delete') }}</el-button>
                      </div>
                    </TransitionGroup>
                    <el-button @click="(rt.SetHeadersList ||= []).push({ Key: '', Value: '' })" type="primary" size="small" style="margin-top: 12px;">
                      <el-icon><Plus /></el-icon> {{ $t('configCard.addHeader') }}
                    </el-button>
                  </div>
                </div>

                <div class="sub-section">
                  <div class="sub-section-header">{{ $t('configCard.requestResponseModify') }}</div>
                  <div class="sub-section-body">
                    <!-- URL 重写规则 -->
                    <el-form-item :label="$t('configCard.urlRewrite')">
                      <TransitionGroup name="list" tag="div">
                        <div v-for="(rule, idx) in (rt.UrlRewriteRules || [])" :key="idx" class="rewrite-rule-item">
                          <el-input v-model="rule.Pattern" :placeholder="$t('configCard.urlRewritePattern')" style="flex: 1;" />
                          <el-input v-model="rule.Replacement" :placeholder="$t('configCard.urlRewriteReplacement')" style="flex: 1;" />
                          <el-switch v-model="rule.Enabled" />
                          <el-button @click="(rt.UrlRewriteRules || []).splice(idx, 1)" type="danger" size="small">{{ $t('configCard.delete') }}</el-button>
                        </div>
                      </TransitionGroup>
                      <el-button @click="(rt.UrlRewriteRules ||= []).push({ Pattern: '', Replacement: '', Enabled: true })" type="primary" size="small" style="margin-top: 12px;">
                        <el-icon><Plus /></el-icon> {{ $t('configCard.addUrlRewrite') }}
                      </el-button>
                      <el-text type="info" size="small" class="mini-hint">
                        {{ $t('configCard.urlRewriteHint') }}
                      </el-text>
                    </el-form-item>

                    <!-- 请求体替换规则 -->
                    <el-form-item :label="$t('configCard.requestBodyReplace')">
                      <TransitionGroup name="list" tag="div">
                        <div v-for="(rule, idx) in (rt.RequestBodyReplace || [])" :key="idx" class="replace-rule-item">
                          <el-input v-model="rule.Find" :placeholder="$t('configCard.findText')" style="flex: 1;" />
                          <el-input v-model="rule.Replace" :placeholder="$t('configCard.replaceWith')" style="flex: 1;" />
                          <el-checkbox v-model="rule.UseRegex">{{ $t('configCard.regex') }}</el-checkbox>
                          <el-switch v-model="rule.Enabled" />
                          <el-button @click="(rt.RequestBodyReplace || []).splice(idx, 1)" type="danger" size="small">{{ $t('configCard.delete') }}</el-button>
                        </div>
                      </TransitionGroup>
                      <el-button @click="(rt.RequestBodyReplace ||= []).push({ Find: '', Replace: '', UseRegex: false, Enabled: true })" type="primary" size="small" style="margin-top: 12px;">
                        <el-icon><Plus /></el-icon> {{ $t('configCard.addRequestBodyReplace') }}
                      </el-button>
                      <el-text type="info" size="small" class="mini-hint">
                        {{ $t('configCard.requestBodyReplaceHint') }}
                      </el-text>
                    </el-form-item>

                    <!-- 响应体替换规则 -->
                    <el-form-item :label="$t('configCard.responseBodyReplace')">
                      <el-alert
                        type="warning"
                        :closable="false"
                        style="margin-bottom: 12px;"
                      >
                        <template #title>
                          <strong>{{ $t('configCard.legalWarning') }}</strong>
                        </template>
                        <template #default>
                          <div style="font-size: 12px; line-height: 1.6;">
                            {{ $t('configCard.legalWarningContent') }}
                            <br />• {{ $t('configCard.legalWarningItem1') }}
                            <br />• {{ $t('configCard.legalWarningItem2') }}
                            <br />• {{ $t('configCard.legalWarningItem3') }}
                            <br />• {{ $t('configCard.legalWarningItem4') }}
                          </div>
                        </template>
                      </el-alert>
                      <TransitionGroup name="list" tag="div">
                        <div v-for="(rule, idx) in (rt.ResponseBodyReplace || [])" :key="idx" class="replace-rule-item">
                          <el-input v-model="rule.Find" :placeholder="$t('configCard.findText')" style="flex: 1;" />
                          <el-input v-model="rule.Replace" :placeholder="$t('configCard.replaceWith')" style="flex: 1;" />
                          <el-checkbox v-model="rule.UseRegex">{{ $t('configCard.regex') }}</el-checkbox>
                          <el-switch v-model="rule.Enabled" />
                          <el-button @click="(rt.ResponseBodyReplace || []).splice(idx, 1)" type="danger" size="small">{{ $t('configCard.delete') }}</el-button>
                        </div>
                      </TransitionGroup>
                      <el-button @click="(rt.ResponseBodyReplace ||= []).push({ Find: '', Replace: '', UseRegex: false, Enabled: true })" type="primary" size="small" style="margin-top: 12px;">
                        <el-icon><Plus /></el-icon> {{ $t('configCard.addResponseBodyReplace') }}
                      </el-button>
                      <el-text type="info" size="small" class="mini-hint">
                        {{ $t('configCard.responseBodyReplaceHint') }}
                      </el-text>
                    </el-form-item>

                    <!-- 移除请求/响应头 -->
                    <el-form-item :label="$t('configCard.removeHeaders')">
                      <TransitionGroup name="list" tag="div">
                        <div v-for="(header, idx) in (rt.RemoveHeaders || [])" :key="idx" class="header-item">
                          <el-input v-model="(rt.RemoveHeaders || [])[idx]" :placeholder="$t('configCard.headerNamePlaceholder')" />
                          <el-button @click="(rt.RemoveHeaders || []).splice(idx, 1)" type="danger" size="small">{{ $t('configCard.delete') }}</el-button>
                        </div>
                      </TransitionGroup>
                      <el-button @click="(rt.RemoveHeaders ||= []).push('')" type="primary" size="small" style="margin-top: 12px;">
                        <el-icon><Plus /></el-icon> {{ $t('configCard.addRemoveHeader') }}
                      </el-button>
                      <el-text type="info" size="small" class="mini-hint">
                        {{ $t('configCard.removeHeadersHint') }}
                      </el-text>
                    </el-form-item>
                  </div>
                </div>
              </el-card>
            </TransitionGroup>
              <el-button @click="addRoute(ruleIndex)" type="primary" style="margin-top: 10px;">
                <el-icon><Plus /></el-icon> {{ $t('configCard.addNewRoute') }}
              </el-button>
          </el-form-item>

          <el-form-item>
            <el-checkbox v-model="rule.SSLEnable">{{ $t('configCard.enableSSLForRule') }}</el-checkbox>
          </el-form-item>

          <template v-if="rule.SSLEnable">
            <el-form-item :label="$t('configCard.certFileLabel')">
              <div class="file-selector">
                <el-input v-model="rule.CertFile" placeholder="ssl/server.crt" readonly />
                <el-button @click="selectCertFile(ruleIndex)" type="primary" :icon="Folder">
                  {{ $t('configCard.selectFile') }}
                </el-button>
              </div>
            </el-form-item>
            <el-form-item :label="$t('configCard.keyFileLabel')">
              <div class="file-selector">
                <el-input v-model="rule.KeyFile" placeholder="ssl/server.key" readonly />
                <el-button @click="selectKeyFile(ruleIndex)" type="primary" :icon="Folder">
                  {{ $t('configCard.selectFile') }}
                </el-button>
              </div>
            </el-form-item>
          </template>

          <el-form-item>
            <el-checkbox v-model="rule.BasicAuthEnable">{{ $t('configCard.enableBasicAuth') }}</el-checkbox>
          </el-form-item>

          <template v-if="rule.BasicAuthEnable">
            <el-form-item :label="$t('configCard.username')">
              <el-input v-model="rule.BasicAuthUsername" placeholder="admin" />
            </el-form-item>
            <el-form-item :label="$t('configCard.password')">
              <el-input v-model="rule.BasicAuthPassword" type="password" placeholder="password" show-password />
            </el-form-item>
            <el-form-item>
              <el-checkbox v-model="rule.BasicAuthForwardHeader">
                {{ $t('configCard.forwardBasicAuthHeader') }}
              </el-checkbox>
              <el-text type="info" size="small" class="mini-hint">
                {{ $t('configCard.forwardBasicAuthHeaderHint') }}
              </el-text>
            </el-form-item>
          </template>

          <el-divider />

          <el-form-item>
            <el-checkbox v-model="rule.RateLimitEnabled">{{ $t('configCard.enableRateLimit') }}</el-checkbox>
            <el-text type="info" size="small" class="mini-hint" style="margin-left: 10px;">
              {{ $t('configCard.rateLimitHint') }}
            </el-text>
          </el-form-item>

          <template v-if="rule.RateLimitEnabled">
            <el-form-item :label="$t('configCard.rateLimitRequestsPerSecond')">
              <el-input-number 
                v-model="rule.RateLimitRequestsPerSecond" 
                :min="1" 
                :max="10000" 
                :step="1" 
                controls-position="right"
                style="width: 200px;"
              />
              <el-text type="info" size="small" class="mini-hint" style="margin-left: 10px;">
                {{ $t('configCard.rateLimitRequestsPerSecondHint') }}
              </el-text>
            </el-form-item>

            <el-form-item :label="$t('configCard.rateLimitBurstSize')">
              <el-input-number 
                v-model="rule.RateLimitBurstSize" 
                :min="1" 
                :max="1000" 
                :step="1" 
                controls-position="right"
                style="width: 200px;"
              />
              <el-text type="info" size="small" class="mini-hint" style="margin-left: 10px;">
                {{ $t('configCard.rateLimitBurstSizeHint') }}
              </el-text>
            </el-form-item>

            <el-form-item :label="$t('configCard.rateLimitBanSeconds')">
              <el-input-number 
                v-model="rule.RateLimitBanSeconds" 
                :min="0" 
                :max="86400" 
                :step="1" 
                controls-position="right"
                style="width: 200px;"
              />
              <el-text type="info" size="small" class="mini-hint" style="margin-left: 10px;">
                {{ $t('configCard.rateLimitBanSecondsHint') }}
              </el-text>
            </el-form-item>
          </template>
        </el-form>
      </el-card>
    </TransitionGroup>

      <el-button @click="addRule" type="primary" style="margin-top: 10px;">
        <el-icon><Plus /></el-icon> {{ $t('configCard.addNewListenRule') }}
      </el-button>
  </el-card>
</template>

<script setup lang="ts">
import { ref, onMounted } from 'vue'
import { GetConfig, OpenCertFileDialog, OpenKeyFileDialog, OpenDirectoryDialog, ExportCurrentConfigToml, SetListenRuleEnabled, SetRouteEnabled } from '../api'
import { Plus, MagicStick, Folder } from '@element-plus/icons-vue'
import { ElMessage } from 'element-plus'
import { useI18n } from 'vue-i18n'

const { t } = useI18n()

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
  Enabled?: boolean
  Host: string
  Path: string

  ProxyPassPath?: string
  FollowRedirects?: boolean
  SetHeaders?: Record<string, string>
  SetHeadersList?: HeaderKV[]

  StaticDir?: string
  ExcludeBasicAuth?: boolean
  UrlRewriteRules?: UrlRewriteRule[]
  RequestBodyReplace?: BodyReplaceRule[]
  ResponseBodyReplace?: BodyReplaceRule[]
  RemoveHeaders?: string[]

  Upstreams: Upstream[]
}

interface UrlRewriteRule {
  Pattern: string
  Replacement: string
  Enabled?: boolean
}

interface BodyReplaceRule {
  Find: string
  Replace: string
  UseRegex?: boolean
  Enabled?: boolean
}

interface ListenRule {
  ID?: string
  Enabled?: boolean
  ListenAddr: string         // 兼容旧字段（单个）
  ListenAddrs?: string[]     // 新字段：多个监听地址
  SSLEnable: boolean
  CertFile: string
  KeyFile: string
  BasicAuthEnable?: boolean
  BasicAuthUsername?: string
  BasicAuthPassword?: string
  BasicAuthForwardHeader?: boolean
  RateLimitEnabled?: boolean
  RateLimitRequestsPerSecond?: number
  RateLimitBurstSize?: number
  RateLimitBanSeconds?: number
  Routes: Route[]
}

// Tauri 后端返回的文件选择结果可能是 string | null
// 这里兼容 ElementPlus v-model 以及 OpenDirectoryDialog 返回类型

const rules = ref<ListenRule[]>([
  {
    ListenAddr: '0.0.0.0:8888',
    ListenAddrs: ['0.0.0.0:8888'],
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
        SetHeaders: {} as Record<string, string>,
        SetHeadersList: [],
        StaticDir: '',
        ExcludeBasicAuth: false,
        Upstreams: [{ URL: '', Weight: 1 }],
      },
    ],
  },
])

const onToggleListenRuleEnabled = async (rule: ListenRule) => {
  try {
    if (!rule.ID) {
      ElMessage.warning(t('configCard.saveConfigFirst'))
      rule.Enabled = true
      return
    }
    const next = !!rule.Enabled
    const cfg = (await SetListenRuleEnabled(rule.ID, next)) as any
    // 用后端返回的最新配置回填，保证持久化状态与 UI 一致
    if (cfg && Array.isArray(cfg.rules)) {
      const found = cfg.rules.find((r: any) => (r.id || '') === rule.ID)
      if (found && found.enabled !== undefined) {
        rule.Enabled = !!found.enabled
      }
    }
    ElMessage.success(next ? t('configCard.listenRuleEnabled') : t('configCard.listenRuleDisabled'))
  } catch (error: any) {
    ElMessage.error(t('configCard.toggleListenRuleFailed', { error: error?.message || error }))
    rule.Enabled = !rule.Enabled
  }
}

const onToggleRouteEnabled = async (rule: ListenRule, rt: Route) => {
  try {
    if (!rule.ID || !rt.ID) {
      ElMessage.warning(t('configCard.saveConfigFirstForRoute'))
      rt.Enabled = true
      return
    }
    const next = !!rt.Enabled
    const cfg = (await SetRouteEnabled(rule.ID, rt.ID, next)) as any
    if (cfg && Array.isArray(cfg.rules)) {
      const foundRule = cfg.rules.find((r: any) => (r.id || '') === rule.ID)
      const foundRt = (foundRule?.routes || []).find((r: any) => (r.id || '') === rt.ID)
      if (foundRt && foundRt.enabled !== undefined) {
        rt.Enabled = !!foundRt.enabled
      }
    }
    ElMessage.success(next ? t('configCard.routeEnabled') : t('configCard.routeDisabled'))
  } catch (error: any) {
    ElMessage.error(t('configCard.toggleRouteFailed', { error: error?.message || error }))
    rt.Enabled = !rt.Enabled
  }
}

onMounted(async () => {
  const configData = (await GetConfig()) as any;

  if (Array.isArray(configData.rules) && configData.rules.length > 0) {
    rules.value = configData.rules.map((rule: any) => {
      const routes = (rule.routes || []).map((rt: any) => ({
        ID: rt.id || '',
        Enabled: rt.enabled !== undefined ? !!rt.enabled : true,
        Host: rt.host || '',
        Path: rt.path || '/',
        ProxyPassPath: rt.proxy_pass_path || '',
        FollowRedirects: !!rt.follow_redirects,
        SetHeaders: rt.set_headers || {},
        SetHeadersList: Object.entries(rt.set_headers || {}).map(([Key, Value]) => ({
          Key,
          Value: String(Value ?? ''),
        })),
        StaticDir: rt.static_dir || '',
        ExcludeBasicAuth: !!rt.exclude_basic_auth,
        UrlRewriteRules: (rt.url_rewrite_rules || []).map((r: any) => ({
          Pattern: r.pattern || '',
          Replacement: r.replacement || '',
          Enabled: r.enabled !== undefined ? !!r.enabled : true,
        })),
        RequestBodyReplace: (rt.request_body_replace || []).map((r: any) => ({
          Find: r.find || '',
          Replace: r.replace || '',
          UseRegex: !!r.use_regex,
          Enabled: r.enabled !== undefined ? !!r.enabled : true,
        })),
        ResponseBodyReplace: (rt.response_body_replace || []).map((r: any) => ({
          Find: r.find || '',
          Replace: r.replace || '',
          UseRegex: !!r.use_regex,
          Enabled: r.enabled !== undefined ? !!r.enabled : true,
        })),
        RemoveHeaders: rt.remove_headers || [],
        Upstreams: (rt.upstreams || []).map((u: any) => ({
          URL: u.url || '',
          Weight: u.weight || 1,
        })),
      }));

      return {
        ID: rule.id || '',
        Enabled: rule.enabled !== undefined ? !!rule.enabled : true,
        // 后端向下兼容：如果有 listen_addrs 就用数组，否则从 listen_addr 构造
        ListenAddr: rule.listen_addr || '0.0.0.0:8888',
        ListenAddrs: Array.isArray(rule.listen_addrs) && rule.listen_addrs.length > 0
          ? rule.listen_addrs
          : [(rule.listen_addr || '0.0.0.0:8888')],
        SSLEnable: !!rule.ssl_enable,
        CertFile: rule.cert_file || '',
        KeyFile: rule.key_file || '',
        BasicAuthEnable: !!rule.basic_auth_enable,
        BasicAuthUsername: rule.basic_auth_username || '',
        BasicAuthPassword: rule.basic_auth_password || '',
        BasicAuthForwardHeader: !!rule.basic_auth_forward_header,
    RateLimitEnabled: rule.rate_limit_enabled !== undefined ? !!rule.rate_limit_enabled : undefined,
    RateLimitRequestsPerSecond: rule.rate_limit_requests_per_second !== undefined ? Number(rule.rate_limit_requests_per_second) : undefined,
    RateLimitBurstSize: rule.rate_limit_burst_size !== undefined ? Number(rule.rate_limit_burst_size) : undefined,
    RateLimitBanSeconds: rule.rate_limit_ban_seconds !== undefined ? Number(rule.rate_limit_ban_seconds) : undefined,
        Routes: routes.length > 0 ? routes : [{
          Host: '',
          Path: '/',
          ProxyPassPath: '',
          SetHeaders: {} as Record<string, string>,
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
        ListenAddrs: ['0.0.0.0:8888'],
        SSLEnable: false,
        CertFile: '',
        KeyFile: '',
        BasicAuthEnable: false,
        BasicAuthUsername: '',
        BasicAuthPassword: '',
        BasicAuthForwardHeader: false,
        RateLimitEnabled: undefined,
        RateLimitRequestsPerSecond: undefined,
        RateLimitBurstSize: undefined,
        RateLimitBanSeconds: undefined,
        Routes: [
          {
            Host: '',
            Path: '/',
            ProxyPassPath: '',
            SetHeaders: {} as Record<string, string>,
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
    ID: `new-rule-${Date.now()}`,
    ListenAddr: '0.0.0.0:8888',
    ListenAddrs: ['0.0.0.0:8888'],
    SSLEnable: false,
    CertFile: '',
    KeyFile: '',
    BasicAuthEnable: false,
    BasicAuthUsername: '',
    BasicAuthPassword: '',
    BasicAuthForwardHeader: false,
    RateLimitEnabled: undefined,
    RateLimitRequestsPerSecond: undefined,
    RateLimitBurstSize: undefined,
    RateLimitBanSeconds: undefined,
    Routes: [
      {
        ID: `new-route-${Date.now()}`,
        Host: '',
        Path: '/',
        ProxyPassPath: '',
        SetHeaders: {} as Record<string, string>,
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
  const rt = rules.value[ruleIndex].Routes[routeIndex]
  const list = rt.Upstreams
  const hasStaticDir = !!(rt.StaticDir && rt.StaticDir.trim() !== '')

  // 如果配置了静态目录，则允许删到 0 个上游
  const minLen = hasStaticDir ? 0 : 1
  if (list.length > minLen) {
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
    ID: `new-route-${Date.now()}`,
    Host: '',
    Path: '/',
    ProxyPassPath: '',
    SetHeaders: {} as Record<string, string>,
    SetHeadersList: [],
    StaticDir: '',
    ExcludeBasicAuth: false,
    UrlRewriteRules: [],
    RequestBodyReplace: [],
    ResponseBodyReplace: [],
    RemoveHeaders: [],
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
      rules.value[ruleIndex].CertFile = String(filePath)
    }
  } catch (error: any) {
    ElMessage.error(t('configCard.selectCertFileFailed', { error: error.message || error }))
  }
}

const selectKeyFile = async (ruleIndex: number) => {
  try {
    const filePath = await OpenKeyFileDialog()
    if (filePath) {
      rules.value[ruleIndex].KeyFile = String(filePath)
    }
  } catch (error: any) {
    ElMessage.error(t('configCard.selectKeyFileFailed', { error: error.message || error }))
  }
}

const selectDirectory = async (ruleIndex: number, routeIndex: number) => {
  try {
    const dirPath = await OpenDirectoryDialog()
    if (dirPath) {
      const rt = rules.value[ruleIndex].Routes[routeIndex]
      rt.StaticDir = String(dirPath)

      // 如果选择了静态目录，允许上游为空：自动清理掉仅用于占位的空上游
      rt.Upstreams = (rt.Upstreams || []).filter((u) => (u.URL || '').trim() !== '')
    }
  } catch (error: any) {
    ElMessage.error(t('configCard.selectDirFailed', { error: error.message || error }))
  }
}

const normalizePath = (p: string) => {
  const v = (p || '').trim()
  if (!v) return '/'
  return v.startsWith('/') ? v : '/' + v
}

const exportConfigToml = async () => {
  try {
    const savedPath = (await ExportCurrentConfigToml()) as string | null
    if (savedPath) {
      ElMessage.success(t('configCard.exportSuccess', { path: savedPath }))
    }
  } catch (error: any) {
    ElMessage.error(t('configCard.exportFailed', { error: error?.message || error }))
  }
}


// 获取配置（供父组件调用）
const getConfig = () => {
  const currentRules = [...rules.value]

  const cleanedRules: ListenRule[] = currentRules.map((rule) => ({
    ID: (rule.ID || '').trim(),
    Enabled: rule.Enabled !== undefined ? !!rule.Enabled : true,
    ListenAddr: rule.ListenAddr.trim(),
    ListenAddrs: (rule.ListenAddrs && rule.ListenAddrs.length > 0
      ? rule.ListenAddrs
      : [rule.ListenAddr]
    ).map((s) => s.trim()).filter((s) => s !== ''),
    SSLEnable: !!rule.SSLEnable,
    CertFile: rule.CertFile || '',
    KeyFile: rule.KeyFile || '',
    BasicAuthEnable: !!rule.BasicAuthEnable,
    BasicAuthUsername: (rule.BasicAuthUsername || '').trim(),
    BasicAuthPassword: (rule.BasicAuthPassword || '').trim(),
    BasicAuthForwardHeader: !!rule.BasicAuthForwardHeader,
    RateLimitEnabled: rule.RateLimitEnabled !== undefined ? !!rule.RateLimitEnabled : undefined,
    RateLimitRequestsPerSecond: rule.RateLimitRequestsPerSecond !== undefined ? Number(rule.RateLimitRequestsPerSecond) : undefined,
    RateLimitBurstSize: rule.RateLimitBurstSize !== undefined ? Number(rule.RateLimitBurstSize) : undefined,
    RateLimitBanSeconds: rule.RateLimitBanSeconds !== undefined ? Number(rule.RateLimitBanSeconds) : undefined,
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
        Enabled: rt.Enabled !== undefined ? !!rt.Enabled : true,
        Host: (rt.Host || '').trim(),
        Path: normalizePath(rt.Path),
        ProxyPassPath: rt.ProxyPassPath ? normalizePath(rt.ProxyPassPath) : '',
        FollowRedirects: !!rt.FollowRedirects,
        SetHeaders: setHeaders,
        StaticDir: (rt.StaticDir || '').trim(),
        ExcludeBasicAuth: !!rt.ExcludeBasicAuth,
        UrlRewriteRules: (rt.UrlRewriteRules || []).filter((r) => r.Pattern.trim() !== '').map((r) => ({
          Pattern: r.Pattern.trim(),
          Replacement: r.Replacement.trim(),
          Enabled: r.Enabled !== undefined ? !!r.Enabled : true,
        })),
        RequestBodyReplace: (rt.RequestBodyReplace || []).filter((r) => r.Find.trim() !== '').map((r) => ({
          Find: r.Find.trim(),
          Replace: r.Replace.trim(),
          UseRegex: !!r.UseRegex,
          Enabled: r.Enabled !== undefined ? !!r.Enabled : true,
        })),
        ResponseBodyReplace: (rt.ResponseBodyReplace || []).filter((r) => r.Find.trim() !== '').map((r) => ({
          Find: r.Find.trim(),
          Replace: r.Replace.trim(),
          UseRegex: !!r.UseRegex,
          Enabled: r.Enabled !== undefined ? !!r.Enabled : true,
        })),
        RemoveHeaders: (rt.RemoveHeaders || []).filter((h) => h.trim() !== '').map((h) => h.trim()),
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
      throw new Error(t('configCard.ruleListenAddrEmpty', { index: i + 1 }))
    }
    if (!rule.Routes || rule.Routes.length === 0) {
      throw new Error(t('configCard.ruleNoRoutes', { index: i + 1 }))
    }

    for (let j = 0; j < rule.Routes.length; j++) {
      const rt: any = rule.Routes[j]
      if (!rt.Path) {
        throw new Error(t('configCard.routePathEmpty', { ruleIndex: i + 1, routeIndex: j + 1 }))
      }
      const hasUpstreams = rt.Upstreams && rt.Upstreams.length > 0
      const hasStaticDir = rt.StaticDir && rt.StaticDir.trim() !== ''
      if (!hasUpstreams && !hasStaticDir) {
        throw new Error(t('configCard.routeNoUpstreamOrStatic', { ruleIndex: i + 1, routeIndex: j + 1 }))
      }
    }

    if (rule.SSLEnable && (!rule.CertFile || !rule.KeyFile)) {
      throw new Error(t('configCard.ruleSSLCertEmpty', { index: i + 1 }))
    }
    if (rule.BasicAuthEnable && (!rule.BasicAuthUsername || !rule.BasicAuthPassword)) {
      throw new Error(t('configCard.ruleBasicAuthEmpty', { index: i + 1 }))
    }
  }

  // 关键：输出为 Rust 后端需要的 snake_case 结构
  const mappedRules = cleanedRules.map((r: any) => ({
    id: r.ID || undefined,
    enabled: r.Enabled !== undefined ? !!r.Enabled : true,
    // 向后端输出新的 listen_addrs 数组，同时保留第一个为 listen_addr 兼容旧字段
    listen_addr: r.ListenAddrs[0] || r.ListenAddr,
    listen_addrs: r.ListenAddrs,
    ssl_enable: !!r.SSLEnable,
    cert_file: r.CertFile,
    key_file: r.KeyFile,
    basic_auth_enable: !!r.BasicAuthEnable,
    basic_auth_username: r.BasicAuthUsername || '',
    basic_auth_password: r.BasicAuthPassword || '',
    basic_auth_forward_header: !!r.BasicAuthForwardHeader,
    rate_limit_enabled: r.RateLimitEnabled !== undefined ? !!r.RateLimitEnabled : undefined,
    rate_limit_requests_per_second: r.RateLimitRequestsPerSecond !== undefined ? Number(r.RateLimitRequestsPerSecond) : undefined,
    rate_limit_burst_size: r.RateLimitBurstSize !== undefined ? Number(r.RateLimitBurstSize) : undefined,
    rate_limit_window_seconds: r.RateLimitWindowSeconds !== undefined ? Number(r.RateLimitWindowSeconds) : 1,
    rate_limit_ban_seconds: r.RateLimitBanSeconds !== undefined ? Number(r.RateLimitBanSeconds) : undefined,
    routes: (r.Routes || []).map((rt: any) => ({
      id: rt.ID || undefined,
      enabled: rt.Enabled !== undefined ? !!rt.Enabled : true,
      host: rt.Host || undefined,
      path: rt.Path,
      proxy_pass_path: rt.ProxyPassPath || undefined,
      follow_redirects: !!rt.FollowRedirects,
      set_headers: rt.SetHeaders || {},
      static_dir: rt.StaticDir || undefined,
      exclude_basic_auth: !!rt.ExcludeBasicAuth,
      url_rewrite_rules: (rt.UrlRewriteRules || []).filter((r: any) => r.Pattern.trim() !== '').map((r: any) => ({
        pattern: r.Pattern.trim(),
        replacement: r.Replacement.trim(),
        enabled: r.Enabled !== undefined ? !!r.Enabled : true,
      })),
      request_body_replace: (rt.RequestBodyReplace || []).filter((r: any) => r.Find.trim() !== '').map((r: any) => ({
        find: r.Find.trim(),
        replace: r.Replace.trim(),
        use_regex: !!r.UseRegex,
        enabled: r.Enabled !== undefined ? !!r.Enabled : true,
      })),
      response_body_replace: (rt.ResponseBodyReplace || []).filter((r: any) => r.Find.trim() !== '').map((r: any) => ({
        find: r.Find.trim(),
        replace: r.Replace.trim(),
        use_regex: !!r.UseRegex,
        enabled: r.Enabled !== undefined ? !!r.Enabled : true,
      })),
      remove_headers: (rt.RemoveHeaders || []).filter((h: any) => h.trim() !== '').map((h: any) => h.trim()),
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
.config-page {
  height: 100%;
  overflow-y: auto;
  padding: 16px;
}

.config-page :deep(.el-card__header) {
  border-bottom: 1px solid var(--border);
  padding: 16px 20px;
}

.config-page :deep(.el-card__body) {
  padding: 20px;
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

.form-grid :deep(.el-form-item) {
  margin-bottom: 22px;
}

.mini-hint {
  display: block;
  margin-top: 6px;
  font-size: 12px;
  line-height: 1.4;
  color: var(--text-muted);
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
  overflow: visible; /* For transition effects */
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

.routes-section {
  display: flex;
  flex-direction: column;
  gap: 16px;
  padding-left: 12px;
}

.route-card {
  border-radius: var(--radius-md);
  background: var(--input-bg);
  border: 1px solid transparent;
  transition: all 0.3s;
}

.route-card:hover {
  border-color: var(--border-hover);
  transform: translateY(-2px);
}

.route-header {
  display: flex;
  justify-content: space-between;
  align-items: center;
}

.route-title {
  font-size: 16px;
  font-weight: 600;
  color: var(--text);
}

.sub-section {
  margin-top: 20px;
  border-radius: var(--radius-md);
  border: 1px solid var(--border);
  background: var(--card-bg);
}

.sub-section-header {
  padding: 12px 16px;
  border-bottom: 1px solid var(--border);
  font-size: 14px;
  font-weight: 600;
  color: var(--text-muted);
}

.sub-section-body {
  padding: 16px;
}

.file-selector {
  display: flex;
  gap: 8px;
  align-items: center;
  width: 100%;
}

.header-item, .upstream-item {
  display: grid;
  grid-template-columns: 1fr 1fr auto;
  gap: 12px;
  align-items: center;
  margin-bottom: 12px;
}

.upstream-item {
  grid-template-columns: 2fr 1fr auto;
}

.rewrite-rule-item {
  grid-template-columns: 2fr 2fr auto auto;
}

.replace-rule-item {
  grid-template-columns: 2fr 2fr auto auto auto;
}

.headers-hint {
  display: block;
  margin-bottom: 12px;
}

.headers-actions {
  margin-bottom: 16px;
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
