import { defineAsyncComponent } from 'vue'

type EchartsRegistryDeps = {
  use: (components: unknown[]) => void
  renderers: typeof import('echarts/renderers')
  charts: typeof import('echarts/charts')
  components: typeof import('echarts/components')
}

const loaderCache = new Map<string, Promise<any>>()

export function createLazyVChart(
  cacheKey: string,
  register: (deps: EchartsRegistryDeps) => void,
) {
  return defineAsyncComponent({
    suspensible: false,
    loader: async () => {
      let loader = loaderCache.get(cacheKey)
      if (!loader) {
        loader = (async () => {
          const [echartsCore, renderers, charts, components, vueEcharts] = await Promise.all([
            import('echarts/core'),
            import('echarts/renderers'),
            import('echarts/charts'),
            import('echarts/components'),
            import('vue-echarts'),
          ])

          register({
            use: echartsCore.use,
            renderers,
            charts,
            components,
          })

          return vueEcharts.default
        })()

        loaderCache.set(cacheKey, loader)
      }

      return loader
    },
  })
}
