<template>
  <el-dialog
    v-model="visible"
    title="使用条款与免责声明"
    width="80%"
    :close-on-click-modal="false"
    :close-on-press-escape="false"
    :show-close="false"
    :before-close="handleBeforeClose"
    class="terms-dialog"
  >
    <div class="terms-content">
      <el-scrollbar height="500px">
        <div class="terms-text">
          <h2>SSLProxyManager 使用条款与免责声明</h2>
          
          <h3>1. 软件性质</h3>
          <p>
            SSLProxyManager 是一个通用的反向代理管理工具，提供网络代理、负载均衡、内容分发等合法功能。
            本软件是通用工具，有多种合法用途，不专门设计用于任何非法用途。
          </p>

          <h3>2. 合法使用</h3>
          <p>您必须确保使用本软件的行为符合以下要求：</p>
          <ul>
            <li>遵守当地法律法规及相关网络服务条款</li>
            <li>仅在您拥有或已获得授权的设备/网络上使用</li>
            <li>不用于任何未授权的网络攻击、数据窃取、欺诈等非法用途</li>
            <li>不用于绕过访问控制、侵犯他人隐私或其他违法违规行为</li>
            <li>不用于传播恶意软件、进行网络攻击或破坏网络安全</li>
          </ul>

          <h3>3. 禁止用途</h3>
          <p>严格禁止将本软件用于以下用途：</p>
          <ul>
            <li>未授权的网络攻击（如 DDoS、中间人攻击等）</li>
            <li>数据窃取或隐私侵犯</li>
            <li>欺诈行为（如钓鱼网站、内容篡改用于欺诈等）</li>
            <li>绕过访问控制（未授权）</li>
            <li>传播恶意软件</li>
            <li>侵犯知识产权（如未授权内容修改、盗版分发等）</li>
            <li>任何其他违法违规用途</li>
          </ul>

          <h3>4. 免责声明</h3>
          <p>
            <strong>本软件按"现状"提供，不提供任何形式的明示或暗示担保</strong>（包括但不限于适用性、可靠性、准确性、可用性、无错误/无漏洞等）。
          </p>
          <p>
            <strong>对于因使用或无法使用本软件导致的任何直接或间接损失</strong>（包括但不限于利润损失、数据丢失、业务中断、设备或系统损坏等），
            <strong>作者与贡献者不承担任何责任</strong>。
          </p>
          <p>
            <strong>任何因您使用本软件从事违法违规或未授权行为所产生的法律责任、行政处罚、第三方索赔及相关后果，均由您自行承担，作者与贡献者不承担任何责任</strong>。
          </p>

          <h3>5. 用户责任</h3>
          <p>
            您在使用本软件时需自行评估并承担全部风险与责任。请确保您的使用行为符合当地法律法规及相关网络服务条款。
            如果您不同意上述条款，请勿使用、分发或基于本项目进行二次开发。
          </p>

          <h3>6. 许可证</h3>
          <p>
            本软件采用 MIT 许可证。使用本软件即表示您同意遵守 MIT 许可证的条款。
            详细信息请参阅 LICENSE 文件。
          </p>

          <h3>7. 特别提示</h3>
          <p>
            <strong>⚠️ 重要：</strong>本软件包含响应体修改等功能，这些功能可用于合法用途（如内容定制、广告过滤等），
            但请确保：
          </p>
          <ul>
            <li>仅在你拥有或已获得授权的设备/网络上使用</li>
            <li>不用于欺诈、钓鱼或其他非法用途</li>
            <li>遵守相关法律法规和网站服务条款</li>
            <li>任何非法使用产生的法律责任由使用者自行承担</li>
          </ul>
        </div>
      </el-scrollbar>
    </div>

    <template #footer>
      <div class="dialog-footer">
        <el-checkbox v-if="requireAccept !== false" v-model="agreed" size="large">
          我已阅读并同意上述使用条款与免责声明
        </el-checkbox>
        <div class="footer-buttons">
          <el-button 
            v-if="requireAccept !== false"
            @click="handleReject" 
            type="danger" 
            size="large"
          >
            不同意并退出
          </el-button>
          <el-button 
            @click="handleAccept" 
            type="primary" 
            size="large"
            :disabled="requireAccept !== false && !agreed"
          >
            {{ requireAccept !== false ? '同意并继续' : '关闭' }}
          </el-button>
        </div>
      </div>
    </template>
  </el-dialog>
</template>

<script setup lang="ts">
import { ref } from 'vue'
import { ElMessage, ElMessageBox } from 'element-plus'
import { SetTermsAccepted, QuitApp } from '../api'

const props = defineProps<{
  requireAccept?: boolean  // 是否要求必须接受（首次启动时为true，查看时为false）
}>()

const emit = defineEmits<{
  close: []
}>()

const visible = ref(true)
const agreed = ref(false)

const handleBeforeClose = (done: () => void) => {
  // 如果要求必须接受（首次启动），则不允许通过点击遮罩或 ESC 关闭
  if (props.requireAccept !== false && !agreed.value) {
    ElMessage.warning('请先阅读并同意使用条款')
    return
  }
  // 如果只是查看（不要求接受），允许直接关闭
  done()
}

const handleAccept = async () => {
  // 如果要求必须接受，需要勾选同意
  if (props.requireAccept !== false) {
    if (!agreed.value) {
      ElMessage.warning('请先勾选同意选项')
      return
    }

    try {
      // 保存状态并重启应用（relaunch 会重启应用，后续代码不会执行）
      await SetTermsAccepted(true)
      // 注意：relaunch() 会重启应用，所以下面的代码不会执行
      // 但为了代码完整性，保留这些行
      visible.value = false
      emit('close')
    } catch (error: any) {
      ElMessage.error(`保存条款接受状态失败: ${error?.message || error}`)
    }
  } else {
    // 如果只是查看，直接关闭
    visible.value = false
    emit('close')
  }
}

const handleReject = async () => {
  // 只有在要求必须接受时才显示退出确认
  if (props.requireAccept !== false) {
    try {
      await ElMessageBox.confirm(
        '您必须同意使用条款才能使用本软件。\n\n确定要退出吗？',
        '确认退出',
        {
          confirmButtonText: '退出',
          cancelButtonText: '返回',
          type: 'warning',
        }
      )
      await QuitApp()
    } catch {
      // 用户取消，不做任何操作
    }
  } else {
    // 如果只是查看，直接关闭
    visible.value = false
    emit('close')
  }
}
</script>

<style scoped>
.terms-dialog :deep(.el-dialog__header) {
  border-bottom: 1px solid var(--border);
  padding: 20px 24px;
  background: var(--card-bg);
}

.terms-dialog :deep(.el-dialog__title) {
  font-size: 20px;
  font-weight: 600;
  color: var(--text);
}

.terms-dialog :deep(.el-dialog__body) {
  padding: 0;
}

.terms-content {
  padding: 24px;
  background: var(--card-bg);
}

.terms-text {
  color: var(--text);
  line-height: 1.8;
}

.terms-text h2 {
  font-size: 24px;
  font-weight: 700;
  margin-bottom: 20px;
  color: var(--primary);
}

.terms-text h3 {
  font-size: 18px;
  font-weight: 600;
  margin-top: 24px;
  margin-bottom: 12px;
  color: var(--text);
}

.terms-text p {
  margin-bottom: 16px;
  font-size: 14px;
}

.terms-text ul {
  margin-left: 24px;
  margin-bottom: 16px;
}

.terms-text li {
  margin-bottom: 8px;
  font-size: 14px;
}

.terms-text strong {
  color: var(--primary);
  font-weight: 600;
}

.dialog-footer {
  display: flex;
  flex-direction: column;
  gap: 16px;
  padding: 20px 24px;
  border-top: 1px solid var(--border);
  background: var(--card-bg);
}

.footer-buttons {
  display: flex;
  justify-content: flex-end;
  gap: 12px;
}

:deep(.el-checkbox) {
  font-size: 14px;
}

:deep(.el-checkbox__label) {
  color: var(--text);
  font-weight: 500;
}
</style>
