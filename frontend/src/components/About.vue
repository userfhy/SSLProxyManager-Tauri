<template>
  <el-card class="config-card config-page" shadow="hover">
    <template #header>
      <h3>关于</h3>
    </template>

    <div class="about-content">
      <el-descriptions :column="1" border>
        <el-descriptions-item label="产品名称">SSLProxyManager</el-descriptions-item>
        <el-descriptions-item label="当前版本">{{ version || '-' }}</el-descriptions-item>
        <el-descriptions-item label="作者">
          <el-link type="primary" @click.prevent="handleOpenURL(authorUrl)">
            {{ authorName }}
          </el-link>
        </el-descriptions-item>
        <el-descriptions-item label="仓库地址">
          <el-link type="primary" @click.prevent="handleOpenURL(repoUrl)">
            {{ repoUrl }}
          </el-link>
        </el-descriptions-item>
        <el-descriptions-item label="版权">© 2026</el-descriptions-item>
      </el-descriptions>

      <el-divider />

      <el-form :model="updateForm" label-width="180px">
        <el-form-item label="启用更新检查">
          <el-switch v-model="updateForm.enabled" />
        </el-form-item>


        <el-form-item v-if="updateForm.enabled" label="启动后自动检查">
          <el-switch v-model="updateForm.auto_check" />
        </el-form-item>

        <el-form-item v-if="updateForm.enabled" label="超时(毫秒)">
          <el-input-number v-model="updateForm.timeout_ms" :min="1000" :max="60000" />
        </el-form-item>

        <el-form-item v-if="updateForm.enabled" label="忽略预发布版本">
          <el-switch v-model="updateForm.ignore_prerelease" />
        </el-form-item>

        <el-form-item>
          <el-button type="primary" @click="handleCheckUpdate" :loading="checking">
            检查新版本
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
            <div style="margin-bottom: 6px;"><strong>最新版本：</strong>{{ checkResult.update_info.latest_version }}</div>
            <div style="margin-bottom: 6px;" v-if="checkResult.update_info.release_notes">
              <strong>更新说明：</strong>{{ checkResult.update_info.release_notes }}
            </div>
            <div v-if="checkResult.has_update && checkResult.update_info.download_url">
              <strong>下载地址：</strong>
              <el-link type="primary" @click.prevent="handleOpenDownload(checkResult.update_info.download_url)">
                打开下载链接
              </el-link>
              <el-link style="margin-left: 10px;" @click.prevent="handleCopyDownload(checkResult.update_info.download_url)">
                复制下载链接
              </el-link>
            </div>
          </div>
        </template>
      </el-alert>
    </div>
  </el-card>
</template>

<script setup lang="ts">
import { computed, onMounted, ref, watch } from 'vue'
import { ElMessage } from 'element-plus'
import { GetConfig, GetVersion, CheckUpdate, OpenURL } from '../api'

const authorName = 'fhy'
const authorUrl = 'https://github.com/userfhy'
const repoUrl = 'https://github.com/userfhy/SSLProxyManager-Tauri'

const version = ref<string>('')
const checking = ref(false)
const checkResult = ref<any>(null)

const updateForm = ref({
  enabled: false,
  server_url: '',
  auto_check: false,
  timeout_ms: 10000,
  ignore_prerelease: true,
})

const resultTitle = computed(() => {
  if (!checkResult.value) return ''
  if (checkResult.value.error) return `检查失败：${checkResult.value.error}`
  if (checkResult.value.has_update) return '发现新版本'
  return '当前已是最新版本'
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

const opening = ref(false)

const handleOpenURL = async (url: string) => {
  const u = (url || '').trim()
  if (!u) {
    ElMessage.warning('链接为空')
    return
  }

  if (opening.value) {
    return
  }

  opening.value = true
  try {
    await OpenURL(u)
  } catch (e: any) {
    console.error('打开链接失败:', e)
    ElMessage.error('打开链接失败，请手动复制到浏览器打开')
  } finally {
    // 防止短时间内连续点击触发系统 shell 多次启动
    setTimeout(() => {
      opening.value = false
    }, 800)
  }
}

const handleOpenDownload = (url: string) => {
  handleOpenURL(url)
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
    ElMessage.warning('下载链接为空')
    return
  }

  const ok = await copyToClipboard(u)
  if (ok) {
    ElMessage.success('已复制下载链接')
  } else {
    ElMessage.error('复制失败，请手动复制')
  }
}

const handleCheckUpdate = async () => {
  if (!updateForm.value.enabled) {
    ElMessage.warning('请先启用更新检查')
    return
  }

  checking.value = true
  checkResult.value = null
  try {
    const res = await CheckUpdate()
    checkResult.value = res
  } catch (e: any) {
    ElMessage.error(`检查失败: ${e?.message || String(e)}`)
  } finally {
    checking.value = false
  }
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
.about-content {
  display: flex;
  flex-direction: column;
  gap: 16px;
}
</style>
