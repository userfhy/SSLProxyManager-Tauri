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
                <div style="margin-top: 8px;">
                  <el-button @click="addHeader" type="primary" :icon="Plus" size="small">
                    {{ $t('testTools.addHeader') }}
                  </el-button>
                  <el-button @click="addCommonHeaders" :icon="DocumentCopy" size="small">
                    {{ $t('testTools.addCommonHeaders') }}
                  </el-button>
                </div>
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
              <h5 style="margin-top: 0;">{{ $t('testTools.requestHeaders') }}</h5>
              <el-table :data="formatHeaders(httpResponse.request_headers || {})" border size="small" max-height="200">
                <el-table-column prop="key" :label="$t('testTools.headerName')" width="200" />
                <el-table-column prop="value" :label="$t('testTools.headerValue')" />
              </el-table>

              <el-descriptions :column="2" border style="margin-top: 16px;">
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
              <div style="margin-bottom: 8px;">
                <el-button @click="previewHtml" size="small" :icon="View">
                  {{ $t('testTools.previewHtml') }}
                </el-button>
              </div>
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

              <el-form-item :label="$t('testTools.listenAddrFilter')">
                <el-input v-model="routeForm.listenAddr" placeholder=":8888" clearable />
              </el-form-item>

              <el-form-item :label="$t('testTools.headersJson')">
                <el-input
                  v-model="routeForm.headersJson"
                  type="textarea"
                  :rows="4"
                  placeholder='{"x-env":"prod","x-version":"v1"}'
                />
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

        <!-- 场景回归（Route Suite） -->
        <el-tab-pane :label="$t('testTools.routeSuite')" name="routeSuite">
          <el-card class="tool-card">
            <template #header>
              <div class="card-header">
                <span>{{ $t('testTools.routeSuiteTitle') }}</span>
              </div>
            </template>

            <el-form :model="routeSuiteForm" label-width="130px">
              <el-form-item :label="$t('testTools.stopOnFailure')">
                <el-switch v-model="routeSuiteForm.stopOnFailure" />
              </el-form-item>

              <el-form-item :label="$t('testTools.casesJson')">
                <el-input
                  v-model="routeSuiteForm.casesJson"
                  type="textarea"
                  :rows="14"
                  :placeholder="$t('testTools.casesJsonHint')"
                />
              </el-form-item>

              <el-form-item>
                <el-button @click="runRouteSuite" type="primary" :loading="routeSuiteLoading" :icon="Search">
                  {{ $t('testTools.runSuite') }}
                </el-button>
                <el-button @click="loadRouteSuiteExample" :icon="DocumentCopy">
                  {{ $t('testTools.loadExample') }}
                </el-button>
              </el-form-item>
            </el-form>

            <el-divider />

            <div v-if="routeSuiteResult" class="response-section">
              <h4>{{ $t('testTools.suiteResults') }}</h4>
              <el-row :gutter="16">
                <el-col :span="8">
                  <el-statistic :title="$t('testTools.totalRequests')" :value="routeSuiteResult.total_cases" />
                </el-col>
                <el-col :span="8">
                  <el-statistic :title="$t('testTools.passed')" :value="routeSuiteResult.passed_cases" />
                </el-col>
                <el-col :span="8">
                  <el-statistic :title="$t('testTools.failed')" :value="routeSuiteResult.failed_cases" />
                </el-col>
              </el-row>
              <p style="margin-top: 12px;">{{ $t('testTools.totalTime') }}: {{ routeSuiteResult.elapsed_ms }} ms</p>

              <el-table :data="routeSuiteResult.cases" border size="small">
                <el-table-column prop="name" :label="$t('testTools.caseName')" width="180" />
                <el-table-column :label="$t('testTools.assertionResult')" width="120">
                  <template #default="{ row }">
                    <el-tag :type="row.passed ? 'success' : 'danger'">
                      {{ row.passed ? $t('testTools.passed') : $t('testTools.failed') }}
                    </el-tag>
                  </template>
                </el-table-column>
                <el-table-column prop="failure_reason" :label="$t('testTools.failureReason')" />
                <el-table-column :label="$t('testTools.actualRoute')">
                  <template #default="{ row }">
                    {{ row.actual?.listen_addr || '-' }} / {{ row.actual?.route_id || '-' }}
                  </template>
                </el-table-column>
                <el-table-column prop="elapsed_ms" :label="$t('testTools.responseTime')" width="120">
                  <template #default="{ row }">
                    {{ row.elapsed_ms }} ms
                  </template>
                </el-table-column>
              </el-table>
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

          <el-divider />

          <h4>{{ $t('testTools.selfSignedCertTitle') }}</h4>
          <el-form :model="selfSignedForm" label-width="130px">
            <el-form-item :label="$t('testTools.commonName')">
              <el-input v-model="selfSignedForm.commonName" placeholder="localhost" />
            </el-form-item>

            <el-form-item :label="$t('testTools.organization')">
              <el-input v-model="selfSignedForm.organization" :placeholder="$t('testTools.organizationHint')" />
            </el-form-item>

            <el-form-item :label="$t('testTools.organizationalUnit')">
              <el-input v-model="selfSignedForm.organizationalUnit" :placeholder="$t('testTools.organizationalUnitHint')" />
            </el-form-item>

            <el-form-item :label="$t('testTools.subjectAltNames')">
              <el-input
                v-model="selfSignedForm.subjectAltNames"
                :placeholder="$t('testTools.subjectAltNamesHint')"
              />
            </el-form-item>

            <el-form-item :label="$t('testTools.validDays')">
              <el-input-number v-model="selfSignedForm.validDays" :min="1" :max="3650" />
            </el-form-item>

            <el-form-item :label="$t('testTools.outputDir')">
              <div class="file-selector">
                <el-input
                  v-model="selfSignedForm.outputDir"
                  :placeholder="$t('testTools.outputDirHint')"
                  readonly
                />
                <el-button @click="selectSelfSignedOutputDir" :icon="Folder">
                  {{ $t('testTools.selectDir') }}
                </el-button>
              </div>
            </el-form-item>

            <el-form-item>
              <el-button @click="generateSelfSignedCert" type="primary" :loading="selfSignedLoading" :icon="Lock">
                {{ $t('testTools.generateSelfSignedCert') }}
              </el-button>
            </el-form-item>
          </el-form>

          <div v-if="selfSignedResult" class="response-section">
            <el-alert :title="$t('testTools.generateSuccess')" type="success" :closable="false" />
            <el-descriptions :column="1" border style="margin-top: 12px;">
              <el-descriptions-item :label="$t('testTools.generatedCertFile')">
                {{ selfSignedResult.cert_file }}
              </el-descriptions-item>
              <el-descriptions-item :label="$t('testTools.generatedKeyFile')">
                {{ selfSignedResult.key_file }}
              </el-descriptions-item>
            </el-descriptions>
          </div>
        </el-card>
      </el-tab-pane>

      <!-- 端口扫描 -->
      <el-tab-pane :label="$t('testTools.portScan')" name="portscan">
        <el-card class="tool-card">
          <template #header>
            <div class="card-header">
              <span>{{ $t('testTools.portScanTitle') }}</span>
            </div>
          </template>

          <el-form :model="portScanForm" label-width="100px">
            <el-form-item :label="$t('testTools.host')">
              <el-input v-model="portScanForm.host" placeholder="127.0.0.1" />
            </el-form-item>

            <el-form-item :label="$t('testTools.ports')">
              <el-input v-model="portScanForm.portsInput" placeholder="80,443,3306,8080" />
              <div style="margin-top: 8px;">
                <el-button @click="setCommonPorts" size="small">{{ $t('testTools.commonPorts') }}</el-button>
                <el-button @click="setWebPorts" size="small">{{ $t('testTools.webPorts') }}</el-button>
                <el-button @click="setDbPorts" size="small">{{ $t('testTools.dbPorts') }}</el-button>
              </div>
            </el-form-item>

            <el-form-item :label="$t('testTools.timeout')">
              <el-input-number v-model="portScanForm.timeout" :min="100" :max="5000" :step="100" />
              <span style="margin-left: 8px;">ms</span>
            </el-form-item>

            <el-form-item>
              <el-button @click="scanPorts" type="primary" :loading="portScanLoading" :icon="Search">
                {{ $t('testTools.startScan') }}
              </el-button>
            </el-form-item>
          </el-form>

          <el-divider />

          <div v-if="portScanResult" class="response-section">
            <h4>{{ $t('testTools.scanResults') }}</h4>
            <p>{{ $t('testTools.totalTime') }}: {{ portScanResult.total_time_ms }} ms</p>
            <el-table :data="portScanResult.results" border size="small">
              <el-table-column prop="port" :label="$t('testTools.port')" width="100" />
              <el-table-column :label="$t('testTools.status')" width="100">
                <template #default="{ row }">
                  <el-tag :type="row.open ? 'success' : 'info'">
                    {{ row.open ? $t('testTools.open') : $t('testTools.closed') }}
                  </el-tag>
                </template>
              </el-table-column>
              <el-table-column prop="service" :label="$t('testTools.service')">
                <template #default="{ row }">
                  {{ row.service || '-' }}
                </template>
              </el-table-column>
            </el-table>
          </div>
        </el-card>
      </el-tab-pane>

      <!-- 编码/解码工具 -->
      <el-tab-pane :label="$t('testTools.encodeDecode')" name="encode">
        <el-card class="tool-card">
          <template #header>
            <div class="card-header">
              <span>{{ $t('testTools.encodeDecodeTitle') }}</span>
            </div>
          </template>

          <el-form :model="encodeForm" label-width="100px">
            <el-form-item :label="$t('testTools.operation')">
              <el-select v-model="encodeForm.operation">
                <el-option label="Base64 编码" value="base64_encode" />
                <el-option label="Base64 解码" value="base64_decode" />
                <el-option label="URL 编码" value="url_encode" />
                <el-option label="URL 解码" value="url_decode" />
                <el-option label="Hex 编码" value="hex_encode" />
                <el-option label="Hex 解码" value="hex_decode" />
              </el-select>
            </el-form-item>

            <el-form-item :label="$t('testTools.input')">
              <el-input v-model="encodeForm.input" type="textarea" :rows="6" placeholder="输入要编码/解码的文本" />
            </el-form-item>

            <el-form-item>
              <el-button @click="doEncodeDecode" type="primary" :icon="Promotion">
                {{ $t('testTools.execute') }}
              </el-button>
              <el-button @click="copyOutput" :icon="DocumentCopy">
                {{ $t('testTools.copyOutput') }}
              </el-button>
            </el-form-item>
          </el-form>

          <el-divider />

          <div v-if="encodeResult" class="response-section">
            <div v-if="encodeResult.error">
              <el-alert :title="$t('testTools.operationFailed')" type="error" :closable="false">
                {{ encodeResult.error }}
              </el-alert>
            </div>
            <div v-else>
              <h5>{{ $t('testTools.output') }}</h5>
              <el-input v-model="encodeResult.output" type="textarea" :rows="10" readonly class="response-body" />
            </div>
          </div>
        </el-card>
      </el-tab-pane>

      <!-- WebSocket 测试 -->
      <el-tab-pane :label="$t('testTools.websocket')" name="websocket">
        <el-card class="tool-card">
          <template #header>
            <div class="card-header">
              <span>{{ $t('testTools.websocketTitle') }}</span>
              <el-tag :type="wsStatusType">{{ wsStatusText }}</el-tag>
            </div>
          </template>

          <el-form :model="wsForm" label-width="100px">
            <el-form-item :label="$t('testTools.url')">
              <el-input v-model="wsForm.url" placeholder="ws://localhost:8800/ws" :disabled="wsConnected || wsConnecting" />
            </el-form-item>

            <el-form-item :label="$t('testTools.connectionOptions')">
              <div class="ws-options-grid">
                <el-switch v-model="wsForm.autoReconnect" :active-text="$t('testTools.autoReconnect')" />
                <div class="ws-inline-item">
                  <span class="ws-inline-label">{{ $t('testTools.reconnectIntervalMs') }}</span>
                  <el-input-number v-model="wsForm.reconnectIntervalMs" :min="200" :max="60000" :step="100" />
                </div>
                <div class="ws-inline-item">
                  <span class="ws-inline-label">{{ $t('testTools.maxReconnectAttempts') }}</span>
                  <el-input-number v-model="wsForm.maxReconnectAttempts" :min="1" :max="100" />
                </div>
                <div class="ws-inline-item">
                  <span class="ws-inline-label">{{ $t('testTools.connectTimeoutMs') }}</span>
                  <el-input-number v-model="wsForm.connectTimeoutMs" :min="1000" :max="120000" :step="500" />
                </div>
              </div>
            </el-form-item>

            <el-form-item>
              <el-button v-if="!wsConnected" @click="connectWebSocket" type="primary" :icon="Connection" :loading="wsConnecting">
                {{ $t('testTools.connect') }}
              </el-button>
              <el-button v-else @click="disconnectWebSocket" type="danger" :icon="Close">
                {{ $t('testTools.disconnect') }}
              </el-button>
              <el-button v-if="wsPeriodicSending" @click="stopPeriodicSend" type="warning">
                {{ $t('testTools.stopPeriodicSend') }}
              </el-button>
              <el-button @click="clearWsMessages" :icon="Delete">
                {{ $t('testTools.clearMessages') }}
              </el-button>
              <el-button @click="copyWsLogs" :icon="DocumentCopy">
                {{ $t('testTools.copyLogs') }}
              </el-button>
            </el-form-item>

            <el-form-item :label="$t('testTools.message')" v-if="wsConnected">
              <el-input v-model="wsForm.message" type="textarea" :rows="3" placeholder="输入要发送的消息" />
            </el-form-item>

            <el-form-item v-if="wsConnected">
              <el-button @click="sendWsMessage" type="primary" :icon="Promotion">
                {{ $t('testTools.sendMessage') }}
              </el-button>
              <el-button @click="sendWsPing">
                {{ $t('testTools.sendPing') }}
              </el-button>
              <div class="ws-inline-item ws-inline-item-tight">
                <span class="ws-inline-label">{{ $t('testTools.sendIntervalMs') }}</span>
                <el-input-number v-model="wsForm.sendIntervalMs" :min="100" :max="60000" :step="100" />
              </div>
              <el-button @click="startPeriodicSend" :disabled="wsPeriodicSending">
                {{ $t('testTools.startPeriodicSend') }}
              </el-button>
            </el-form-item>
          </el-form>

          <el-divider />

          <div class="response-section">
            <div style="display: flex; justify-content: space-between; align-items: center; margin-bottom: 12px;">
              <h4 style="margin: 0;">{{ $t('testTools.messageLog') }} ({{ filteredWsMessages.length }}/{{ wsMessages.length }})</h4>
              <el-switch v-model="wsShowHtml" :active-text="$t('testTools.renderHtml')" />
            </div>

            <div class="ws-filter-bar">
              <el-select v-model="wsFilterType" style="width: 160px;">
                <el-option :label="$t('testTools.filterAll')" value="all" />
                <el-option :label="$t('testTools.filterSent')" value="sent" />
                <el-option :label="$t('testTools.filterReceived')" value="received" />
                <el-option :label="$t('testTools.filterSystem')" value="system" />
              </el-select>
              <el-input v-model="wsKeyword" :placeholder="$t('testTools.filterKeyword')" clearable />
            </div>

            <div class="ws-messages">
              <div v-for="(msg, index) in filteredWsMessages" :key="index" class="ws-message" :class="msg.type">
                <span class="ws-time">{{ msg.time }}</span>
                <span class="ws-type">{{ wsTypeLabel(msg.type) }}</span>
                <span class="ws-size">{{ msg.bytes }}B</span>
                <span v-if="wsShowHtml" class="ws-content" v-html="msg.content"></span>
                <span v-else class="ws-content">{{ msg.content }}</span>
              </div>
              <el-empty v-if="filteredWsMessages.length === 0" :description="$t('testTools.noMessages')" />
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

    <!-- HTML 预览对话框 -->
    <el-dialog v-model="htmlPreviewVisible" :title="$t('testTools.htmlPreview')" width="80%" top="5vh">
      <iframe :srcdoc="htmlPreviewContent" style="width: 100%; height: 70vh; border: 1px solid var(--border); border-radius: 4px;"></iframe>
    </el-dialog>
  </div>
</template>

<script setup lang="ts">
import { ref, reactive, computed, nextTick, onBeforeUnmount } from 'vue'
import { ElMessage } from 'element-plus'
import { Delete, Plus, Promotion, Search, Timer, CircleCheck, Lock, DocumentCopy, Connection, Close, View, Folder } from '@element-plus/icons-vue'
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
const htmlPreviewVisible = ref(false)
const htmlPreviewContent = ref('')

const addHeader = () => {
  httpForm.headers.push({ key: '', value: '' })
}

const removeHeader = (index: number) => {
  httpForm.headers.splice(index, 1)
}

const addCommonHeaders = () => {
  const commonHeaders = [
    { key: 'User-Agent', value: 'Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36' },
    { key: 'Accept', value: 'application/json, text/plain, */*' },
    { key: 'Content-Type', value: 'application/json' },
    { key: 'Accept-Language', value: 'zh-CN,zh;q=0.9,en;q=0.8' },
  ]
  httpForm.headers.push(...commonHeaders)
  ElMessage.success(t('testTools.commonHeadersAdded'))
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

    // 添加请求头信息到响应中
    httpResponse.value = {
      ...response,
      request_headers: headers
    }
  } catch (error: any) {
    ElMessage.error(t('testTools.requestFailed') + ': ' + error)
  } finally {
    httpLoading.value = false
  }
}

const clearHttpResponse = () => {
  httpResponse.value = null
}

const previewHtml = async () => {
  if (!httpResponse.value || !httpResponse.value.body) {
    ElMessage.warning(t('testTools.noContentToPreview'))
    return
  }
  htmlPreviewContent.value = ''
  htmlPreviewVisible.value = true
  await nextTick()
  htmlPreviewContent.value = httpResponse.value.body
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
  listenAddr: '',
  headersJson: '',
})

const routeLoading = ref(false)
const routeResult = ref<any>(null)

const testRoute = async () => {
  if (!routeForm.path) {
    ElMessage.warning(t('testTools.pleaseEnterPath'))
    return
  }

  let parsedHeaders: Record<string, string> | null = null
  if (routeForm.headersJson.trim()) {
    try {
      const obj = JSON.parse(routeForm.headersJson)
      if (!obj || typeof obj !== 'object' || Array.isArray(obj)) {
        ElMessage.warning(t('testTools.pleaseEnterValidHeadersJson'))
        return
      }
      parsedHeaders = Object.entries(obj).reduce((acc, [key, value]) => {
        acc[String(key)] = String(value)
        return acc
      }, {} as Record<string, string>)
    } catch {
      ElMessage.warning(t('testTools.pleaseEnterValidHeadersJson'))
      return
    }
  }

  routeLoading.value = true
  routeResult.value = null

  try {
    const result = await invoke('test_route_match', {
      req: {
        path: routeForm.path,
        method: routeForm.method || null,
        host: routeForm.host || null,
        headers: parsedHeaders,
        listen_addr: routeForm.listenAddr || null,
      }
    })

    routeResult.value = result
  } catch (error: any) {
    ElMessage.error(t('testTools.testFailed') + ': ' + error)
  } finally {
    routeLoading.value = false
  }
}

// 场景回归（Route Suite）
const routeSuiteForm = reactive({
  stopOnFailure: false,
  casesJson: '',
})

const routeSuiteLoading = ref(false)
const routeSuiteResult = ref<any>(null)

const loadRouteSuiteExample = () => {
  const example = [
    {
      name: 'api-prefix-prod-host',
      path: '/api/users',
      method: 'GET',
      host: 'api.example.com',
      headers: { 'x-env': 'prod' },
      listen_addr: null,
      expect_matched: true,
      expect_listen_rule_id: null,
      expect_route_id: null,
      expect_listen_addr: null,
    },
    {
      name: 'unmatched-route',
      path: '/not-exists',
      method: 'GET',
      host: 'example.com',
      headers: {},
      listen_addr: null,
      expect_matched: false,
      expect_listen_rule_id: null,
      expect_route_id: null,
      expect_listen_addr: null,
    },
  ]
  routeSuiteForm.casesJson = JSON.stringify(example, null, 2)
}

const runRouteSuite = async () => {
  let cases: any[] = []
  try {
    const parsed = JSON.parse(routeSuiteForm.casesJson || '[]')
    if (!Array.isArray(parsed)) {
      ElMessage.warning(t('testTools.pleaseEnterValidCasesJson'))
      return
    }
    cases = parsed
  } catch {
    ElMessage.warning(t('testTools.pleaseEnterValidCasesJson'))
    return
  }

  if (cases.length === 0) {
    ElMessage.warning(t('testTools.pleaseEnterValidCasesJson'))
    return
  }

  routeSuiteLoading.value = true
  routeSuiteResult.value = null
  try {
    const result = await invoke('run_route_test_suite', {
      req: {
        cases,
        stop_on_failure: routeSuiteForm.stopOnFailure,
      }
    })
    routeSuiteResult.value = result
    ElMessage.success(t('testTools.suiteCompleted'))
  } catch (error: any) {
    ElMessage.error(t('testTools.suiteFailed') + ': ' + error)
  } finally {
    routeSuiteLoading.value = false
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
const selfSignedLoading = ref(false)
const selfSignedResult = ref<any>(null)
const selfSignedForm = reactive({
  commonName: 'localhost',
  organization: '',
  organizationalUnit: '',
  subjectAltNames: 'localhost,127.0.0.1',
  validDays: 365,
  outputDir: '',
})

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

const selectSelfSignedOutputDir = async () => {
  try {
    const selected = await invoke('open_directory_dialog')
    if (selected) {
      selfSignedForm.outputDir = String(selected)
    }
  } catch (error: any) {
    ElMessage.error(t('testTools.generateFailed') + ': ' + error)
  }
}

const generateSelfSignedCert = async () => {
  if (!selfSignedForm.commonName.trim()) {
    ElMessage.warning(t('testTools.pleaseEnterCommonName'))
    return
  }

  selfSignedLoading.value = true
  selfSignedResult.value = null

  try {
    const subjectAltNames = selfSignedForm.subjectAltNames
      .split(',')
      .map(s => s.trim())
      .filter(Boolean)

    const result = await invoke('generate_self_signed_cert', {
      req: {
        common_name: selfSignedForm.commonName.trim(),
        organization: selfSignedForm.organization.trim() || null,
        organizational_unit: selfSignedForm.organizationalUnit.trim() || null,
        subject_alt_names: subjectAltNames.length > 0 ? subjectAltNames : null,
        valid_days: selfSignedForm.validDays,
        output_dir: selfSignedForm.outputDir.trim() || null,
      }
    })

    selfSignedResult.value = result
    ElMessage.success(t('testTools.generateSuccess'))
  } catch (error: any) {
    ElMessage.error(t('testTools.generateFailed') + ': ' + error)
  } finally {
    selfSignedLoading.value = false
  }
}

// 端口扫描
const portScanForm = reactive({
  host: '127.0.0.1',
  portsInput: '80,443,3306,8080',
  timeout: 1000,
})

const portScanLoading = ref(false)
const portScanResult = ref<any>(null)

const setCommonPorts = () => {
  portScanForm.portsInput = '21,22,23,25,53,80,110,143,443,3306,3389,5432,6379,8080,8888,27017'
}

const setWebPorts = () => {
  portScanForm.portsInput = '80,443,8000,8080,8443,8888,9000'
}

const setDbPorts = () => {
  portScanForm.portsInput = '3306,5432,6379,27017,1433,5984'
}

const scanPorts = async () => {
  if (!portScanForm.host) {
    ElMessage.warning(t('testTools.pleaseEnterHost'))
    return
  }

  const ports = portScanForm.portsInput.split(',').map(p => parseInt(p.trim())).filter(p => !isNaN(p) && p > 0 && p <= 65535)
  if (ports.length === 0) {
    ElMessage.warning(t('testTools.pleaseEnterValidPorts'))
    return
  }

  portScanLoading.value = true
  portScanResult.value = null

  try {
    const result = await invoke('scan_ports', {
      req: {
        host: portScanForm.host,
        ports,
        timeout_ms: portScanForm.timeout,
      }
    })
    portScanResult.value = result
    ElMessage.success(t('testTools.scanCompleted'))
  } catch (error: any) {
    ElMessage.error(t('testTools.scanFailed') + ': ' + error)
  } finally {
    portScanLoading.value = false
  }
}

// 编码/解码
const encodeForm = reactive({
  operation: 'base64_encode',
  input: '',
})

const encodeResult = ref<any>(null)

const doEncodeDecode = async () => {
  if (!encodeForm.input) {
    ElMessage.warning(t('testTools.pleaseEnterInput'))
    return
  }

  try {
    const result = await invoke('encode_decode', {
      req: {
        operation: encodeForm.operation,
        input: encodeForm.input,
      }
    })
    encodeResult.value = result
  } catch (error: any) {
    ElMessage.error(t('testTools.operationFailed') + ': ' + error)
  }
}

const copyOutput = () => {
  if (!encodeResult.value || !encodeResult.value.output) {
    ElMessage.warning(t('testTools.noOutputToCopy'))
    return
  }
  navigator.clipboard.writeText(encodeResult.value.output)
  ElMessage.success(t('testTools.copiedToClipboard'))
}

// WebSocket 测试
const wsForm = reactive({
  url: 'ws://localhost:8800/ws',
  message: '',
  autoReconnect: true,
  reconnectIntervalMs: 2000,
  maxReconnectAttempts: 10,
  connectTimeoutMs: 8000,
  sendIntervalMs: 1000,
})

const wsConnected = ref(false)
const wsConnecting = ref(false)
const wsShowHtml = ref(false)
const wsFilterType = ref<'all' | 'sent' | 'received' | 'system'>('all')
const wsKeyword = ref('')
const wsReconnectAttempts = ref(0)
const wsPeriodicSending = ref(false)
const wsMessages = ref<Array<{ time: string; type: 'sent' | 'received' | 'system'; content: string; bytes: number }>>([])
const WS_MAX_MESSAGES = 1000
let wsClient: WebSocket | null = null
let wsCloseByUser = false
let wsReconnectTimer: ReturnType<typeof setTimeout> | null = null
let wsConnectTimeoutTimer: ReturnType<typeof setTimeout> | null = null
let wsPeriodicSendTimer: ReturnType<typeof setInterval> | null = null

const wsStatusType = computed(() => {
  if (wsConnected.value) return 'success'
  if (wsConnecting.value) return 'warning'
  return 'info'
})

const wsStatusText = computed(() => {
  if (wsConnected.value) return t('testTools.connected')
  if (wsConnecting.value) {
    if (wsReconnectAttempts.value > 0) {
      return t('testTools.reconnectingWithCount', { count: wsReconnectAttempts.value })
    }
    return t('testTools.connecting')
  }
  return t('testTools.disconnected')
})

const filteredWsMessages = computed(() => {
  const keyword = wsKeyword.value.trim().toLowerCase()
  return wsMessages.value.filter(msg => {
    if (wsFilterType.value !== 'all' && msg.type !== wsFilterType.value) {
      return false
    }
    if (!keyword) {
      return true
    }
    return msg.content.toLowerCase().includes(keyword)
  })
})

const clearWsReconnectTimer = () => {
  if (wsReconnectTimer) {
    clearTimeout(wsReconnectTimer)
    wsReconnectTimer = null
  }
}

const clearWsConnectTimeoutTimer = () => {
  if (wsConnectTimeoutTimer) {
    clearTimeout(wsConnectTimeoutTimer)
    wsConnectTimeoutTimer = null
  }
}

const stopPeriodicSend = (showToast = true) => {
  if (wsPeriodicSendTimer) {
    clearInterval(wsPeriodicSendTimer)
    wsPeriodicSendTimer = null
  }
  if (wsPeriodicSending.value && showToast) {
    ElMessage.info(t('testTools.periodicSendStopped'))
  }
  wsPeriodicSending.value = false
}

const wsTypeLabel = (type: 'sent' | 'received' | 'system') => {
  if (type === 'sent') return t('testTools.filterSent')
  if (type === 'received') return t('testTools.filterReceived')
  return t('testTools.filterSystem')
}

const textByteLength = (content: string) => {
  return new TextEncoder().encode(content).length
}

const scheduleReconnect = () => {
  if (wsCloseByUser || !wsForm.autoReconnect) return
  if (wsReconnectAttempts.value >= wsForm.maxReconnectAttempts) {
    addWsMessage('system', t('testTools.reconnectExhausted'), textByteLength(t('testTools.reconnectExhausted')))
    return
  }
  wsReconnectAttempts.value += 1
  const delay = Math.max(200, wsForm.reconnectIntervalMs)
  addWsMessage('system', t('testTools.reconnectScheduled', { count: wsReconnectAttempts.value, delay }), 0)
  clearWsReconnectTimer()
  wsReconnectTimer = setTimeout(() => {
    openWebSocket(true)
  }, delay)
}

const openWebSocket = (isReconnect = false) => {
  if (!wsForm.url) {
    ElMessage.warning(t('testTools.pleaseEnterUrl'))
    return
  }
  if (wsClient && (wsClient.readyState === WebSocket.OPEN || wsClient.readyState === WebSocket.CONNECTING)) {
    return
  }

  wsConnecting.value = true
  if (!isReconnect) {
    wsReconnectAttempts.value = 0
  }

  try {
    wsClient = new WebSocket(wsForm.url)

    clearWsConnectTimeoutTimer()
    wsConnectTimeoutTimer = setTimeout(() => {
      if (wsClient && wsClient.readyState === WebSocket.CONNECTING) {
        addWsMessage('system', t('testTools.connectTimeout'), textByteLength(t('testTools.connectTimeout')))
        wsClient.close(4000, 'connect timeout')
      }
    }, Math.max(1000, wsForm.connectTimeoutMs))

    wsClient.onopen = () => {
      clearWsConnectTimeoutTimer()
      wsConnected.value = true
      wsConnecting.value = false
      wsReconnectAttempts.value = 0
      addWsMessage('system', t('testTools.connectionEstablished'), textByteLength(t('testTools.connectionEstablished')))
      ElMessage.success(t('testTools.connected'))
    }

    wsClient.onmessage = (event) => {
      if (typeof event.data === 'string') {
        addWsMessage('received', event.data, textByteLength(event.data))
      } else if (event.data instanceof Blob) {
        addWsMessage('received', `[Blob ${event.data.size} bytes]`, event.data.size)
      } else if (event.data instanceof ArrayBuffer) {
        addWsMessage('received', `[ArrayBuffer ${event.data.byteLength} bytes]`, event.data.byteLength)
      } else {
        addWsMessage('received', String(event.data), textByteLength(String(event.data)))
      }
    }

    wsClient.onerror = () => {
      addWsMessage('system', t('testTools.connectionError'), textByteLength(t('testTools.connectionError')))
    }

    wsClient.onclose = (event) => {
      clearWsConnectTimeoutTimer()
      wsConnected.value = false
      wsConnecting.value = false
      stopPeriodicSend(false)
      wsClient = null

      const reasonText = event.reason ? ` (${event.reason})` : ''
      addWsMessage('system', `${t('testTools.connectionClosed')} [${event.code}]${reasonText}`, 0)

      if (!wsCloseByUser) {
        scheduleReconnect()
      }
      wsCloseByUser = false
    }
  } catch (error: any) {
    wsConnected.value = false
    wsConnecting.value = false
    clearWsConnectTimeoutTimer()
    ElMessage.error(t('testTools.connectionFailed') + ': ' + error)
    if (!wsCloseByUser) {
      scheduleReconnect()
    }
  }
}

const connectWebSocket = () => {
  wsCloseByUser = false
  clearWsReconnectTimer()
  openWebSocket(false)
}

const disconnectWebSocket = () => {
  wsCloseByUser = true
  wsConnecting.value = false
  clearWsReconnectTimer()
  clearWsConnectTimeoutTimer()
  stopPeriodicSend(false)
  if (wsClient) {
    wsClient.close()
    wsClient = null
  }
  wsConnected.value = false
}

const sendWsMessage = () => {
  if (!wsClient || !wsConnected.value || wsClient.readyState !== WebSocket.OPEN) {
    ElMessage.warning(t('testTools.notConnected'))
    return
  }
  if (!wsForm.message) {
    ElMessage.warning(t('testTools.pleaseEnterMessage'))
    return
  }

  try {
    wsClient.send(wsForm.message)
    addWsMessage('sent', wsForm.message, textByteLength(wsForm.message))
    wsForm.message = ''
  } catch (error: any) {
    ElMessage.error(t('testTools.sendFailed') + ': ' + error)
  }
}

const sendWsPing = () => {
  if (!wsClient || !wsConnected.value || wsClient.readyState !== WebSocket.OPEN) {
    ElMessage.warning(t('testTools.notConnected'))
    return
  }
  const ping = 'ping'
  try {
    wsClient.send(ping)
    addWsMessage('sent', ping, textByteLength(ping))
  } catch (error: any) {
    ElMessage.error(t('testTools.sendFailed') + ': ' + error)
  }
}

const startPeriodicSend = () => {
  if (!wsClient || !wsConnected.value || wsClient.readyState !== WebSocket.OPEN) {
    ElMessage.warning(t('testTools.notConnected'))
    return
  }
  if (!wsForm.message) {
    ElMessage.warning(t('testTools.pleaseEnterMessage'))
    return
  }
  stopPeriodicSend(false)
  const interval = Math.max(100, wsForm.sendIntervalMs)
  wsPeriodicSending.value = true
  wsPeriodicSendTimer = setInterval(() => {
    if (!wsClient || !wsConnected.value || wsClient.readyState !== WebSocket.OPEN) {
      stopPeriodicSend(false)
      return
    }
    const message = wsForm.message
    wsClient.send(message)
    addWsMessage('sent', message, textByteLength(message))
  }, interval)
  ElMessage.success(t('testTools.periodicSendStarted'))
}

const addWsMessage = (type: 'sent' | 'received' | 'system', content: string, bytes: number) => {
  const now = new Date()
  const time = `${now.getHours().toString().padStart(2, '0')}:${now.getMinutes().toString().padStart(2, '0')}:${now.getSeconds().toString().padStart(2, '0')}.${now.getMilliseconds().toString().padStart(3, '0')}`
  wsMessages.value.push({ time, type, content, bytes })
  if (wsMessages.value.length > WS_MAX_MESSAGES) {
    wsMessages.value.splice(0, wsMessages.value.length - WS_MAX_MESSAGES)
  }
}

const clearWsMessages = () => {
  wsMessages.value = []
}

const copyWsLogs = async () => {
  const lines = filteredWsMessages.value.map(msg => `[${msg.time}] [${wsTypeLabel(msg.type)}] [${msg.bytes}B] ${msg.content}`)
  if (lines.length === 0) {
    ElMessage.warning(t('testTools.noMessages'))
    return
  }
  await navigator.clipboard.writeText(lines.join('\n'))
  ElMessage.success(t('testTools.copiedToClipboard'))
}

onBeforeUnmount(() => {
  clearWsReconnectTimer()
  clearWsConnectTimeoutTimer()
  stopPeriodicSend(false)
  disconnectWebSocket()
})
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

.file-selector {
  display: flex;
  width: 100%;
  gap: 8px;
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

.ws-messages {
  max-height: 400px;
  overflow-y: auto;
  border: 1px solid var(--border);
  border-radius: var(--radius-sm);
  padding: 12px;
  background: var(--card-bg);
}

.ws-message {
  padding: 8px;
  margin-bottom: 8px;
  border-radius: var(--radius-sm);
  font-family: 'Courier New', monospace;
  font-size: 13px;
  display: flex;
  gap: 8px;
}

.ws-options-grid {
  width: 100%;
  display: grid;
  grid-template-columns: repeat(auto-fit, minmax(220px, 1fr));
  gap: 8px 12px;
}

.ws-inline-item {
  display: flex;
  align-items: center;
  gap: 8px;
}

.ws-inline-item-tight {
  margin-left: 8px;
}

.ws-inline-label {
  white-space: nowrap;
  color: var(--text-secondary);
  font-size: 12px;
}

.ws-filter-bar {
  display: flex;
  gap: 8px;
  margin-bottom: 12px;
}

.ws-message.sent {
  background: #e3f2fd;
  border-left: 3px solid #2196f3;
}

.ws-message.received {
  background: #e8f5e9;
  border-left: 3px solid #4caf50;
}

.ws-message.system {
  background: #fff3e0;
  border-left: 3px solid #ff9800;
}

.ws-time {
  color: #666;
  min-width: 60px;
}

.ws-type {
  font-weight: bold;
  min-width: 40px;
}

.ws-size {
  color: var(--text-secondary);
  min-width: 52px;
  text-align: right;
}

.ws-content {
  flex: 1;
  word-break: break-all;
}

</style>
