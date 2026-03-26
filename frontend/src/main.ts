import { createApp } from 'vue'
import 'element-plus/dist/index.css'
import 'element-plus/theme-chalk/dark/css-vars.css'
import './style.css'
import { listen } from '@tauri-apps/api/event'
import { getCurrentWindow } from '@tauri-apps/api/window'
import i18n from './i18n'

const params = new URLSearchParams(window.location.search)
const isChartPreviewMode = params.get('chart_preview') === '1'

const bootstrap = async () => {
  const rootComponent = isChartPreviewMode
    ? (await import('./components/ChartPreviewPage.vue')).default
    : (await import('./App.vue')).default

  const app = createApp(rootComponent)
  app.use(i18n)

  if (!isChartPreviewMode) {
    // 托盘点击唤醒窗口（Linux/Wayland 下由后端直接 set_focus 可能无效，改为前端主动调用窗口 API）
    listen('tray-show-main', async () => {
      try {
        const w = getCurrentWindow()
        await w.show()
        await w.unminimize()
        await w.setFocus()
      } catch {
        // ignore
      }
    })
  }

  // 正式版禁用右键菜单（避免弹出浏览器默认菜单）
  if (!import.meta.env.DEV) {
    window.addEventListener(
      'contextmenu',
      (e) => {
        e.preventDefault()
      },
      { capture: true }
    )
  }

  app.mount('#app')

  const bootLoading = document.getElementById('boot-loading')
  if (bootLoading) {
    requestAnimationFrame(() => {
      bootLoading.classList.add('boot-loading--hide')
    })
    window.setTimeout(() => {
      bootLoading.remove()
    }, 260)
  }
}

void bootstrap()
