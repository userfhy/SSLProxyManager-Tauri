import { defineAsyncComponent } from 'vue'

let loader: Promise<any> | null = null

export const LazyDashboardVChart = defineAsyncComponent({
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
          charts.BarChart,
          charts.PieChart,
          components.GridComponent,
          components.TooltipComponent,
          components.LegendComponent,
          components.GraphicComponent,
          components.DataZoomComponent,
        ])

        return vueEcharts.default
      })()
    }

    return loader
  },
})
