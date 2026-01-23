<template>
  <el-card class="config-card config-page" shadow="hover">
    <template #header>
      <div class="header-row">
        <h3>Stream 代理配置</h3>
        <div class="header-actions">
          <el-switch v-model="enabled" active-text="启用" inactive-text="禁用" />
        </div>
      </div>
    </template>

    <el-alert
      type="info"
      :closable="false"
      title="说明"
      description="Stream 为四层(TCP/UDP)转发：监听端口 -> upstream。hash_key 目前仅支持 $remote_addr（客户端 IP）。consistent=true 时启用一致性哈希环（更接近 nginx: hash $remote_addr consistent）。"
      style="margin-bottom: 12px;"
    />

    <el-card shadow="never" class="section-card">
      <template #header>
        <div class="section-header">
          <span>Upstreams</span>
          <el-button type="primary" size="small" @click="addUpstream">
            <el-icon><Plus /></el-icon>
            添加 Upstream
          </el-button>
        </div>
      </template>

      <div v-if="upstreams.length === 0" class="empty-hint">
        暂无 upstream，请先添加。
      </div>

      <TransitionGroup name="list" tag="div">
      <el-card v-for="(up, upIndex) in upstreams" :key="up.id || upIndex" shadow="never" class="sub-card">
        <template #header>
          <div class="sub-header">
            <span>Upstream {{ upIndex + 1 }}</span>
            <el-button type="danger" size="small" @click="removeUpstream(upIndex)">
              删除
            </el-button>
          </div>
        </template>

        <el-form :model="up" label-width="140px">
          <el-form-item label="name" required>
            <el-input v-model="up.name" placeholder="sendimage" style="max-width: 360px;" />
          </el-form-item>

          <el-form-item label="hash_key">
            <el-input v-model="up.hash_key" placeholder="$remote_addr" style="max-width: 360px;" />
          </el-form-item>

          <el-form-item label="consistent">
            <el-switch v-model="up.consistent" />
          </el-form-item>
        </el-form>

        <el-card shadow="never" class="inner-card">
          <template #header>
            <div class="section-header">
              <span>Servers</span>
              <el-button type="primary" size="small" @click="addUpstreamServer(upIndex)">
                <el-icon><Plus /></el-icon>
                添加 Server
              </el-button>
            </div>
          </template>

          <TransitionGroup name="list" tag="div">
            <div v-for="(sv, svIndex) in up.servers" :key="sv.id || svIndex" class="server-row">
              <el-row :gutter="10">
                <el-col :span="10">
                  <el-input v-model="sv.addr" placeholder="59.227.155.134:8089" />
                </el-col>
                <el-col :span="4">
                  <el-input-number v-model="sv.weight" :min="1" />
                </el-col>
                <el-col :span="4">
                  <el-input-number v-model="sv.max_fails" :min="0" />
                </el-col>
                <el-col :span="4">
                  <el-input v-model="sv.fail_timeout" placeholder="30s" />
                </el-col>
                <el-col :span="2">
                  <el-button type="danger" size="small" :disabled="up.servers.length <= 1" @click="removeUpstreamServer(upIndex, svIndex)">
                    删除
                  </el-button>
                </el-col>
              </el-row>
            </div>
          </TransitionGroup>

          <el-text type="info" size="small" class="mini-hint">
            字段说明：addr=host:port；weight 当前不参与 hash；max_fails/fail_timeout 已用于 TCP 连接失败熔断。
          </el-text>
        </el-card>
              </el-card>
      </TransitionGroup>
    </el-card>

    <el-card shadow="never" class="section-card">
      <template #header>
        <div class="section-header">
          <span>Servers（监听端口）</span>
          <el-button type="primary" size="small" @click="addServer">
            <el-icon><Plus /></el-icon>
            添加监听
          </el-button>
        </div>
      </template>

      <div v-if="servers.length === 0" class="empty-hint">
        暂无 server，请先添加。
      </div>

      <TransitionGroup name="list" tag="div">
        <el-card v-for="(sv, sIndex) in servers" :key="sv.id || sIndex" shadow="never" class="sub-card">
        <template #header>
          <div class="sub-header">
            <span>Server {{ sIndex + 1 }}</span>
            <el-button type="danger" size="small" @click="removeServer(sIndex)">
              删除
            </el-button>
          </div>
        </template>

        <el-form :model="sv" label-width="200px">
          <el-form-item label="启用">
            <el-switch v-model="sv.enabled" />
          </el-form-item>

          <el-form-item label="listen_port" required>
            <el-input-number v-model="sv.listen_port" :min="1" :max="65535" />
          </el-form-item>

          <el-form-item label="udp">
            <el-switch v-model="sv.udp" active-text="UDP" inactive-text="TCP" />
          </el-form-item>

          <el-form-item label="proxy_pass（upstream）" required>
            <el-input v-model="sv.proxy_pass" placeholder="sendimage" style="max-width: 200px;" />
          </el-form-item>

          <el-form-item label="proxy_connect_timeout">
            <el-input v-model="sv.proxy_connect_timeout" placeholder="300s" style="max-width: 200px;" />
          </el-form-item>

          <el-form-item label="proxy_timeout">
            <el-input v-model="sv.proxy_timeout" placeholder="600s" style="max-width: 200px;" />
          </el-form-item>
        </el-form>
        </el-card>
      </TransitionGroup>
    </el-card>
  </el-card>
</template>

<script setup lang="ts">
import { ref, onMounted } from 'vue'
import { GetConfig } from '../api'
import { Plus } from '@element-plus/icons-vue'

interface StreamUpstreamServer {
  id?: string
  addr: string
  weight: number
  max_fails: number
  fail_timeout: string
}

interface StreamUpstream {
  id?: string
  name: string
  hash_key: string
  consistent: boolean
  servers: StreamUpstreamServer[]
}

interface StreamServer {
  id?: string
  enabled: boolean
  listen_port: number
  proxy_pass: string
  proxy_connect_timeout: string
  proxy_timeout: string
  udp: boolean
}

const enabled = ref(false)
const upstreams = ref<StreamUpstream[]>([])
const servers = ref<StreamServer[]>([])

const defaultUpstreamServer = (): StreamUpstreamServer => ({
  id: `new-server-${Date.now()}`,
  addr: '',
  weight: 1,
  max_fails: 1,
  fail_timeout: '30s',
})

const defaultUpstream = (): StreamUpstream => ({
  id: `new-upstream-${Date.now()}`,
  name: '',
  hash_key: '$remote_addr',
  consistent: true,
  servers: [defaultUpstreamServer()],
})

const defaultServer = (): StreamServer => ({
  id: `new-server-${Date.now()}`,
  enabled: true,
  listen_port: 50002,
  proxy_pass: '',
  proxy_connect_timeout: '300s',
  proxy_timeout: '600s',
  udp: false,
})

onMounted(async () => {
  const cfg = (await GetConfig()) as any
  const stream = cfg?.stream || {}

  enabled.value = !!stream.enabled

  upstreams.value = Array.isArray(stream.upstreams)
    ? stream.upstreams.map((u: any) => ({
        name: u?.name || '',
        hash_key: u?.hash_key || '$remote_addr',
        consistent: u?.consistent !== false,
        servers:
          Array.isArray(u?.servers) && u.servers.length > 0
            ? u.servers.map((s: any) => ({
                addr: s?.addr || '',
                weight: Number(s?.weight ?? 1),
                max_fails: Number(s?.max_fails ?? 1),
                fail_timeout: String(s?.fail_timeout ?? '30s'),
              }))
            : [defaultUpstreamServer()],
      }))
    : []

  servers.value = Array.isArray(stream.servers)
    ? stream.servers.map((s: any) => ({
        enabled: s?.enabled !== false,
        listen_port: Number(s?.listen_port ?? 50002),
        proxy_pass: s?.proxy_pass || '',
        proxy_connect_timeout: String(s?.proxy_connect_timeout ?? '300s'),
        proxy_timeout: String(s?.proxy_timeout ?? '600s'),
        udp: !!s?.udp,
      }))
    : []
})

const addUpstream = () => {
  upstreams.value.push(defaultUpstream())
}

const removeUpstream = (idx: number) => {
  upstreams.value.splice(idx, 1)
}

const addUpstreamServer = (upIndex: number) => {
  upstreams.value[upIndex].servers.push(defaultUpstreamServer())
}

const removeUpstreamServer = (upIndex: number, svIndex: number) => {
  const list = upstreams.value[upIndex].servers
  if (list.length <= 1) return
  list.splice(svIndex, 1)
}

const addServer = () => {
  servers.value.push(defaultServer())
}

const removeServer = (idx: number) => {
  servers.value.splice(idx, 1)
}

const isValidHostPort = (v: string): boolean => {
  const s = (v || '').trim()
  if (!s) return false

  // 简单校验：允许域名 / IPv4；IPv6（带冒号）不做严格支持（可后续增强）
  const idx = s.lastIndexOf(':')
  if (idx <= 0 || idx === s.length - 1) return false
  const host = s.slice(0, idx).trim()
  const portStr = s.slice(idx + 1).trim()
  if (!host) return false
  const port = Number(portStr)
  return Number.isInteger(port) && port >= 1 && port <= 65535
}

const getConfig = () => {
  // 更严格校验（TCP 优先）
  const cleanedUpstreams = upstreams.value.map((u) => ({
    name: (u.name || '').trim(),
    hash_key: (u.hash_key || '$remote_addr').trim() || '$remote_addr',
    consistent: !!u.consistent,
    servers: (u.servers || [])
      .map((s) => ({
        addr: (s.addr || '').trim(),
        weight: Number(s.weight || 1),
        max_fails: Number(s.max_fails ?? 1),
        fail_timeout: (s.fail_timeout || '30s').trim() || '30s',
      }))
      .filter((s) => s.addr !== ''),
  }))

  const cleanedServers = servers.value.map((s) => ({
    enabled: !!s.enabled,
    listen_port: Number(s.listen_port || 0),
    proxy_pass: (s.proxy_pass || '').trim(),
    proxy_connect_timeout: (s.proxy_connect_timeout || '300s').trim() || '300s',
    proxy_timeout: (s.proxy_timeout || '600s').trim() || '600s',
    udp: !!s.udp,
  }))

  // 仅当启用 stream 时做强校验
  if (enabled.value) {
    // 1) upstream name
    if (cleanedUpstreams.length === 0) {
      throw new Error('Stream：请至少配置一个 upstream')
    }
    const names = new Set<string>()
    for (let i = 0; i < cleanedUpstreams.length; i++) {
      const u = cleanedUpstreams[i]
      if (!u.name) {
        throw new Error(`Stream Upstream ${i + 1}：name 不能为空`)
      }
      if (names.has(u.name)) {
        throw new Error(`Stream：upstream name 重复：${u.name}`)
      }
      names.add(u.name)

      if (!u.servers || u.servers.length === 0) {
        throw new Error(`Stream Upstream ${i + 1}（${u.name}）：请至少配置一个 server addr`) 
      }
      for (let j = 0; j < u.servers.length; j++) {
        const sv = u.servers[j]
        if (!sv.addr) {
          throw new Error(`Stream Upstream ${i + 1}（${u.name}）/ Server ${j + 1}：addr 不能为空`) 
        }
        if (!isValidHostPort(sv.addr)) {
          throw new Error(`Stream Upstream ${i + 1}（${u.name}）/ Server ${j + 1}：addr 格式错误（需要 host:port）：${sv.addr}`)
        }
      }
    }

    // 2) listen servers
    if (cleanedServers.length === 0) {
      throw new Error('Stream：请至少配置一个监听 server')
    }

    const usedPorts = new Set<string>()
    for (let i = 0; i < cleanedServers.length; i++) {
      const sv = cleanedServers[i]
      if (!sv.enabled) {
        continue
      }
      if (!sv.listen_port || sv.listen_port < 1 || sv.listen_port > 65535) {
        throw new Error(`Stream Server ${i + 1}：listen_port 必须为 1-65535`) 
      }
      if (!sv.proxy_pass) {
        throw new Error(`Stream Server ${i + 1}：proxy_pass 不能为空`) 
      }
      if (!names.has(sv.proxy_pass)) {
        throw new Error(`Stream Server ${i + 1}：proxy_pass 引用了不存在的 upstream：${sv.proxy_pass}`)
      }

      const portKey = `${sv.listen_port}/${sv.udp ? 'udp' : 'tcp'}`
      if (usedPorts.has(portKey)) {
        throw new Error(`Stream：监听端口冲突：${portKey}`)
      }
      usedPorts.add(portKey)

      // TCP 优先：如果是 TCP，强校验 timeout 字符串是否像 "300s/5m/1h" 这种
      if (!sv.udp) {
        const okTimeout = (v: string) => /^\d+\s*[smh]?$/.test((v || '').trim())
        if (!okTimeout(sv.proxy_connect_timeout)) {
          throw new Error(`Stream Server ${i + 1}：proxy_connect_timeout 格式不正确：${sv.proxy_connect_timeout}`)
        }
        if (!okTimeout(sv.proxy_timeout)) {
          throw new Error(`Stream Server ${i + 1}：proxy_timeout 格式不正确：${sv.proxy_timeout}`)
        }
      }
    }
  }

  return {
    stream: {
      enabled: !!enabled.value,
      upstreams: cleanedUpstreams,
      servers: cleanedServers,
    },
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

.section-card {
  border-radius: var(--radius-lg);
  margin-bottom: 24px;
  border: 1px solid var(--border);
  background: var(--card-bg);
}

.section-header {
  display: flex;
  justify-content: space-between;
  align-items: center;
  font-size: 18px;
  font-weight: 600;
  color: var(--text);
}

.sub-card {
  border-radius: var(--radius-md);
  margin-bottom: 16px;
  background: var(--input-bg);
  border: 1px solid var(--border);
}

.sub-header {
  display: flex;
  justify-content: space-between;
  align-items: center;
  font-size: 16px;
  font-weight: 600;
}

.inner-card {
  border-radius: var(--radius-md);
  margin-top: 16px;
  background: var(--card-bg);
  border: 1px solid var(--border);
}

.server-row {
  margin-bottom: 12px;
}

.empty-hint {
  color: var(--text-muted);
  padding: 16px;
  text-align: center;
}

.mini-hint {
  display: block;
  margin-top: 8px;
  font-size: 12px;
  color: var(--text-muted);
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
