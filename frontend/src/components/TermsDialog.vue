<template>
  <el-dialog
    v-model="visible"
    :title="$t('terms.title')"
    width="80%"
    :close-on-click-modal="false"
    :close-on-press-escape="false"
    :show-close="false"
    :before-close="handleBeforeClose"
    class="terms-dialog"
  >
    <div class="terms-content">
      <div class="language-selector">
        <LanguageSelector />
      </div>
      <el-scrollbar height="500px">
        <div class="terms-text">
          <pre>{{ $t("terms.content") }}</pre>
        </div>
      </el-scrollbar>
    </div>

    <template #footer>
      <div class="dialog-footer">
        <el-checkbox v-if="requireAccept !== false" v-model="agreed" size="large">
          {{ $t("terms.readAndAgree") }}
        </el-checkbox>
        <div class="footer-buttons">
          <el-button
            v-if="requireAccept !== false"
            @click="handleReject"
            type="danger"
            size="large"
          >
            {{ $t("terms.disagreeAndQuit") }}
          </el-button>
          <el-button
            @click="handleAccept"
            type="primary"
            size="large"
            :disabled="requireAccept !== false && !agreed"
          >
            {{ requireAccept !== false ? $t("terms.agreeAndContinue") : $t("terms.close") }}
          </el-button>
        </div>
      </div>
    </template>
  </el-dialog>
</template>

<script setup lang="ts">
import { ref } from "vue";
import { ElMessage, ElMessageBox } from "element-plus";
import { SetTermsAccepted, QuitApp } from "../api";
import { useI18n } from "vue-i18n";
import LanguageSelector from "./LanguageSelector.vue";

const { t } = useI18n();

const props = defineProps<{
  requireAccept?: boolean; // 是否要求必须接受（首次启动时为true，查看时为false）
}>();

const emit = defineEmits<{
  close: [];
}>();

const visible = ref(true);
const agreed = ref(false);

const handleBeforeClose = (done: () => void) => {
  // 如果要求必须接受（首次启动），则不允许通过点击遮罩或 ESC 关闭
  if (props.requireAccept !== false && !agreed.value) {
    ElMessage.warning(t("terms.pleaseReadAndAgree"));
    return;
  }
  // 如果只是查看（不要求接受），允许直接关闭
  done();
};

const handleAccept = async () => {
  // 如果要求必须接受，需要勾选同意
  if (props.requireAccept !== false) {
    if (!agreed.value) {
      ElMessage.warning(t("terms.pleaseCheckAgree"));
      return;
    }

    try {
      // 保存状态并重启应用（relaunch 会重启应用，后续代码不会执行）
      await SetTermsAccepted(true);
      // 注意：relaunch() 会重启应用，所以下面的代码不会执行
      // 但为了代码完整性，保留这些行
      visible.value = false;
      emit("close");
    } catch (error: any) {
      ElMessage.error(t("terms.saveFailed", { error: error?.message || error }));
    }
  } else {
    // 如果只是查看，直接关闭
    visible.value = false;
    emit("close");
  }
};

const handleReject = async () => {
  // 只有在要求必须接受时才显示退出确认
  if (props.requireAccept !== false) {
    try {
      await ElMessageBox.confirm(t("terms.mustAgreeToUse"), t("terms.confirmQuit"), {
        confirmButtonText: t("terms.quit"),
        cancelButtonText: t("terms.return"),
        type: "warning",
      });
      await QuitApp();
    } catch {
      // 用户取消，不做任何操作
    }
  } else {
    // 如果只是查看，直接关闭
    visible.value = false;
    emit("close");
  }
};
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

.language-selector {
  display: flex;
  justify-content: flex-end;
  margin-bottom: 16px;
}

.terms-text {
  color: var(--text);
  line-height: 1.8;
  font-size: 14px;
  white-space: pre-wrap;
  word-wrap: break-word;
}

.terms-text pre {
  margin: 0;
  padding: 0;
  font-family: inherit;
  font-size: inherit;
  color: inherit;
  white-space: pre-wrap;
  word-wrap: break-word;
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
