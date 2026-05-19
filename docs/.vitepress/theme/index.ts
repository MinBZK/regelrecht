import DefaultTheme from 'vitepress/theme'
import type { Theme } from 'vitepress'
// NLDD design-system stylesheet. Plain CSS (no DOM), SSR-safe, static import.
import '@nldd/design-system/styles'
import './custom.css'
import './landing.css'
import Layout from './Layout.vue'
import { VPNavBarSearch } from 'vitepress/theme'
import RfcIndexTable from './components/RfcIndexTable.vue'
import SignupForm from './components/landing/SignupForm.vue'

// Eagerly register the NLDD Lit components on the client at module-eval
// time — exactly like polder's body `<script>import '@nldd/design-system'`.
// The earlier lazy import() inside enhanceApp() registered them too late
// and unordered, so nldd-menu was undefined when the menu-bar overflow
// fired (`showPopover is not a function`). Eager import fixes that race.
// Guarded for SSR: Lit needs the DOM.
if (typeof window !== 'undefined') {
  import('@nldd/design-system')
}

export default {
  extends: DefaultTheme,
  Layout,
  enhanceApp({ app }) {
    app.component('RfcIndexTable', RfcIndexTable)
    app.component('SignupForm', SignupForm)
    app.component('VPNavBarSearch', VPNavBarSearch)
  },
} satisfies Theme
