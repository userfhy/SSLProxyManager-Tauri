<template>
  <el-card class="config-card config-page" shadow="hover">
    <template #header>
      <h3>{{ $t('about.title') }}</h3>
    </template>

    <div class="about-content">
      <el-descriptions :column="1" border>
        <el-descriptions-item :label="$t('about.productName')">SSLProxyManager</el-descriptions-item>
        <el-descriptions-item :label="$t('about.currentVersion')">{{ version || '-' }}</el-descriptions-item>
        <el-descriptions-item :label="$t('about.author')">
          <el-link type="primary" @click.prevent="handleOpenURL(authorUrl)">
            {{ authorName }}
          </el-link>
        </el-descriptions-item>
        <el-descriptions-item :label="$t('about.repository')">
          <el-link type="primary" @click.prevent="handleOpenURL(repoUrl)">
            {{ repoUrl }}
          </el-link>
        </el-descriptions-item>
        <el-descriptions-item :label="$t('about.copyright')">© 2026</el-descriptions-item>
        <el-descriptions-item :label="$t('about.terms')">
          <el-link type="primary" @click.prevent="handleShowTerms">
            {{ $t('about.viewTerms') }}
          </el-link>
        </el-descriptions-item>
      </el-descriptions>

      <el-divider />

      <el-form :model="updateForm" label-width="180px">
        <el-form-item :label="$t('about.updateCheck')">
          <el-switch v-model="updateForm.enabled" />
        </el-form-item>


        <el-form-item v-if="updateForm.enabled" :label="$t('about.autoCheckOnStart')">
          <el-switch v-model="updateForm.auto_check" />
        </el-form-item>

        <el-form-item v-if="updateForm.enabled" :label="$t('about.timeout')">
          <el-input-number v-model="updateForm.timeout_ms" :min="1000" :max="60000" />
        </el-form-item>

        <el-form-item v-if="updateForm.enabled" :label="$t('about.ignorePrerelease')">
          <el-switch v-model="updateForm.ignore_prerelease" />
        </el-form-item>

        <el-form-item>
          <el-button type="primary" @click="handleCheckUpdate" :loading="checking">
            {{ $t('about.checkUpdate') }}
          </el-button>
        </el-form-item>
      </el-form>

      <el-alert
        v-if="checkResult"
        :title="resultTitle"
        :type="checkResult.has_update ? 'warning' : 'success'"
        :closable="false"
        show-icon
      >
        <template #default>
          <div v-if="checkResult.update_info">
            <div style="margin-bottom: 6px;"><strong>{{ $t('about.latestVersion') }}</strong>{{ checkResult.update_info.latest_version }}</div>
            <div style="margin-bottom: 6px;" v-if="checkResult.update_info.release_notes">
              <strong>{{ $t('about.releaseNotes') }}</strong>{{ checkResult.update_info.release_notes }}
            </div>
            <div v-if="checkResult.has_update && checkResult.update_info.download_url">
              <strong>{{ $t('about.downloadUrl') }}</strong>
              <el-link type="primary" @click.prevent="handleOpenDownload(checkResult.update_info.download_url)">
                {{ $t('about.openDownloadLink') }}
              </el-link>
              <el-link style="margin-left: 10px;" @click.prevent="handleCopyDownload(checkResult.update_info.download_url)">
                {{ $t('about.copyDownloadLink') }}
              </el-link>
            </div>
          </div>
        </template>
      </el-alert>

      <el-divider />

      <el-form label-width="180px">
        <el-form-item :label="$t('about.terms')">
          <el-button type="warning" @click="handleResetTerms" :loading="resettingTerms">
            {{ $t('about.resetTerms') }}
          </el-button>
          <el-text type="info" size="small" class="mini-hint" style="margin-left: 12px;">
            {{ $t('about.resetTermsHint') }}
          </el-text>
        </el-form-item>
      </el-form>
    </div>

    <!-- 使用条款对话框（查看模式，不要求必须接受） -->
    <TermsDialog v-if="showTermsDialog" :require-accept="false" @close="handleTermsDialogClose" />
  </el-card>
</template>

<script setup lang="ts">
import { computed, onMounted, ref, watch } from 'vue'
import { ElMessage, ElMessageBox } from 'element-plus'
import { GetConfig, GetVersion, CheckUpdate, ResetTermsAccepted } from '../api'
import { openUrl } from '@tauri-apps/plugin-opener'
import TermsDialog from './TermsDialog.vue'
import { useI18n } from 'vue-i18n'

const { t } = useI18n()

const authorName = 'fhy'
const authorUrl = 'https://github.com/userfhy'
const repoUrl = 'https://github.com/userfhy/SSLProxyManager-Tauri'

const version = ref<string>('')
const checking = ref(false)
const checkResult = ref<any>(null)
const resettingTerms = ref(false)
const showTermsDialog = ref(false)

const updateForm = ref({
  enabled: false,
  server_url: '',
  auto_check: false,
  timeout_ms: 10000,
  ignore_prerelease: true,
})

const resultTitle = computed(() => {
  if (!checkResult.value) return ''
  if (checkResult.value.error) return t('about.checkFailed', { error: checkResult.value.error })
  if (checkResult.value.has_update) return t('about.newVersionFound')
  return t('about.currentLatest')
})

const loadInfo = async () => {
  try {
    version.value = String(await GetVersion())
  } catch (e: any) {
    version.value = ''
  }

  try {
    const cfg: any = await GetConfig()
    if (cfg && cfg.update) {
      updateForm.value.enabled = !!cfg.update.enabled
      updateForm.value.server_url = cfg.update.server_url || ''
      updateForm.value.auto_check = !!cfg.update.auto_check
      updateForm.value.timeout_ms = cfg.update.timeout_ms || 10000
      updateForm.value.ignore_prerelease = cfg.update.ignore_prerelease !== false
    }
  } catch (e: any) {
    // ignore
  }
}

const handleOpenURL = async (url: string) => {
  const u = (url || '').trim()
  if (!u) {
    ElMessage.warning(t('about.linkEmpty'))
    return
  }

  try {
    await openUrl(u)
  } catch (e: any) {
    console.error('打开链接失败:', e)
    ElMessage.error(t('about.openLinkFailed'))
  }
}

const handleOpenDownload = async (url: string) => {
  await handleOpenURL(url)
}

const copyToClipboard = async (text: string) => {
  // 1) 优先用标准 Clipboard API
  try {
    if (navigator.clipboard && typeof navigator.clipboard.writeText === 'function') {
      await navigator.clipboard.writeText(text)
      return true
    }
  } catch {
    // fallback
  }

  // 2) fallback：execCommand
  try {
    const ta = document.createElement('textarea')
    ta.value = text
    ta.style.position = 'fixed'
    ta.style.left = '-9999px'
    ta.style.top = '-9999px'
    document.body.appendChild(ta)
    ta.focus()
    ta.select()
    const ok = document.execCommand('copy')
    document.body.removeChild(ta)
    return ok
  } catch {
    return false
  }
}

const handleCopyDownload = async (url: string) => {
  const u = (url || '').trim()
  if (!u) {
    ElMessage.warning(t('about.downloadLinkEmpty'))
    return
  }

  const ok = await copyToClipboard(u)
  if (ok) {
    ElMessage.success(t('about.copySuccess'))
  } else {
    ElMessage.error(t('about.copyFailed'))
  }
}

const handleCheckUpdate = async () => {
  if (!updateForm.value.enabled) {
    ElMessage.warning(t('about.enableUpdateCheckFirst'))
    return
  }

  checking.value = true
  checkResult.value = null
  try {
    const res = await CheckUpdate()
    checkResult.value = res
  } catch (e: any) {
    ElMessage.error(t('about.checkFailed', { error: e?.message || String(e) }))
  } finally {
    checking.value = false
  }
}

const handleShowTerms = () => {
  showTermsDialog.value = true
}

const handleTermsDialogClose = () => {
  showTermsDialog.value = false
}

const handleResetTerms = () => {
  ElMessageBox.confirm(
    t('about.resetTermsConfirm'),
    t('about.resetTermsTitle'),
    {
      confirmButtonText: t('common.confirm'),
      cancelButtonText: t('common.cancel'),
      type: 'warning',
    }
  )
    .then(async () => {
      resettingTerms.value = true
      try {
        // 重置状态并重启应用（relaunch 会重启应用，后续代码不会执行）
        await ResetTermsAccepted()
        // 注意：relaunch() 会重启应用，所以下面的代码不会执行
        // 但为了代码完整性，保留这些行
        ElMessage.success(t('about.resetTermsSuccess'))
      } catch (e: any) {
        ElMessage.error(t('about.resetTermsFailed', { error: e?.message || String(e) }))
        resettingTerms.value = false
      }
    })
    .catch(() => {
      // 用户取消
    })
}

// 暴露给父组件，用于保存配置
const getConfig = () => {
  return {
    update: { ...updateForm.value },
  }
}

defineExpose({ getConfig })

onMounted(() => {
  loadInfo()
})

watch(
  () => updateForm.value,
  () => {
    // 仅本地编辑；真正写回配置由主界面的“保存配置”按钮统一处理
  },
  { deep: true }
)
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
  padding: 24px;
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

.about-content {
  display: flex;
  flex-direction: column;
  gap: 24px;
}

:deep(.el-descriptions__label) {
  width: 120px;
}

.mini-hint {
  display: block;
  margin-top: 6px;
  font-size: 12px;
  line-height: 1.4;
  color: var(--text-muted);
}
</style>
