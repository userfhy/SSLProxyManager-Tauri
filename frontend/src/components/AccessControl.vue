<template>
  <el-card class="config-card config-page" shadow="hover">
    <template #header>
      <h3>访问控制</h3>
    </template>

    <el-form label-width="180px">
      <el-form-item>
        <el-checkbox v-model="localConfig.allow_all_lan">
          允许所有局域网 IP 访问
        </el-checkbox>
        <el-text type="info" size="small" class="mini-hint">
          勾选后，所有局域网地址（如 192.168.x.x, 10.x.x.x）都将被允许访问，白名单依然有效。
        </el-text>
      </el-form-item>

      <el-form-item label="IP 白名单">
        <div class="whitelist-list">
          <div v-for="(item, index) in localConfig.whitelist" :key="index" class="whitelist-item">
            <el-input
              v-model="item.ip"
              placeholder="例如: 192.168.1.100"
              class="whitelist-input"
            />
            <el-button @click="removeWhitelistItem(index)" type="danger" size="small">删除</el-button>
          </div>
        </div>
        <el-button @click="addWhitelistItem" type="primary" style="margin-top: 10px;">
          <el-icon><Plus /></el-icon> 添加 IP 地址
        </el-button>
      </el-form-item>

      <el-divider style="margin: 16px 0;" />

      <div class="blacklist-header">
        <h4>IP 黑名单</h4>
        <div class="blacklist-actions">
          <el-button type="primary" @click="showAddDialog = true" :icon="Plus">
            添加黑名单
          </el-button>
          <el-button @click="refreshBlacklist" :loading="blacklistLoading" :icon="Refresh">
            刷新列表
          </el-button>
          <el-button @click="refreshCache" :loading="refreshingCache" :icon="RefreshRight">
            刷新缓存
          </el-button>
        </div>
      </div>

      <el-alert
        v-if="dbStatus && !dbStatus.enabled"
        title="数据库未启用"
        type="warning"
        :closable="false"
        show-icon
        style="margin-bottom: 12px;"
      >
        <template #default>
          <div>
            <p>黑名单功能需要启用数据持久化才能使用。</p>
            <p>请前往 <strong>\"数据持久化\"</strong> 标签页启用数据库功能。</p>
          </div>
        </template>
      </el-alert>

      <el-table
        :data="blacklist"
        v-loading="blacklistLoading"
        stripe
        border
        style="width: 100%; margin-top: 12px;"
      >
        <el-table-column prop="ip" label="IP地址" width="180" sortable />
        <el-table-column prop="reason" label="拉黑原因" min-width="200" show-overflow-tooltip />
        <el-table-column prop="expires_at" label="过期时间" width="180" sortable>
          <template #default="{ row }">
            <span v-if="row.expires_at === 0">永久</span>
            <span v-else>
              {{ formatTime(row.expires_at) }}
              <el-tag
                v-if="isExpired(row.expires_at)"
                type="danger"
                size="small"
                style="margin-left: 8px;"
              >
                已过期
              </el-tag>
              <el-tag
                v-else
                type="success"
                size="small"
                style="margin-left: 8px;"
              >
                有效
              </el-tag>
            </span>
          </template>
        </el-table-column>
        <el-table-column prop="created_at" label="创建时间" width="180" sortable>
          <template #default="{ row }">
            {{ formatTime(row.created_at) }}
          </template>
        </el-table-column>
        <el-table-column label="操作" width="120" fixed="right">
          <template #default="{ row }">
            <el-button
              type="danger"
              size="small"
              @click="handleRemove(row.ip)"
              :icon="Delete"
            >
              删除
            </el-button>
          </template>
        </el-table-column>
      </el-table>

      <el-dialog
        v-model="showAddDialog"
        title="添加黑名单"
        width="520px"
        :close-on-click-modal="false"
      >
        <el-form :model="addForm" label-width="120px" :rules="addRules" ref="addFormRef">
          <el-form-item label="IP地址" prop="ip">
            <el-input
              v-model="addForm.ip"
              placeholder="请输入IP地址，例如：192.168.1.1"
              clearable
            />
            <el-text type="info" size="small" class="hint">
              支持IPv4和IPv6地址
            </el-text>
          </el-form-item>

          <el-form-item label="拉黑原因" prop="reason">
            <el-input
              v-model="addForm.reason"
              type="textarea"
              :rows="3"
              placeholder="请输入拉黑原因（可选）"
              clearable
            />
          </el-form-item>

          <el-form-item label="拉黑时长">
            <el-radio-group v-model="addForm.durationType">
              <el-radio label="permanent">永久</el-radio>
              <el-radio label="temporary">临时</el-radio>
            </el-radio-group>
          </el-form-item>

          <el-form-item
            v-if="addForm.durationType === 'temporary'"
            label="过期时间"
            prop="expiresAt"
          >
            <el-config-provider :locale="zhCn">
              <el-date-picker
                v-model="addForm.expiresAt"
                type="datetime"
                format="YYYY-MM-DD HH:mm:ss"
                value-format="x"
                :shortcuts="dateShortcuts"
                placeholder="选择过期日期和时间"
                :disabled-date="disabledDate"
                style="width: 100%;"
              />
            </el-config-provider>
            <el-text type="info" size="small" class="hint">
              只允许选择未来时间
            </el-text>
          </el-form-item>
        </el-form>

        <template #footer>
          <el-button @click="showAddDialog = false">取消</el-button>
          <el-button type="primary" @click="handleAdd" :loading="adding">确定</el-button>
        </template>
      </el-dialog>
    </el-form>
  </el-card>
</template>

<script setup lang="ts">
import { ref, watch, onMounted } from 'vue'
import { Plus, Refresh, RefreshRight, Delete } from '@element-plus/icons-vue'
import { ElMessage, ElMessageBox, ElConfigProvider } from 'element-plus'
import zhCn from 'element-plus/dist/locale/zh-cn.mjs'
// @ts-ignore
import { AddBlacklistEntry, RemoveBlacklistEntry, GetBlacklistEntries, RefreshBlacklistCache, GetMetricsDBStatus } from '../api'

interface BlacklistEntry {
  id: number
  ip: string
  reason?: string | null
  expires_at: number
  created_at: number
}

const props = defineProps<{ config: any }>()

const localConfig = ref({
  allow_all_lan: true,
  whitelist: [] as { ip: string }[],
})

// 黑名单状态
const blacklist = ref<BlacklistEntry[]>([])
const blacklistLoading = ref(false)
const refreshingCache = ref(false)
const adding = ref(false)
const showAddDialog = ref(false)
const addFormRef = ref()
const dbStatus = ref<any>(null)

const addForm = ref({
  ip: '',
  reason: '',
  durationType: 'permanent' as 'permanent' | 'temporary',
  // el-date-picker value-format="x" 返回毫秒时间戳字符串
  expiresAt: null as string | null,
})

const dateShortcuts = [
  {
    text: '1小时',
    value: () => new Date(Date.now() + 1 * 60 * 60 * 1000),
  },
  {
    text: '6小时',
    value: () => new Date(Date.now() + 6 * 60 * 60 * 1000),
  },
  {
    text: '1天',
    value: () => new Date(Date.now() + 24 * 60 * 60 * 1000),
  },
  {
    text: '1个月',
    value: () => {
      const d = new Date()
      d.setMonth(d.getMonth() + 1)
      return d
    },
  },
  {
    text: '6个月',
    value: () => {
      const d = new Date()
      d.setMonth(d.getMonth() + 6)
      return d
    },
  },
  {
    text: '1年',
    value: () => {
      const d = new Date()
      d.setFullYear(d.getFullYear() + 1)
      return d
    },
  },
]

const addRules = {
  ip: [
    { required: true, message: '请输入IP地址', trigger: 'blur' },
    {
      validator: (rule: any, value: string, callback: Function) => {
        if (!value) {
          callback(new Error('请输入IP地址'))
          return
        }
        const ipv4Regex = /^(\d{1,3}\.){3}\d{1,3}$/
        const ipv6Regex = /^([0-9a-fA-F]{0,4}:){2,7}[0-9a-fA-F]{0,4}$/
        if (!ipv4Regex.test(value) && !ipv6Regex.test(value)) {
          callback(new Error('请输入有效的IP地址'))
          return
        }
        callback()
      },
      trigger: 'blur',
    },
  ],
  expiresAt: [
    {
      validator: (rule: any, value: any, callback: Function) => {
        if (addForm.value.durationType === 'permanent') {
          callback()
          return
        }
        if (!value) {
          callback(new Error('请选择过期时间'))
          return
        }
        const ms = Number(value)
        if (!Number.isFinite(ms)) {
          callback(new Error('过期时间格式无效'))
          return
        }
        if (ms <= Date.now()) {
          callback(new Error('过期时间必须大于当前时间'))
          return
        }
        callback()
      },
      trigger: 'change',
    },
  ],
}

const disabledDate = (time: Date) => {
  // 禁用今天之前的日期（今天可以选，但还会被 expiresAt 校验限制必须大于当前时间）
  return time.getTime() < new Date().setHours(0, 0, 0, 0)
}

watch(
  () => props.config,
  (newConfig) => {
    if (newConfig) {
      localConfig.value.allow_all_lan = newConfig.allow_all_lan ?? true
      localConfig.value.whitelist = Array.isArray(newConfig.whitelist) ? [...newConfig.whitelist] : []
    }
  },
  { immediate: true, deep: true }
)

const addWhitelistItem = () => {
  localConfig.value.whitelist.push({ ip: '' })
}

const removeWhitelistItem = (index: number) => {
  localConfig.value.whitelist.splice(index, 1)
}

const formatTime = (timestamp: number) => {
  if (!timestamp) return '-'
  const date = new Date(timestamp * 1000)
  return date.toLocaleString('zh-CN', {
    year: 'numeric',
    month: '2-digit',
    day: '2-digit',
    hour: '2-digit',
    minute: '2-digit',
    second: '2-digit',
  })
}

const isExpired = (expiresAt: number) => {
  if (expiresAt === 0) return false
  return Date.now() / 1000 > expiresAt
}

const refreshBlacklist = async () => {
  blacklistLoading.value = true
  try {
    // @ts-ignore
    const entries = await GetBlacklistEntries()
    blacklist.value = entries || []
  } catch (error: any) {
    console.error('获取黑名单失败:', error)
    ElMessage.error('获取黑名单失败: ' + (error.message || String(error)))
  } finally {
    blacklistLoading.value = false
  }
}

const refreshCache = async () => {
  refreshingCache.value = true
  try {
    // @ts-ignore
    await RefreshBlacklistCache()
    ElMessage.success('缓存已刷新')
    await refreshBlacklist()
  } catch (error: any) {
    console.error('刷新缓存失败:', error)
    ElMessage.error('刷新缓存失败: ' + (error.message || String(error)))
  } finally {
    refreshingCache.value = false
  }
}

const handleAdd = async () => {
  if (!addFormRef.value) {
    ElMessage.warning('表单未初始化')
    return
  }

  try {
    // @ts-ignore
    dbStatus.value = await GetMetricsDBStatus()
  } catch {
    dbStatus.value = null
  }

  if (!dbStatus.value || !dbStatus.value.enabled) {
    ElMessage.error({
      message: '数据库未启用！请先在"数据持久化"标签页中启用数据库功能。',
      duration: 5000,
      showClose: true,
    })
    return
  }

  if (!dbStatus.value.initialized) {
    ElMessage.error({
      message: '数据库未初始化！请检查数据库配置和路径。',
      duration: 5000,
      showClose: true,
    })
    return
  }

  try {
    await addFormRef.value.validate()
  } catch {
    ElMessage.warning('请检查表单输入')
    return
  }

  adding.value = true
  try {
    let durationSeconds = 0
    if (addForm.value.durationType === 'temporary') {
      const ms = Number(addForm.value.expiresAt)
      const now = Date.now()
      durationSeconds = Math.ceil((ms - now) / 1000)
      if (durationSeconds <= 0) {
        ElMessage.warning('过期时间必须大于当前时间')
        adding.value = false
        return
      }
    }

    // @ts-ignore
    await AddBlacklistEntry(addForm.value.ip, addForm.value.reason || '', durationSeconds)
    ElMessage.success('黑名单已添加')
    showAddDialog.value = false

    addForm.value = {
      ip: '',
      reason: '',
      durationType: 'permanent',
      expiresAt: null,
    }

    if (addFormRef.value) {
      addFormRef.value.resetFields()
    }

    await refreshBlacklist()
  } catch (error: any) {
    console.error('添加黑名单失败:', error)
    ElMessage.error({
      message: '添加黑名单失败: ' + (error?.message || String(error)),
      duration: 5000,
      showClose: true,
    })
  } finally {
    adding.value = false
  }
}

const handleRemove = async (ip: string) => {
  try {
    await ElMessageBox.confirm(
      `确定要删除IP "${ip}" 的黑名单记录吗？`,
      '确认删除',
      {
        confirmButtonText: '确定',
        cancelButtonText: '取消',
        type: 'warning',
      }
    )

    // @ts-ignore
    await RemoveBlacklistEntry(ip)
    ElMessage.success('黑名单已删除')
    await refreshBlacklist()
  } catch (error: any) {
    if (error !== 'cancel') {
      console.error('删除黑名单失败:', error)
      ElMessage.error('删除黑名单失败: ' + (error.message || String(error)))
    }
  }
}

onMounted(async () => {
  try {
    // @ts-ignore
    dbStatus.value = await GetMetricsDBStatus()
  } catch {
    dbStatus.value = null
  }
  await refreshBlacklist()
})

// 供父组件调用
const getConfig = () => {
  return {
    allow_all_lan: localConfig.value.allow_all_lan,
    whitelist: localConfig.value.whitelist.filter((item) => item.ip.trim() !== ''),
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

.whitelist-list {
  display: grid;
  grid-template-columns: repeat(auto-fill, minmax(260px, 1fr));
  gap: 12px;
  width: 100%;
}

.whitelist-item {
  display: grid;
  grid-template-columns: 1fr auto;
  gap: 10px;
  align-items: center;
}

.whitelist-input {
  font-family: 'JetBrains Mono', 'Consolas', monospace;
}

.blacklist-header {
  display: flex;
  justify-content: space-between;
  align-items: center;
  margin-top: 8px;
}

.blacklist-header h4 {
  margin: 0;
  font-size: 16px;
  font-weight: 600;
  color: var(--text);
}

.blacklist-actions {
  display: flex;
  gap: 10px;
}

.hint {
  margin-left: 12px;
  font-size: 12px;
  display: block;
  margin-top: 4px;
}
</style>
