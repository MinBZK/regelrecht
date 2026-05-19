<script setup lang="ts">
import { computed } from 'vue'
import { useData } from 'vitepress'

// VitePress puts the page's heading tree on page.value.headers
// ({ level, title, link, children }). Render a flat h2/h3 outline.
const { page, frontmatter } = useData()

interface Header {
  level: number
  title: string
  link: string
  children?: Header[]
}

const flat = computed<Header[]>(() => {
  if (frontmatter.value.aside === false) return []
  const out: Header[] = []
  const walk = (hs: Header[] = []) => {
    for (const h of hs) {
      if (h.level <= 3) out.push(h)
      if (h.children) walk(h.children)
    }
  }
  walk((page.value.headers as Header[]) || [])
  return out
})
</script>

<template>
  <aside
    v-if="flat.length"
    class="rr-doc-outline"
    aria-label="On this page"
  >
    <p class="rr-doc-outline-title">On this page</p>
    <ul>
      <li
        v-for="h in flat"
        :key="h.link"
        :class="{ 'is-sub': h.level === 3 }"
      >
        <a :href="h.link">{{ h.title }}</a>
      </li>
    </ul>
  </aside>
</template>

<style>
.rr-doc-outline {
  font-size: 0.85rem;
  position: sticky;
  top: 1.5rem;
  align-self: start;
  max-height: calc(100vh - 6rem);
  overflow-y: auto;
}
.rr-doc-outline-title {
  font-weight: 700;
  margin: 0 0 0.5rem;
  color: var(--vp-c-text-1);
}
.rr-doc-outline ul {
  list-style: none;
  margin: 0;
  padding: 0;
  border-left: 1px solid var(--vp-c-divider);
}
.rr-doc-outline li {
  margin: 0.25rem 0;
}
.rr-doc-outline li.is-sub {
  padding-left: 0.75rem;
}
.rr-doc-outline a {
  color: var(--vp-c-text-2);
  text-decoration: none;
  display: block;
  padding-left: 0.75rem;
  margin-left: -1px;
  border-left: 2px solid transparent;
}
.rr-doc-outline a:hover {
  color: var(--vp-c-brand-1);
  border-left-color: var(--vp-c-brand-1);
}
</style>
