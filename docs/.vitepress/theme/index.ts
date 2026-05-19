import DefaultTheme from 'vitepress/theme'
import type { Theme } from 'vitepress'
import './custom.css'
import RfcIndexTable from './components/RfcIndexTable.vue'

export default {
  extends: DefaultTheme,
  enhanceApp({ app }) {
    app.component('RfcIndexTable', RfcIndexTable)

    // Import design system tokens and components (client-side only)
    // @nldd/design-system is optional — the site works without it
    if (typeof window !== 'undefined') {
      import('@nldd/design-system/styles').catch(() => {
        console.info('[docs] @nldd/design-system not installed — using fallback styling')
      })
      import('@nldd/design-system').catch(() => {})
    }
  },
} satisfies Theme
