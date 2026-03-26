import { defineAsyncComponent } from 'vue'

let loader: Promise<any> | null = null

export const LazySystemMetricsVChart = defineAsyncComponent({
  suspensible: false,
  loader: async () => {
    if (!loader) {
      loader = (async () => {
        const [echartsCore, renderers, charts, components, vueEcharts] = await Promise.all([
          import('echarts/core'),
          import('echarts/renderers'),
          import('echarts/charts'),
          import('echarts/components'),
          import('vue-echarts'),
        ])

        echartsCore.use([
          renderers.CanvasRenderer,
          charts.LineChart,
          components.GridComponent,
          components.TooltipComponent,
          components.LegendComponent,
          components.DataZoomComponent,
        ])

        return vueEcharts.default
      })()
    }

    return loader
  },
})
