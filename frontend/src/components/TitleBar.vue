<template>
  <div class="title-bar" :class="{ 'is-maximized': isMaximized }">
    <div class="title-bar-drag-region" @dblclick="handleMaximize" @mousedown="handleStartDragging">
      <div class="title-bar-left">
        <div class="app-icon">
          <el-icon><Setting /></el-icon>
        </div>
        <span class="app-title">SSLProxyManager</span>
      </div>
      <div class="title-bar-right">
        <div class="window-controls">
          <button
            class="window-control-btn minimize-btn"
            @click="handleMinimize"
            title="最小化"
          >
            <svg width="12" height="12" viewBox="0 0 12 12">
              <path d="M 0 6 L 12 6" stroke="currentColor" stroke-width="1.5" fill="none" />
            </svg>
          </button>
          <button
            class="window-control-btn maximize-btn"
            @click="handleMaximize"
            :title="isMaximized ? '还原' : '最大化'"
          >
            <svg v-if="!isMaximized" width="12" height="12" viewBox="0 0 12 12">
              <path d="M 1 1 L 11 1 L 11 11 L 1 11 Z" stroke="currentColor" stroke-width="1.5" fill="none" />
            </svg>
            <svg v-else width="12" height="12" viewBox="0 0 12 12">
              <path d="M 2 4 L 10 4 L 10 10 L 2 10 Z" stroke="currentColor" stroke-width="1.5" fill="none" />
              <path d="M 4 2 L 10 2 L 10 8" stroke="currentColor" stroke-width="1.5" fill="none" />
            </svg>
          </button>
          <button
            class="window-control-btn close-btn"
            @click="handleClose"
            title="关闭"
          >
            <svg width="12" height="12" viewBox="0 0 12 12">
              <path
                d="M 1 1 L 11 11 M 11 1 L 1 11"
                stroke="currentColor"
                stroke-width="1.5"
                fill="none"
                stroke-linecap="round"
              />
            </svg>
          </button>
        </div>
      </div>
    </div>
  </div>
</template>

<script setup lang="ts">
import { ref, onMounted, onBeforeUnmount } from 'vue'
import { Setting } from '@element-plus/icons-vue'
import { HideToTray } from '../api'
import { getCurrentWindow } from '@tauri-apps/api/window'

const isMaximized = ref(false)

const checkMaximized = async () => {
  try {
    const appWindow = getCurrentWindow()
    isMaximized.value = await appWindow.isMaximized()
  } catch (e) {
    console.error('检查窗口状态失败:', e)
  }
}

const getAppWindow = () => {
  return getCurrentWindow()
}

const handleMinimize = async (e?: MouseEvent) => {
  e?.preventDefault()
  e?.stopPropagation()
  try {
    const appWindow = getAppWindow()
    await appWindow.minimize()
  } catch (err) {
    console.error('最小化失败:', err)
  }
}

const handleMaximize = async (e?: MouseEvent) => {
  e?.preventDefault()
  e?.stopPropagation()
  try {
    const appWindow = getAppWindow()
    if (typeof (appWindow as any).toggleMaximize === 'function') {
      await (appWindow as any).toggleMaximize()
    } else {
      if (await appWindow.isMaximized()) {
        await appWindow.unmaximize()
      } else {
        await appWindow.maximize()
      }
    }

    setTimeout(() => {
      checkMaximized()
    }, 100)
  } catch (err) {
    console.error('最大化/还原失败:', err)
  }
}

const handleClose = () => {
  HideToTray()
}

const handleStartDragging = async (e: MouseEvent) => {
  const target = e.target as HTMLElement
  if (target.closest('.window-controls')) {
    return
  }

  try {
    const appWindow = getCurrentWindow()
    await appWindow.startDragging()
  } catch (err) {
    // Linux/X11 下 CSS 的 -webkit-app-region 可能不生效，这里用 startDragging 兜底。
    // 失败时不阻塞其它交互。
    console.error('startDragging 失败:', err)
  }
}

let resizeTimer: number | null = null
const handleResize = () => {
  if (resizeTimer) {
    clearTimeout(resizeTimer)
  }
  resizeTimer = window.setTimeout(() => {
    checkMaximized()
  }, 200)
}

const handleWindowFocus = () => {
  setTimeout(() => {
    checkMaximized()
  }, 100)
}

const handleVisibilityChange = () => {
  if (!document.hidden) {
    setTimeout(() => {
      checkMaximized()
    }, 100)
  }
}

onMounted(() => {
  checkMaximized()
  window.addEventListener('resize', handleResize)
  window.addEventListener('focus', handleWindowFocus)
  document.addEventListener('visibilitychange', handleVisibilityChange)
})

onBeforeUnmount(() => {
  window.removeEventListener('resize', handleResize)
  window.removeEventListener('focus', handleWindowFocus)
  document.removeEventListener('visibilitychange', handleVisibilityChange)

  if (resizeTimer) {
    clearTimeout(resizeTimer)
  }
})
</script>

<style scoped>
.title-bar {
  height: 32px;
  background: var(--card-bg, #1e293b);
  border-bottom: 1px solid var(--border, rgba(255, 255, 255, 0.1));
  display: flex;
  align-items: center;
  user-select: none;
  -webkit-app-region: drag;
  position: relative;
  z-index: 1000;
  flex-shrink: 0;
  cursor: default;
}

.title-bar-drag-region {
  width: 100%;
  height: 100%;
  display: flex;
  align-items: center;
  justify-content: space-between;
  padding: 0 8px;
  -webkit-app-region: drag;
}

.title-bar-left {
  display: flex;
  align-items: center;
  gap: 8px;
  flex: 1;
  min-width: 0;
  -webkit-app-region: drag;
}

.app-icon {
  display: flex;
  align-items: center;
  justify-content: center;
  width: 20px;
  height: 20px;
  color: var(--primary, #3b82f6);
  flex-shrink: 0;
  -webkit-app-region: drag;
  pointer-events: none;
}

.app-title {
  font-size: 13px;
  font-weight: 500;
  color: var(--text, #e2e8f0);
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
  -webkit-app-region: drag;
  pointer-events: none;
}

.title-bar-right {
  display: flex;
  align-items: center;
  flex-shrink: 0;
  -webkit-app-region: no-drag;
}

.window-controls {
  display: flex;
  align-items: center;
  -webkit-app-region: no-drag;
  gap: 2px;
}

.window-control-btn {
  width: 46px;
  height: 32px;
  border: none;
  background: transparent;
  color: var(--text, #e2e8f0);
  cursor: pointer;
  display: flex;
  align-items: center;
  justify-content: center;
  transition: background-color 0.2s;
  padding: 0;
  margin: 0;
  outline: none;
  -webkit-app-region: no-drag;
}

.window-control-btn:hover {
  background: rgba(255, 255, 255, 0.1);
}

.window-control-btn:active {
  background: rgba(255, 255, 255, 0.15);
}

.close-btn:hover {
  background: #e81123;
  color: white;
}

.close-btn:active {
  background: #c50e1f;
}

.window-control-btn svg {
  display: block;
  pointer-events: none;
}

.light-mode .title-bar {
  background: var(--card-bg, #ffffff);
  border-bottom-color: var(--border, rgba(0, 0, 0, 0.1));
}

.light-mode .title-bar .app-title {
  color: var(--text, #1e293b);
}

.light-mode .window-control-btn {
  color: var(--text, #1e293b);
}

.light-mode .window-control-btn:hover {
  background: rgba(0, 0, 0, 0.05);
}

.light-mode .close-btn:hover {
  background: #e81123;
  color: white;
}
</style>
