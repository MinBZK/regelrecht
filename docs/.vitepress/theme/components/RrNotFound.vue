<script setup lang="ts">
import { computed } from 'vue'
import { useRoute } from 'vitepress'
import { langFromPath } from './landing/content'

// 404 is reached from arbitrary paths; the path prefix still tells us
// which language site the user was in.
const route = useRoute()
const lang = computed(() => langFromPath(route.path))
const home = computed(() => (lang.value === 'en' ? '/en/' : '/'))

const t = computed(() =>
  lang.value === 'en'
    ? {
        code: '404',
        title: 'Page not found',
        body: 'The page you were looking for does not exist or has moved.',
        home: 'Back to home',
        docs: 'Documentation',
      }
    : {
        code: '404',
        title: 'Pagina niet gevonden',
        body: 'De pagina die je zocht bestaat niet of is verplaatst.',
        home: 'Terug naar de startpagina',
        docs: 'Documentatie',
      }
)
</script>

<template>
  <section class="rr-notfound" :lang="lang">
    <p class="rr-notfound-code">{{ t.code }}</p>
    <h1>{{ t.title }}</h1>
    <p class="rr-notfound-body">{{ t.body }}</p>
    <p class="rr-notfound-actions">
      <a class="rr-btn rr-btn--primary" :href="home">{{ t.home }}</a>
      <a class="rr-btn rr-btn--ghost" href="/guide/what-is-regelrecht">
        {{ t.docs }}
      </a>
    </p>
  </section>
</template>

<style>
.rr-notfound {
  max-width: 640px;
  margin: 0 auto;
  padding: clamp(3rem, 10vw, 7rem) 1.5rem;
  text-align: center;
}
.rr-notfound-code {
  font-size: clamp(3rem, 12vw, 6rem);
  font-weight: 700;
  line-height: 1;
  margin: 0;
  color: var(--vp-c-brand-1);
}
.rr-notfound h1 {
  margin: 0.5rem 0 1rem;
  font-size: clamp(1.5rem, 5vw, 2.2rem);
  color: var(--vp-c-text-1);
}
.rr-notfound-body {
  color: var(--vp-c-text-2);
  font-size: 1.1rem;
  margin: 0 0 2rem;
}
.rr-notfound-actions {
  display: flex;
  flex-wrap: wrap;
  gap: 1rem;
  justify-content: center;
}
</style>
