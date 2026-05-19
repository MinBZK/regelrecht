import DefaultTheme from 'vitepress/theme'
import type { Theme } from 'vitepress'
import './custom.css'
import './landing.css'
import Layout from './Layout.vue'
import RfcIndexTable from './components/RfcIndexTable.vue'
import SignupForm from './components/landing/SignupForm.vue'
import RrNav from './components/RrNav.vue'

export default {
  extends: DefaultTheme,
  Layout,
  enhanceApp({ app }) {
    app.component('RfcIndexTable', RfcIndexTable)
    app.component('SignupForm', SignupForm)
    app.component('RrNav', RrNav)

    // @nldd/design-system is Lit-based: it needs the DOM and must not run
    // during SSR. Import it client-side only. The landing degrades to plain
    // semantic HTML with our token styling if this ever fails to load, so
    // a failure is non-fatal but no longer expected (it is a real dep now).
    if (typeof window !== 'undefined') {
      import('@nldd/design-system/styles').catch((e) => {
        console.error('[docs] failed to load @nldd/design-system styles', e)
      })
      import('@nldd/design-system').catch((e) => {
        console.error('[docs] failed to load @nldd/design-system components', e)
      })
    }
  },
} satisfies Theme
