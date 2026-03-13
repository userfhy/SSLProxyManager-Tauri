<template>
  <div class="test-tools config-page">
    <el-tabs v-model="activeTab" class="tools-tabs">
      <!-- HTTP 客户端 -->
      <el-tab-pane :label="$t('testTools.httpClient')" name="http">
        <el-card class="tool-card">
          <template #header>
            <div class="card-header">
              <span>{{ $t('testTools.httpClientTitle') }}</span>
            </div>
          </template>

          <el-form :model="httpForm" label-width="100px">
            <el-form-item :label="$t('testTools.method')">
              <el-select v-model="httpForm.method" style="width: 150px;">
                <el-option label="GET" value="GET" />
                <el-option label="POST" value="POST" />
                <el-option label="PUT" value="PUT" />
                <el-option label="DELETE" value="DELETE" />
                <el-option label="PATCH" value="PATCH" />
                <el-option label="HEAD" value="HEAD" />
                <el-option label="OPTIONS" value="OPTIONS" />
              </el-select>
            </el-form-item>

            <el-form-item :label="$t('testTools.url')">
              <el-input v-model="httpForm.url" placeholder="http://localhost:8888/api/test" />
            </el-form-item>

            <el-form-item :label="$t('testTools.headers')">
              <div class="headers-editor">
                <div v-for="(header, index) in httpForm.headers" :key="index" class="header-row">
                  <el-input v-model="header.key" placeholder="Header Name" style="width: 200px;" />
                  <el-input v-model="header.value" placeholder="Header Value" style="flex: 1; margin: 0 8px;" />
                  <el-button @click="removeHeader(index)" type="danger" :icon="Delete" circle />
                </div>
                <el-button @click="addHeader" type="primary" :icon="Plus" size="small">
                  {{ $t('testTools.addHeader') }}
                </el-button>
              </div>
            </el-form-item>

            <el-form-item :label="$t('testTools.body')">
              <el-input
                v-model="httpForm.body"
                type="textarea"
                :rows="6"
                placeholder='{ "key": "value" }'
              />
            </el-form-item>

            <el-form-item :label="$t('testTools.timeout')">
              <el-input-number v-model="httpForm.timeout" :min="1000" :max="60000" :step="1000" />
              <span style="margin-left: 8px;">ms</span>
            </el-form-item>

            <el-form-item :label="$t('testTools.followRedirects')">
              <el-switch v-model="httpForm.followRedirects" />
            </el-form-item>

            <el-form-item>
              <el-button @click="sendHttpRequest" type="primary" :loading="httpLoading" :icon="Promotion">
                {{ $t('testTools.sendRequest') }}
              </el-button>
              <el-button @click="clearHttpResponse" :icon="Delete">
                {{ $t('testTools.clearResponse') }}
              </el-button>
            </el-form-item>
          </el-form>

          <el-divider />

          <div v-if="httpResponse" class="response-section">
            <h4>{{ $t('testTools.response') }}</h4>

            <div v-if="httpResponse.error" class="error-box">
              <el-alert :title="$t('testTools.requestFailed')" type="error" :closable="false">
                {{ httpResponse.error }}
              </el-alert>
            </div>

            <div v-else>
              <el-descriptions :column="2" border>
                <el-descriptions-item :label="$t('testTools.statusCode')">
                  <el-tag :type="getStatusType(httpResponse.status)">
                    {{ httpResponse.status }} {{ httpResponse.status_text }}
                  </el-tag>
                </el-descriptions-item>
                <el-descriptions-item :label="$t('testTools.responseTime')">
                  <el-tag type="info">{{ httpResponse.elapsed_ms }} ms</el-tag>
                </el-descriptions-item>
              </el-descriptions>

              <h5 style="margin-top: 16px;">{{ $t('testTools.responseHeaders') }}</h5>
              <el-table :data="formatHeaders(httpResponse.headers)" border size="small" max-height="200">
                <el-table-column prop="key" :label="$t('testTools.headerName')" width="200" />
                <el-table-column prop="value" :label="$t('testTools.headerValue')" />
              </el-table>

              <h5 style="margin-top: 16px;">{{ $t('testTools.responseBody') }}</h5>
              <el-input
                v-model="httpResponse.body"
                type="textarea"
                :rows="10"
                readonly
                class="response-body"
              />
            </div>
          </div>
        </el-card>
      </el-tab-pane>

      <!-- 路由测试器 -->
      <el-tab-pane :label="$t('testTools.routeTester')" name="route">
        <el-card class="tool-card">
          <template #header>
            <div class="card-header">
              <span>{{ $t('testTools.routeTesterTitle') }}</span>
            </div>
          </template>

          <el-form :model="routeForm" label-width="100px">
            <el-form-item :label="$t('testTools.path')">
              <el-input v-model="routeForm.path" placeholder="/api/users" />
            </el-form-item>

            <el-form-item :label="$t('testTools.method')">
              <el-select v-model="routeForm.method" clearable>
                <el-option label="GET" value="GET" />
                <el-option label="POST" value="POST" />
                <el-option label="PUT" value="PUT" />
                <el-option label="DELETE" value="DELETE" />
              </el-select>
            </el-form-item>

            <el-form-item :label="$t('testTools.host')">
              <el-input v-model="routeForm.host" placeholder="example.com" clearable />
            </el-form-item>

            <el-form-item>
              <el-button @click="testRoute" type="primary" :loading="routeLoading" :icon="Search">
                {{ $t('testTools.testRoute') }}
              </el-button>
            </el-form-item>
          </el-form>

          <el-divider />

          <div v-if="routeResult" class="response-section">
            <h4>{{ $t('testTools.matchResult') }}</h4>

            <el-result
              :icon="routeResult.matched ? 'success' : 'warning'"
              :title="routeResult.matched ? $t('testTools.routeMatched') : $t('testTools.routeNotMatched')"
            >
              <template #extra>
                <el-descriptions v-if="routeResult.matched" :column="1" border>
                  <el-descriptions-item :label="$t('testTools.listenAddr')">
                    {{ routeResult.listen_addr }}
                  </el-descriptions-item>
                  <el-descriptions-item :label="$t('testTools.matchedPath')">
                    {{ routeResult.matched_path }}
                  </el-descriptions-item>
                  <el-descriptions-item :label="$t('testTools.upstreamUrl')">
                    {{ routeResult.upstream_url || 'N/A' }}
                  </el-descriptions-item>
                  <el-descriptions-item :label="$t('testTools.staticDir')">
                    {{ routeResult.static_dir || 'N/A' }}
                  </el-descriptions-item>
                  <el-descriptions-item :label="$t('testTools.sslEnabled')">
                    <el-tag :type="routeResult.ssl_enabled ? 'success' : 'info'">
                      {{ routeResult.ssl_enabled ? $t('common.yes') : $t('common.no') }}
                    </el-tag>
                  </el-descriptions-item>
                  <el-descriptions-item :label="$t('testTools.basicAuthRequired')">
                    <el-tag :type="routeResult.basic_auth_required ? 'warning' : 'info'">
                      {{ routeResult.basic_auth_required ? $t('common.yes') : $t('common.no') }}
                    </el-tag>
                  </el-descriptions-item>
                </el-descriptions>
              </template>
            </el-result>
          </div>
        </el-card>
      </el-tab-pane>

      <!-- 性能测试 -->
      <el-tab-pane :label="$t('testTools.performanceTest')" name="performance">
        <el-card class="tool-card">
          <template #header>
            <div class="card-header">
              <span>{{ $t('testTools.performanceTestTitle') }}</span>
            </div>
          </template>

          <el-alert
            :title="$t('testTools.performanceWarning')"
            type="warning"
            :closable="false"
            style="margin-bottom: 16px;"
          />

          <el-form :model="perfForm" label-width="120px">
            <el-form-item :label="$t('testTools.url')">
              <el-input v-model="perfForm.url" placeholder="http://localhost:8888/api/test" />
            </el-form-item>

            <el-form-item :label="$t('testTools.method')">
              <el-select v-model="perfForm.method">
                <el-option label="GET" value="GET" />
                <el-option label="POST" value="POST" />
              </el-select>
            </el-form-item>

            <el-form-item :label="$t('testTools.concurrent')">
              <el-input-number v-model="perfForm.concurrent" :min="1" :max="100" />
            </el-form-item>

            <el-form-item :label="$t('testTools.duration')">
              <el-input-number v-model="perfForm.duration" :min="1" :max="300" />
              <span style="margin-left: 8px;">{{ $t('testTools.seconds') }}</span>
            </el-form-item>

            <el-form-item>
              <el-button @click="runPerformanceTest" type="primary" :loading="perfLoading" :icon="Timer">
                {{ $t('testTools.startTest') }}
              </el-button>
            </el-form-item>
          </el-form>

          <el-divider />

          <div v-if="perfResult" class="response-section">
            <h4>{{ $t('testTools.testResults') }}</h4>

            <el-row :gutter="16">
              <el-col :span="8">
                <el-statistic :title="$t('testTools.totalRequests')" :value="perfResult.total_requests" />
              </el-col>
              <el-col :span="8">
                <el-statistic :title="$t('testTools.successfulRequests')" :value="perfResult.successful_requests">
                  <template #suffix>
                    <el-tag type="success" size="small">
                      {{ ((perfResult.successful_requests / perfResult.total_requests) * 100).toFixed(2) }}%
                    </el-tag>
                  </template>
                </el-statistic>
              </el-col>
              <el-col :span="8">
                <el-statistic :title="$t('testTools.failedRequests')" :value="perfResult.failed_requests">
                  <template #suffix>
                    <el-tag v-if="perfResult.failed_requests > 0" type="danger" size="small">
                      {{ ((perfResult.failed_requests / perfResult.total_requests) * 100).toFixed(2) }}%
                    </el-tag>
                  </template>
                </el-statistic>
              </el-col>
            </el-row>

            <el-divider />

            <el-row :gutter="16">
              <el-col :span="12">
                <el-statistic :title="$t('testTools.requestsPerSecond')" :value="perfResult.requests_per_second.toFixed(2)" />
              </el-col>
              <el-col :span="12">
                <el-statistic :title="$t('testTools.avgResponseTime')" :value="perfResult.avg_response_time_ms.toFixed(2)">
                  <template #suffix>ms</template>
                </el-statistic>
              </el-col>
            </el-row>

            <el-divider />

            <h5>{{ $t('testTools.responseTimeDistribution') }}</h5>
            <el-descriptions :column="2" border>
              <el-descriptions-item :label="$t('testTools.min')">{{ perfResult.min_response_time_ms }} ms</el-descriptions-item>
              <el-descriptions-item :label="$t('testTools.max')">{{ perfResult.max_response_time_ms }} ms</el-descriptions-item>
              <el-descriptions-item label="P50">{{ perfResult.p50_response_time_ms }} ms</el-descriptions-item>
              <el-descriptions-item label="P95">{{ perfResult.p95_response_time_ms }} ms</el-descriptions-item>
              <el-descriptions-item label="P99" :span="2">{{ perfResult.p99_response_time_ms }} ms</el-descriptions-item>
            </el-descriptions>

            <h5 style="margin-top: 16px;">{{ $t('testTools.statusCodeDistribution') }}</h5>
            <el-table :data="formatStatusCodes(perfResult.status_codes)" border size="small">
              <el-table-column prop="code" :label="$t('testTools.statusCode')" width="150">
                <template #default="{ row }">
                  <el-tag :type="getStatusType(row.code)">{{ row.code }}</el-tag>
                </template>
              </el-table-column>
              <el-table-column prop="count" :label="$t('testTools.count')" />
              <el-table-column prop="percentage" :label="$t('testTools.percentage')" />
            </el-table>
          </div>
        </el-card>
      </el-tab-pane>

      <!-- DNS 查询 -->
      <el-tab-pane :label="$t('testTools.dnsLookup')" name="dns">
        <el-card class="tool-card">
          <template #header>
            <div class="card-header">
              <span>{{ $t('testTools.dnsLookupTitle') }}</span>
            </div>
          </template>

          <el-form :model="dnsForm" label-width="100px">
            <el-form-item :label="$t('testTools.domain')">
              <el-input v-model="dnsForm.domain" placeholder="example.com" />
            </el-form-item>

            <el-form-item>
              <el-button @click="lookupDns" type="primary" :loading="dnsLoading" :icon="Search">
                {{ $t('testTools.lookup') }}
              </el-button>
            </el-form-item>
          </el-form>

          <el-divider />

          <div v-if="dnsResult" class="response-section">
            <div v-if="dnsResult.error">
              <el-alert :title="$t('testTools.lookupFailed')" type="error" :closable="false">
                {{ dnsResult.error }}
              </el-alert>
            </div>
            <div v-else>
              <h5>{{ $t('testTools.ipv4Addresses') }}</h5>
              <el-tag v-for="ip in dnsResult.ipv4_addresses" :key="ip" style="margin: 4px;">{{ ip }}</el-tag>
              <el-empty v-if="dnsResult.ipv4_addresses.length === 0" :description="$t('testTools.noRecords')" />

              <h5 style="margin-top: 16px;">{{ $t('testTools.ipv6Addresses') }}</h5>
              <el-tag v-for="ip in dnsResult.ipv6_addresses" :key="ip" type="info" style="margin: 4px;">{{ ip }}</el-tag>
              <el-empty v-if="dnsResult.ipv6_addresses.length === 0" :description="$t('testTools.noRecords')" />
            </div>
          </div>
        </el-card>
      </el-tab-pane>

      <!-- SSL 证书信息 -->
      <el-tab-pane :label="$t('testTools.sslCertInfo')" name="ssl">
        <el-card class="tool-card">
          <template #header>
            <div class="card-header">
              <span>{{ $t('testTools.sslCertInfoTitle') }}</span>
            </div>
          </template>

          <el-form :model="sslForm" label-width="100px">
            <el-form-item :label="$t('testTools.url')">
              <el-input v-model="sslForm.url" placeholder="https://example.com" />
            </el-form-item>

            <el-form-item>
              <el-button @click="getSslInfo" type="primary" :loading="sslLoading" :icon="Lock">
                {{ $t('testTools.getCertInfo') }}
              </el-button>
            </el-form-item>
          </el-form>

          <el-divider />

          <div v-if="sslResult" class="response-section">
            <div v-if="sslResult.error">
              <el-alert :title="$t('testTools.certCheckFailed')" type="error" :closable="false">
                {{ sslResult.error }}
              </el-alert>
            </div>
            <div v-else>
              <el-descriptions :column="1" border>
                <el-descriptions-item :label="$t('testTools.certStatus')">
                  <el-tag :type="sslResult.valid ? 'success' : 'danger'">
                    {{ sslResult.valid ? $t('testTools.valid') : $t('testTools.invalid') }}
                  </el-tag>
                </el-descriptions-item>
                <el-descriptions-item :label="$t('testTools.subject')">{{ sslResult.subject }}</el-descriptions-item>
                <el-descriptions-item :label="$t('testTools.issuer')">{{ sslResult.issuer }}</el-descriptions-item>
                <el-descriptions-item :label="$t('testTools.notBefore')">{{ sslResult.not_before }}</el-descriptions-item>
                <el-descriptions-item :label="$t('testTools.notAfter')">{{ sslResult.not_after }}</el-descriptions-item>
                <el-descriptions-item :label="$t('testTools.daysUntilExpiry')">
                  <el-tag :type="sslResult.days_until_expiry < 30 ? 'danger' : 'success'">
                    {{ sslResult.days_until_expiry }} {{ $t('testTools.days') }}
                  </el-tag>
                </el-descriptions-item>
              </el-descriptions>
            </div>
          </div>
        </el-card>
      </el-tab-pane>

      <!-- 配置验证 -->
      <el-tab-pane :label="$t('testTools.configValidation')" name="validation">
        <el-card class="tool-card">
          <template #header>
            <div class="card-header">
              <span>{{ $t('testTools.configValidationTitle') }}</span>
            </div>
          </template>

          <el-form :model="validationForm" label-width="150px">
            <el-form-item :label="$t('testTools.checkCertificates')">
              <el-switch v-model="validationForm.check_certificates" />
            </el-form-item>

            <el-form-item :label="$t('testTools.checkUpstreams')">
              <el-switch v-model="validationForm.check_upstreams" />
            </el-form-item>

            <el-form-item :label="$t('testTools.checkPorts')">
              <el-switch v-model="validationForm.check_ports" />
            </el-form-item>

            <el-form-item>
              <el-button @click="validateConfig" type="primary" :loading="validationLoading" :icon="CircleCheck">
                {{ $t('testTools.startValidation') }}
              </el-button>
            </el-form-item>
          </el-form>

          <el-divider />

          <div v-if="validationResult" class="response-section">
            <el-result
              :icon="validationResult.valid ? 'success' : 'error'"
              :title="validationResult.valid ? $t('testTools.configValid') : $t('testTools.configInvalid')"
            >
              <template #extra>
                <div v-if="validationResult.errors.length > 0">
                  <h5>{{ $t('testTools.errors') }}</h5>
                  <el-alert
                    v-for="(error, index) in validationResult.errors"
                    :key="index"
                    :title="error"
                    type="error"
                    :closable="false"
                    style="margin-bottom: 8px;"
                  />
                </div>

                <div v-if="validationResult.warnings.length > 0" style="margin-top: 16px;">
                  <h5>{{ $t('testTools.warnings') }}</h5>
                  <el-alert
                    v-for="(warning, index) in validationResult.warnings"
                    :key="index"
                    :title="warning"
                    type="warning"
                    :closable="false"
                    style="margin-bottom: 8px;"
                  />
                </div>

                <div v-if="validationResult.certificate_checks.length > 0" style="margin-top: 16px;">
                  <h5>{{ $t('testTools.certificateChecks') }}</h5>
                  <el-table :data="validationResult.certificate_checks" border size="small">
                    <el-table-column prop="listen_addr" :label="$t('testTools.listenAddr')" width="150" />
                    <el-table-column prop="cert_file" :label="$t('testTools.certFile')" />
                    <el-table-column :label="$t('testTools.status')" width="100">
                      <template #default="{ row }">
                        <el-tag :type="row.valid ? 'success' : 'danger'">
                          {{ row.valid ? $t('testTools.valid') : $t('testTools.invalid') }}
                        </el-tag>
                      </template>
                    </el-table-column>
                  </el-table>
                </div>

                <div v-if="validationResult.upstream_checks.length > 0" style="margin-top: 16px;">
                  <h5>{{ $t('testTools.upstreamChecks') }}</h5>
                  <el-table :data="validationResult.upstream_checks" border size="small">
                    <el-table-column prop="url" :label="$t('testTools.url')" />
                    <el-table-column :label="$t('testTools.status')" width="100">
                      <template #default="{ row }">
                        <el-tag :type="row.reachable ? 'success' : 'danger'">
                          {{ row.reachable ? $t('testTools.reachable') : $t('testTools.unreachable') }}
                        </el-tag>
                      </template>
                    </el-table-column>
                    <el-table-column prop="response_time_ms" :label="$t('testTools.responseTime')" width="120">
                      <template #default="{ row }">
                        {{ row.response_time_ms ? row.response_time_ms + ' ms' : 'N/A' }}
                      </template>
                    </el-table-column>
                  </el-table>
                </div>

                <div v-if="validationResult.port_checks.length > 0" style="margin-top: 16px;">
                  <h5>{{ $t('testTools.portChecks') }}</h5>
                  <el-table :data="validationResult.port_checks" border size="small">
                    <el-table-column prop="listen_addr" :label="$t('testTools.listenAddr')" />
                    <el-table-column :label="$t('testTools.status')" width="120">
                      <template #default="{ row }">
                        <el-tag :type="row.available ? 'success' : 'danger'">
                          {{ row.available ? $t('testTools.available') : $t('testTools.unavailable') }}
                        </el-tag>
                      </template>
                    </el-table-column>
                  </el-table>
                </div>
              </template>
            </el-result>
          </div>
        </el-card>
      </el-tab-pane>
    </el-tabs>
  </div>
</template>

<script setup lang="ts">
import { ref, reactive } from 'vue'
import { ElMessage } from 'element-plus'
import { Delete, Plus, Promotion, Search, Timer, CircleCheck, Lock } from '@element-plus/icons-vue'
import { invoke } from '@tauri-apps/api/core'
import { useI18n } from 'vue-i18n'

const { t } = useI18n()

const activeTab = ref('http')

// HTTP 客户端
const httpForm = reactive({
  method: 'GET',
  url: 'https://127.0.0.1:8888/',
  headers: [] as Array<{ key: string; value: string }>,
  body: '',
  timeout: 30000,
  followRedirects: true,
})

const httpLoading = ref(false)
const httpResponse = ref<any>(null)

const addHeader = () => {
  httpForm.headers.push({ key: '', value: '' })
}

const removeHeader = (index: number) => {
  httpForm.headers.splice(index, 1)
}

const sendHttpRequest = async () => {
  if (!httpForm.url) {
    ElMessage.warning(t('testTools.pleaseEnterUrl'))
    return
  }

  httpLoading.value = true
  httpResponse.value = null

  try {
    const headers: Record<string, string> = {}
    httpForm.headers.forEach(h => {
      if (h.key && h.value) {
        headers[h.key] = h.value
      }
    })

    const response = await invoke('send_http_test', {
      req: {
        method: httpForm.method,
        url: httpForm.url,
        headers,
        body: httpForm.body || null,
        timeout_ms: httpForm.timeout,
        follow_redirects: httpForm.followRedirects,
      }
    })

    httpResponse.value = response
  } catch (error: any) {
    ElMessage.error(t('testTools.requestFailed') + ': ' + error)
  } finally {
    httpLoading.value = false
  }
}

const clearHttpResponse = () => {
  httpResponse.value = null
}

const formatHeaders = (headers: Record<string, string>) => {
  return Object.entries(headers).map(([key, value]) => ({ key, value }))
}

const getStatusType = (status: number) => {
  if (status >= 200 && status < 300) return 'success'
  if (status >= 300 && status < 400) return 'info'
  if (status >= 400 && status < 500) return 'warning'
  if (status >= 500) return 'danger'
  return 'info'
}

// 路由测试器
const routeForm = reactive({
  path: '/api/test',
  method: 'GET',
  host: '',
})

const routeLoading = ref(false)
const routeResult = ref<any>(null)

const testRoute = async () => {
  if (!routeForm.path) {
    ElMessage.warning(t('testTools.pleaseEnterPath'))
    return
  }

  routeLoading.value = true
  routeResult.value = null

  try {
    const result = await invoke('test_route_match', {
      req: {
        path: routeForm.path,
        method: routeForm.method || null,
        host: routeForm.host || null,
        headers: null,
      }
    })

    routeResult.value = result
  } catch (error: any) {
    ElMessage.error(t('testTools.testFailed') + ': ' + error)
  } finally {
    routeLoading.value = false
  }
}

// 性能测试
const perfForm = reactive({
  url: 'http://localhost:8888/',
  method: 'GET',
  concurrent: 10,
  duration: 10,
})

const perfLoading = ref(false)
const perfResult = ref<any>(null)

const runPerformanceTest = async () => {
  if (!perfForm.url) {
    ElMessage.warning(t('testTools.pleaseEnterUrl'))
    return
  }

  perfLoading.value = true
  perfResult.value = null

  try {
    const result = await invoke('run_performance_test', {
      req: {
        url: perfForm.url,
        method: perfForm.method,
        headers: null,
        body: null,
        concurrent: perfForm.concurrent,
        duration_seconds: perfForm.duration,
      }
    })

    perfResult.value = result
    ElMessage.success(t('testTools.testCompleted'))
  } catch (error: any) {
    ElMessage.error(t('testTools.testFailed') + ': ' + error)
  } finally {
    perfLoading.value = false
  }
}

const formatStatusCodes = (statusCodes: Record<number, number>) => {
  const total = Object.values(statusCodes).reduce((sum, count) => sum + count, 0)
  return Object.entries(statusCodes).map(([code, count]) => ({
    code: parseInt(code),
    count,
    percentage: ((count / total) * 100).toFixed(2) + '%',
  }))
}

// 配置验证
const validationForm = reactive({
  check_certificates: true,
  check_upstreams: true,
  check_ports: true,
})

const validationLoading = ref(false)
const validationResult = ref<any>(null)

const validateConfig = async () => {
  validationLoading.value = true
  validationResult.value = null

  try {
    const result = await invoke('validate_config_tool', {
      req: {
        check_certificates: validationForm.check_certificates,
        check_upstreams: validationForm.check_upstreams,
        check_ports: validationForm.check_ports,
      }
    })

    validationResult.value = result

    if (result.valid) {
      ElMessage.success(t('testTools.validationPassed'))
    } else {
      ElMessage.warning(t('testTools.validationFailed'))
    }
  } catch (error: any) {
    ElMessage.error(t('testTools.validationError') + ': ' + error)
  } finally {
    validationLoading.value = false
  }
}

// DNS 查询
const dnsForm = reactive({
  domain: '',
})

const dnsLoading = ref(false)
const dnsResult = ref<any>(null)

const lookupDns = async () => {
  if (!dnsForm.domain) {
    ElMessage.warning(t('testTools.pleaseEnterDomain'))
    return
  }

  dnsLoading.value = true
  dnsResult.value = null

  try {
    const result = await invoke('dns_lookup', {
      req: { domain: dnsForm.domain }
    })
    dnsResult.value = result
  } catch (error: any) {
    ElMessage.error(t('testTools.lookupFailed') + ': ' + error)
  } finally {
    dnsLoading.value = false
  }
}

// SSL 证书信息
const sslForm = reactive({
  url: 'https://',
})

const sslLoading = ref(false)
const sslResult = ref<any>(null)

const getSslInfo = async () => {
  if (!sslForm.url || !sslForm.url.startsWith('https://')) {
    ElMessage.warning(t('testTools.pleaseEnterHttpsUrl'))
    return
  }

  sslLoading.value = true
  sslResult.value = null

  try {
    const result = await invoke('get_ssl_cert_info', {
      req: { url: sslForm.url }
    })
    sslResult.value = result
  } catch (error: any) {
    ElMessage.error(t('testTools.certCheckFailed') + ': ' + error)
  } finally {
    sslLoading.value = false
  }
}
</script>

<style scoped>
.test-tools {
  height: 100%;
  display: flex;
  flex-direction: column;
  overflow: hidden;
}

.tools-tabs {
  margin-left: 10px;
  flex: 1;
  display: flex;
  flex-direction: column;
  overflow: hidden;
}

.tools-tabs :deep(.el-tabs__content) {
  flex: 1;
  overflow-y: auto;
  padding: 16px;
}

.tool-card {
  margin-bottom: 16px;
}

.card-header {
  display: flex;
  justify-content: space-between;
  align-items: center;
  font-weight: 600;
}

.headers-editor {
  width: 100%;
}

.header-row {
  display: flex;
  align-items: center;
  margin-bottom: 8px;
}

.response-section {
  margin-top: 16px;
}

.response-section h4 {
  margin-bottom: 16px;
  color: var(--text);
}

.response-section h5 {
  margin: 16px 0 8px;
  color: var(--text);
}

.response-body {
  font-family: 'Courier New', monospace;
}

.error-box {
  margin-bottom: 16px;
}

.el-statistic {
  text-align: center;
  padding: 16px;
  background: var(--card-bg);
  border-radius: var(--radius-md);
  border: 1px solid var(--border);
}
</style>
