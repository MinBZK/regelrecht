<script setup lang="ts">
import { useData } from 'vitepress'
import DefaultTheme from 'vitepress/theme'
import LandingPage from './components/landing/LandingPage.vue'

const { Layout } = DefaultTheme
const { frontmatter } = useData()
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
