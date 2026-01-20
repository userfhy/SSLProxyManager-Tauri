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
      description="Stream 为四层(TCP/UDP)转发：监听端口 -> upstream。hash_key 目前仅支持 $remote_addr（客户端 IP），其它值会回退轮询。"
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

      <el-card v-for="(up, upIndex) in upstreams" :key="upIndex" shadow="never" class="sub-card">
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

          <div v-for="(sv, svIndex) in up.servers" :key="svIndex" class="server-row">
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

          <el-text type="info" size="small" class="mini-hint">
            字段说明：addr=host:port；weight 暂不参与 hash；max_fails/fail_timeout 暂不生效（后续可增强）。
          </el-text>
        </el-card>
      </el-card>
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

      <el-card v-for="(sv, sIndex) in servers" :key="sIndex" shadow="never" class="sub-card">
        <template #header>
          <div class="sub-header">
            <span>Server {{ sIndex + 1 }}</span>
            <el-button type="danger" size="small" @click="removeServer(sIndex)">
              删除
            </el-button>
          </div>
        </template>

        <el-form :model="sv" label-width="200px">
          <el-form-item label="enabled">
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
    </el-card>
  </el-card>
</template>

<script setup lang="ts">
import { ref, onMounted } from 'vue'
import { GetConfig } from '../api'
import { Plus } from '@element-plus/icons-vue'

interface StreamUpstreamServer {
  addr: string
  weight: number
  max_fails: number
  fail_timeout: string
}

interface StreamUpstream {
  name: string
  hash_key: string
  consistent: boolean
  servers: StreamUpstreamServer[]
}

interface StreamServer {
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
  addr: '',
  weight: 1,
  max_fails: 1,
  fail_timeout: '30s',
})

const defaultUpstream = (): StreamUpstream => ({
  name: '',
  hash_key: '$remote_addr',
  consistent: true,
  servers: [defaultUpstreamServer()],
})

const defaultServer = (): StreamServer => ({
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
        servers: Array.isArray(u?.servers) && u.servers.length > 0
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

const getConfig = () => {
  // 基础校验（只做必填项，避免阻塞；更严格校验可按需加）
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
.config-card {
  height: 100%;
  overflow-y: auto;
  border-radius: 20px;
  backdrop-filter: blur(10px);
}

.header-row {
  display: flex;
  justify-content: space-between;
  align-items: center;
}

.section-card {
  border-radius: 12px;
  margin-bottom: 12px;
}

.sub-card {
  border-radius: 12px;
  margin-bottom: 10px;
}

.inner-card {
  border-radius: 12px;
}

.section-header {
  display: flex;
  justify-content: space-between;
  align-items: center;
}

.sub-header {
  display: flex;
  justify-content: space-between;
  align-items: center;
}

.server-row {
  margin-bottom: 10px;
}

.empty-hint {
  color: var(--text-muted);
  padding: 10px 0;
}

.mini-hint {
  display: block;
  margin-top: 8px;
}
</style>
