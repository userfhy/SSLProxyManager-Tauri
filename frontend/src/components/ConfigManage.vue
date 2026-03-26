<template>
  <el-card class="config-card config-page" shadow="hover">
    <template #header>
      <h3>{{ $t('configManage.title') }}</h3>
    </template>

    <div class="config-manage-content">
      <el-form :model="alertForm" label-width="180px">
        <el-form-item :label="$t('about.alertingEnabled')">
          <el-switch v-model="alertForm.enabled" />
        </el-form-item>

        <template v-if="alertForm.enabled">
          <el-form-item :label="$t('about.alertWebhookEnabled')">
            <el-switch v-model="alertForm.webhook.enabled" />
          </el-form-item>

          <template v-if="alertForm.webhook.enabled">
            <el-form-item :label="$t('about.alertProvider')">
              <el-select v-model="alertForm.webhook.provider" style="width: 220px;">
                <el-option label="企业微信 WeCom" value="wecom" />
                <el-option label="飞书 Feishu" value="feishu" />
              </el-select>
            </el-form-item>

            <el-form-item :label="$t('about.alertWebhookUrl')">
              <el-input v-model="alertForm.webhook.url" :placeholder="$t('about.alertWebhookUrlPlaceholder')" />
            </el-form-item>
          </template>

          <el-form-item :label="$t('about.alertRuleServerStartError')">
            <el-switch v-model="alertForm.rules.server_start_error" />
          </el-form-item>

          <el-form-item>
            <el-button type="primary" @click="handleSendTestAlert" :loading="sendingTestAlert">
              {{ $t('about.sendTestAlert') }}
            </el-button>
          </el-form-item>
        </template>
      </el-form>

      <el-divider />

      <el-form label-width="180px">
        <el-form-item :label="$t('about.configSnapshots')">
          <el-button @click="loadSnapshots" :loading="loadingSnapshots">{{ $t('about.refreshSnapshots') }}</el-button>
        </el-form-item>

        <el-form-item>
          <el-table :data="snapshotList" style="width: 100%" size="small" v-loading="loadingSnapshots">
            <el-table-column prop="name" :label="$t('about.snapshotName')" min-width="240" />
            <el-table-column :label="$t('about.snapshotTime')" min-width="180">
              <template #default="scope">
                {{ formatTs(scope.row.created_at_unix_ms) }}
              </template>
            </el-table-column>
            <el-table-column :label="$t('about.snapshotSize')" width="120">
              <template #default="scope">
                {{ formatSize(scope.row.size_bytes) }}
              </template>
            </el-table-column>
            <el-table-column :label="$t('about.actions')" width="120">
              <template #default="scope">
                <el-button size="small" type="warning" @click="handleRestoreSnapshot(scope.row.name)" :loading="restoringSnapshotName === scope.row.name">
                  {{ $t('about.restoreSnapshot') }}
                </el-button>
              </template>
            </el-table-column>
          </el-table>
        </el-form-item>
      </el-form>
    </div>
  </el-card>
</template>

<script setup lang="ts">
import { onMounted, ref } from 'vue'
import { ElMessage, ElMessageBox } from 'element-plus'
import {
  GetConfig,
  ListConfigSnapshots,
  RestoreConfigSnapshot,
  SendTestAlert,
  type ConfigSnapshotInfo,
  type AlertingConfig,
} from '../api'
import { useI18n } from 'vue-i18n'

const { t } = useI18n()

const sendingTestAlert = ref(false)
const loadingSnapshots = ref(false)
const restoringSnapshotName = ref('')
const snapshotList = ref<ConfigSnapshotInfo[]>([])

const alertForm = ref<AlertingConfig>({
  enabled: false,
  webhook: {
    enabled: false,
    provider: 'wecom',
    url: '',
    secret: '',
  },
  rules: {
    server_start_error: true,
  },
})

const formatTs = (ms: number) => {
  if (!ms) return '-'
  try {
    return new Date(ms).toLocaleString()
  } catch {
    return String(ms)
  }
}

const formatSize = (size: number) => {
  if (size < 1024) return `${size} B`
  if (size < 1024 * 1024) return `${(size / 1024).toFixed(1)} KB`
  return `${(size / (1024 * 1024)).toFixed(1)} MB`
}

const loadSnapshots = async () => {
  loadingSnapshots.value = true
  try {
    snapshotList.value = await ListConfigSnapshots()
  } catch (e: any) {
    ElMessage.error(e?.message || String(e))
  } finally {
    loadingSnapshots.value = false
  }
}

const handleRestoreSnapshot = async (name: string) => {
  await ElMessageBox.confirm(
    t('about.restoreSnapshotConfirm', { name }),
    t('about.restoreSnapshotTitle'),
    {
      type: 'warning',
      confirmButtonText: t('common.confirm'),
      cancelButtonText: t('common.cancel'),
    }
  )

  restoringSnapshotName.value = name
  try {
    await RestoreConfigSnapshot(name)
    ElMessage.success(t('about.restoreSnapshotSuccess'))
    await loadSnapshots()
  } catch (e: any) {
    ElMessage.error(t('about.restoreSnapshotFailed', { error: e?.message || String(e) }))
  } finally {
    restoringSnapshotName.value = ''
  }
}

const handleSendTestAlert = async () => {
  if (!alertForm.value.enabled || !alertForm.value.webhook?.enabled) {
    ElMessage.warning(t('about.alertConfigIncomplete'))
    return
  }
  if (!alertForm.value.webhook.url?.trim()) {
    ElMessage.warning(t('about.alertWebhookUrlRequired'))
    return
  }

  sendingTestAlert.value = true
  try {
    await SendTestAlert(alertForm.value)
    ElMessage.success(t('about.sendTestAlertSuccess'))
  } catch (e: any) {
    ElMessage.error(t('about.sendTestAlertFailed', { error: e?.message || String(e) }))
  } finally {
    sendingTestAlert.value = false
  }
}

const loadInfo = async () => {
  try {
    const cfg: any = await GetConfig()
    const alerting = cfg?.alerting
    if (alerting) {
      alertForm.value.enabled = !!alerting.enabled
      alertForm.value.webhook = {
        enabled: !!alerting?.webhook?.enabled,
        provider: alerting?.webhook?.provider || 'wecom',
        url: alerting?.webhook?.url || '',
        secret: alerting?.webhook?.secret || '',
      }
      alertForm.value.rules = {
        server_start_error: alerting?.rules?.server_start_error !== false,
      }
    }
  } catch {
    // ignore
  }

  await loadSnapshots()
}

const getConfig = () => ({
  alerting: {
    ...alertForm.value,
    webhook: alertForm.value.webhook
      ? {
          ...alertForm.value.webhook,
          url: (alertForm.value.webhook.url || '').trim(),
        }
      : null,
  },
})

defineExpose({ getConfig })

onMounted(() => {
  loadInfo()
})
</script>

<style scoped>
.config-page {
  height: 100%;
  overflow-y: auto;
}

.config-page :deep(.el-card__body) {
  padding: 24px;
}

.config-manage-content {
  display: flex;
  flex-direction: column;
  gap: 16px;
}
</style>
