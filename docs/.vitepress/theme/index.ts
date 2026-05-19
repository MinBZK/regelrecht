import DefaultTheme from 'vitepress/theme'
import type { Theme } from 'vitepress'
// NLDD design-system stylesheet. This is plain CSS (no DOM), so it is
// SSR-safe and MUST be a static import: a dynamic import() of a CSS-only
// module is not reliably injected by Vite, which left the web components
// (nldd-top-navigation-bar etc.) unstyled and collapsed. The Lit
// *components* still load client-only below (they need the DOM).
import '@nldd/design-system/styles'
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

    // The Lit components need the DOM and must not run during SSR — load
    // them client-side only. Styles are imported statically above.
    if (typeof window !== 'undefined') {
      import('@nldd/design-system').catch((e) => {
        console.error('[docs] failed to load @nldd/design-system components', e)
      })
    }
  },
} satisfies Theme
