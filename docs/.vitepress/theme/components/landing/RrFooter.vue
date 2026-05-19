<script setup lang="ts">
import { computed } from 'vue'
import { useRoute } from 'vitepress'
import { content, langFromPath } from './content'

const route = useRoute()
const lang = computed(() => langFromPath(route.path))
const f = computed(() => content[lang.value].footer)
</script>

<template>
  <footer
    class="rr-footer"
    :aria-label="lang === 'en' ? 'Site footer' : 'Sitevoettekst'"
  >
    <div class="rr-footer-grid">
      <div>
        <h3>RegelRecht</h3>
        <p>{{ f.blurb }}</p>
      </div>
      <div>
        <h4>{{ f.linksTitle }}</h4>
        <ul>
          <li v-for="l in f.links" :key="l.href">
            <a :href="l.href">{{ l.label }}</a>
          </li>
        </ul>
      </div>
      <div>
        <h4>{{ f.contactTitle }}</h4>
        <ul>
          <li>
            <a href="mailto:regelrecht@minbzk.nl">regelrecht@minbzk.nl</a>
          </li>
        </ul>
      </div>
      <div>
        <h4>{{ f.partOfTitle }}</h4>
        <ul>
          <li v-for="o in f.partOf" :key="o">{{ o }}</li>
        </ul>
      </div>
    </div>
    <div class="rr-footer-bottom">
      <p>{{ f.copyright }}</p>
    </div>
  </footer>
</template>
