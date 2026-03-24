import { computed, ref, watch, type Ref } from 'vue'
import type { ComposerTranslation } from 'vue-i18n'

const FONT_SIZE_STORAGE_KEY = 'globalFontSizeSetting'
const SIDEBAR_COLLAPSED_STORAGE_KEY = 'sidebarCollapsed'
const AUTO_THEME_STORAGE_KEY = 'autoThemeEnabled'
const THEME_STORAGE_KEY = 'theme'
const fontSizeOptions = [9, 10, 11, 12, 13, 14, 16, 17, 18, 19, 20, 21, 22, 23, 24] as const

type FontSizeOption = (typeof fontSizeOptions)[number]

export function useAppShellPreferences(t: ComposerTranslation, locale: Ref<string>) {
  const isDark = ref(true)
  const autoThemeEnabled = ref(true)
  const isCollapsed = ref(false)
  const fontSizeSetting = ref<string>('default')
  let autoThemeTimer: number | null = null

  const fontSizeButtonText = computed(() => {
    if (fontSizeSetting.value === 'default') {
      return t('app.fontSizeDefault')
    }
    return t('app.fontSizeButton', { size: fontSizeSetting.value })
  })

  const applyGlobalFontSize = (value: string) => {
    const rootStyle = document.documentElement.style
    const keys = [
      '--spm-font-size-base',
      '--el-font-size-extra-large',
      '--el-font-size-large',
      '--el-font-size-medium',
      '--el-font-size-base',
      '--el-font-size-small',
      '--el-font-size-extra-small',
    ]

    if (value === 'default') {
      keys.forEach((k) => rootStyle.removeProperty(k))
      return
    }

    const size = Number(value)
    if (!Number.isFinite(size)) {
      keys.forEach((k) => rootStyle.removeProperty(k))
      return
    }

    rootStyle.setProperty('--spm-font-size-base', `${size}px`)
    rootStyle.setProperty('--el-font-size-extra-large', `${size + 4}px`)
    rootStyle.setProperty('--el-font-size-large', `${size + 2}px`)
    rootStyle.setProperty('--el-font-size-medium', `${size + 1}px`)
    rootStyle.setProperty('--el-font-size-base', `${size}px`)
    rootStyle.setProperty('--el-font-size-small', `${Math.max(size - 1, 10)}px`)
    rootStyle.setProperty('--el-font-size-extra-small', `${Math.max(size - 2, 9)}px`)
  }

  const loadFontSize = () => {
    const saved = localStorage.getItem(FONT_SIZE_STORAGE_KEY)
    const isAllowed = saved !== null && fontSizeOptions.includes(Number(saved) as FontSizeOption)
    fontSizeSetting.value = isAllowed ? String(saved) : 'default'
    applyGlobalFontSize(fontSizeSetting.value)
  }

  const handleFontSizeCommand = (command: string | number) => {
    const value = String(command)
    const isAllowed = value === 'default' || fontSizeOptions.includes(Number(value) as FontSizeOption)
    if (!isAllowed) return

    fontSizeSetting.value = value
    if (value === 'default') {
      localStorage.removeItem(FONT_SIZE_STORAGE_KEY)
    } else {
      localStorage.setItem(FONT_SIZE_STORAGE_KEY, value)
    }
    applyGlobalFontSize(value)
  }

  const shouldUseDarkMode = (): boolean => {
    const hour = new Date().getHours()
    return hour >= 18 || hour < 6
  }

  const applyTheme = () => {
    document.documentElement.classList.toggle('light-mode', !isDark.value)
  }

  const checkAndAutoSwitchTheme = () => {
    if (!autoThemeEnabled.value) return
    const shouldDark = shouldUseDarkMode()
    if (isDark.value !== shouldDark) {
      isDark.value = shouldDark
      applyTheme()
      console.log(`自动切换主题: ${shouldDark ? '夜间模式' : '日间模式'}`)
    }
  }

  const startAutoTheme = () => {
    if (autoThemeTimer) {
      clearInterval(autoThemeTimer)
    }

    if (!autoThemeEnabled.value) {
      autoThemeTimer = null
      return
    }

    checkAndAutoSwitchTheme()
    autoThemeTimer = window.setInterval(() => {
      checkAndAutoSwitchTheme()
    }, 60000)
  }

  const stopAutoTheme = () => {
    if (autoThemeTimer) {
      clearInterval(autoThemeTimer)
      autoThemeTimer = null
    }
  }

  const toggleSidebar = () => {
    isCollapsed.value = !isCollapsed.value
    localStorage.setItem(SIDEBAR_COLLAPSED_STORAGE_KEY, String(isCollapsed.value))
  }

  const loadSidebarState = () => {
    const saved = localStorage.getItem(SIDEBAR_COLLAPSED_STORAGE_KEY)
    if (saved !== null) {
      isCollapsed.value = saved === 'true'
    }
  }

  const loadTheme = () => {
    const savedAutoTheme = localStorage.getItem(AUTO_THEME_STORAGE_KEY)
    if (savedAutoTheme !== null) {
      autoThemeEnabled.value = savedAutoTheme === 'true'
    }

    if (autoThemeEnabled.value) {
      isDark.value = shouldUseDarkMode()
    } else {
      const savedTheme = localStorage.getItem(THEME_STORAGE_KEY)
      isDark.value = savedTheme ? savedTheme === 'dark' : true
    }
    applyTheme()
  }

  const toggleTheme = () => {
    if (autoThemeEnabled.value) {
      autoThemeEnabled.value = false
      localStorage.setItem(AUTO_THEME_STORAGE_KEY, 'false')
      stopAutoTheme()
    }

    isDark.value = !isDark.value
    localStorage.setItem(THEME_STORAGE_KEY, isDark.value ? 'dark' : 'light')
    applyTheme()
  }

  watch(isDark, applyTheme)

  const formatTimeUnit = (value: number, unitKey: string, pluralKey?: string) => {
    if (locale.value === 'en-US') {
      const unit = pluralKey && value !== 1 ? pluralKey : unitKey
      return `${value} ${t(`app.timeUnit.${unit}`)}`
    }
    return `${value}${t(`app.timeUnit.${unitKey}`)}`
  }

  const getSeparator = () => (locale.value === 'en-US' ? ' ' : '')

  return {
    isDark,
    autoThemeEnabled,
    isCollapsed,
    fontSizeOptions,
    fontSizeSetting,
    fontSizeButtonText,
    loadFontSize,
    handleFontSizeCommand,
    loadTheme,
    toggleTheme,
    startAutoTheme,
    stopAutoTheme,
    toggleSidebar,
    loadSidebarState,
    applyTheme,
    shouldUseDarkMode,
    getSeparator,
    formatTimeUnit,
  }
}
