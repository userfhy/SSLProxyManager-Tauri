import { defineConfig } from 'vite'
import vue from '@vitejs/plugin-vue'
import AutoImport from 'unplugin-auto-import/vite'
import Components from 'unplugin-vue-components/vite'
import { ElementPlusResolver } from 'unplugin-vue-components/resolvers'

const isNodeModulePath = (id: string, pkg: string) =>
  id.includes(`/node_modules/${pkg}/`) || id.includes(`\\node_modules\\${pkg}\\`)

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

          if (isNodeModulePath(id, 'vue-echarts')) {
            return 'vendor-vue-echarts'
          }

          if (id.includes('@element-plus/icons-vue')) {
            return 'vendor-element-plus-icons'
          }

          if (isNodeModulePath(id, 'zrender')) {
            return 'vendor-zrender'
          }

          if (isNodeModulePath(id, 'echarts')) {
            if (id.includes('/echarts/charts') || id.includes('/echarts/lib/chart/')) {
              return 'vendor-echarts-charts'
            }
            if (id.includes('/echarts/components') || id.includes('/echarts/lib/component/')) {
              return 'vendor-echarts-components'
            }
            if (id.includes('/echarts/renderers') || id.includes('/echarts/lib/renderer/')) {
              return 'vendor-echarts-renderers'
            }
            return 'vendor-echarts-core'
          }

          if (id.includes('element-plus')) {
            return 'vendor-element-plus'
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
