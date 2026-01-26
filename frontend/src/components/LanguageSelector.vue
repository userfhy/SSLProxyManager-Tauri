<template>
    <div class="translation-container">
        <el-dropdown trigger="click" @command="handleLocaleChange">
            <div class="language-icon" role="button" tabindex="0">
                <svg viewBox="0 0 24 24" fill="currentColor">
                    <path d="m18.5 10l4.4 11h-2.155l-1.201-3h-4.09l-1.199 3h-2.154L16.5 10zM10 2v2h6v2h-1.968a18.2 18.2 0 0 1-3.62 6.301a15 15 0 0 0 2.335 1.707l-.75 1.878A17 17 0 0 1 9 13.725a16.7 16.7 0 0 1-6.201 3.548l-.536-1.929a14.7 14.7 0 0 0 5.327-3.042A18 18 0 0 1 4.767 8h2.24A16 16 0 0 0 9 10.877a16.2 16.2 0 0 0 2.91-4.876L2 6V4h6V2zm7.5 10.885L16.253 16h2.492z"></path>
                </svg>
            </div>
            <template #dropdown>
                <el-dropdown-menu>
                    <el-dropdown-item :command="'zh-CN'" :class="{ 'is-active': currentLocale === 'zh-CN' }">
                        {{ $t('common.chinese') }}
                    </el-dropdown-item>
                    <el-dropdown-item :command="'en-US'" :class="{ 'is-active': currentLocale === 'en-US' }">
                        {{ $t('common.english') }}
                    </el-dropdown-item>
                </el-dropdown-menu>
            </template>
        </el-dropdown>
    </div>
</template>

<script setup lang="ts">
import { computed } from 'vue'
import { useI18n } from 'vue-i18n'
import { SetLocale } from '../api'

// 移除 style prop，因为不再需要 el-select

const { locale } = useI18n()

// 语言切换
const currentLocale = computed({
  get: () => locale.value,
  set: (val) => {
    locale.value = val
    localStorage.setItem('locale', val)
  }
})

// 处理语言切换
const handleLocaleChange = async (val: string) => {
  if (val === currentLocale.value) return
  currentLocale.value = val
  // 同步到后端，更新托盘菜单
  try {
    // @ts-ignore
    await SetLocale(val)
  } catch (error) {
    console.error('设置语言失败:', error)
  }
}
</script>

<style scoped>
.translation-container {
  display: flex;
  align-items: center;
  cursor: pointer;
}

.language-icon {
  display: flex;
  align-items: center;
  justify-content: center;
  width: 32px;
  height: 32px;
  cursor: pointer;
  border-radius: 4px;
  transition: background-color 0.2s;
}

.language-icon:hover {
  background-color: var(--el-fill-color-light);
}

.language-icon svg {
  width: 24px;
  height: 24px;
  color: var(--text);
}

:deep(.el-dropdown-menu__item) {
  color: var(--el-text-color-primary) !important;
  font-weight: normal;
}

:deep(.el-dropdown-menu__item:hover) {
  background-color: rgba(0, 0, 0, 0.04) !important;
  color: var(--el-text-color-primary) !important;
  font-weight: 600 !important;
}

/* 暗色模式（非 light-mode） */
:deep(html:not(.light-mode) .el-dropdown-menu__item:hover) {
  font-weight: 600 !important;
}

:deep(.el-dropdown-menu__item.is-active) {
  color: var(--el-color-primary) !important;
  font-weight: 600 !important;
  background-color: transparent !important;
}

:deep(.el-dropdown-menu__item.is-active:hover) {
  background-color: rgba(0, 0, 0, 0.04) !important;
  color: var(--el-color-primary) !important;
  font-weight: 600 !important;
}

:deep(html:not(.light-mode) .el-dropdown-menu__item.is-active:hover) {
  color: var(--el-color-primary) !important;
  font-weight: 600 !important;
}
</style>

<style>
/* 全局样式：暗色模式下的下拉菜单项（下拉菜单挂载在 body 上，需要使用全局样式） */
html:not(.light-mode) .el-dropdown-menu__item:hover {
  background-color: rgba(255, 255, 255, 0.08) !important;
  color: #003cff !important;
  font-weight: 600 !important;
}

html:not(.light-mode) .el-dropdown-menu__item.is-active {
  color: #60a5fa !important;
  font-weight: 600 !important;
}

html:not(.light-mode) .el-dropdown-menu__item.is-active:hover {
  background-color: rgba(255, 255, 255, 0.08) !important;
  color: #93c5fd !important;
  font-weight: 600 !important;
}
</style>
