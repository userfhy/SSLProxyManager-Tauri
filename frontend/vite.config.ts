import { defineConfig } from 'vite'
import vue from '@vitejs/plugin-vue'
import AutoImport from 'unplugin-auto-import/vite'
import Components from 'unplugin-vue-components/vite'
import { ElementPlusResolver } from 'unplugin-vue-components/resolvers'

export default defineConfig({
  plugins: [
    vue(),
    AutoImport({
      resolvers: [ElementPlusResolver()],
    }),
    Components({
      resolvers: [ElementPlusResolver()],
    }),
  ],
  server: {
    port: 5173,
    strictPort: true,
  },
  build: {
    rollupOptions: {
      output: {
        manualChunks(id) {
          if (!id.includes('node_modules')) return

          if (id.includes('echarts') || id.includes('zrender')) {
            return 'vendor-echarts'
          }

          if (id.includes('element-plus')) {
            return 'vendor-element-plus'
          }

          if (id.includes('@element-plus/icons-vue')) {
            return 'vendor-element-plus-icons'
          }

          if (id.includes('vue-i18n')) {
            return 'vendor-i18n'
          }

          if (id.includes('@tauri-apps')) {
            return 'vendor-tauri'
          }

          if (id.includes('vue') || id.includes('pinia')) {
            return 'vendor-vue'
          }

          return 'vendor-misc'
        },
      },
    },
  },
})
