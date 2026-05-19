<script setup lang="ts">
import { useData } from 'vitepress'
import DefaultTheme from 'vitepress/theme'
import LandingPage from './components/landing/LandingPage.vue'

const { Layout } = DefaultTheme
const { frontmatter } = useData()
</script>

<template>
  <LandingPage v-if="frontmatter.layout === 'landing'" />
  <Layout v-else />
</template>
