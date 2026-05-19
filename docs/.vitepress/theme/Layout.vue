<script setup lang="ts">
import { watch } from 'vue'
import { useData } from 'vitepress'
import DefaultTheme from 'vitepress/theme'
import LandingPage from './components/landing/LandingPage.vue'

const { Layout } = DefaultTheme
const { frontmatter, isDark } = useData()

// NLDD colours come from CSS `light-dark()` tokens, which resolve via the
// inherited `color-scheme` property — and NLDD's token *set* is selected
// by `<html data-scheme>`. VitePress drives its own light/dark
// independently, so without pinning both the cards rendered dark on a
// light page (OS prefers-color-scheme leaking into light-dark()). Mirror
// VitePress' isDark onto BOTH data-scheme and color-scheme so NLDD tracks
// the page exactly.
if (typeof document !== 'undefined') {
  watch(
    isDark,
    (dark) => {
      const root = document.documentElement
      root.setAttribute('data-scheme', dark ? 'dark' : 'light')
      root.style.colorScheme = dark ? 'dark' : 'light'
    },
    { immediate: true }
  )
}
</script>

<template>
  <LandingPage v-if="frontmatter.layout === 'landing'" />
  <Layout v-else>
    <!-- Replace the (CSS-hidden) VitePress navbar with our shared nav,
         rendered above the preserved sidebar/content/footer. -->
    <template #layout-top>
      <RrNav />
    </template>
  </Layout>
</template>
