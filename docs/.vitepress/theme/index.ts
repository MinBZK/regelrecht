import DefaultTheme from 'vitepress/theme'
import type { Theme } from 'vitepress'
import './custom.css'

export default {
  extends: DefaultTheme,
  enhanceApp({ app }) {
    // Import design system tokens and components (client-side only)
    // @minbzk/storybook is optional — the site works without it
    if (typeof window !== 'undefined') {
      import('@minbzk/storybook/css').catch(() => {
        console.info('[docs] @minbzk/storybook not installed — using fallback styling')
      })
      import('@minbzk/storybook').catch(() => {})
    }
  },
} satisfies Theme
