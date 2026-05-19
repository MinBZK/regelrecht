<script setup lang="ts">
import { computed } from 'vue'
import { useData, useRoute } from 'vitepress'
import { useSidebar } from 'vitepress/theme'

const route = useRoute()
const { page, theme } = useData()
const { sidebarGroups } = useSidebar()

interface FlatLink {
  text: string
  link: string
}

function norm(p: string) {
  return p.replace(/(index)?\.(md|html)$/, '').replace(/\/$/, '')
}

// Flatten the resolved sidebar into an ordered link list; prev/next are
// the neighbours of the current route. Same data VitePress uses, without
// the non-exported usePrevNext composable.
const flat = computed<FlatLink[]>(() => {
  const out: FlatLink[] = []
  const walk = (items: any[] = []) => {
    for (const it of items) {
      if (it.link) out.push({ text: it.text, link: it.link })
      if (it.items) walk(it.items)
    }
  }
  for (const g of sidebarGroups.value as any[]) walk(g.items)
  return out
})

const idx = computed(() =>
  flat.value.findIndex((l) => norm(l.link) === norm(route.path))
)
const prev = computed(() =>
  idx.value > 0 ? flat.value[idx.value - 1] : null
)
const next = computed(() =>
  idx.value >= 0 && idx.value < flat.value.length - 1
    ? flat.value[idx.value + 1]
    : null
)

const editUrl = computed(() => {
  const pattern = theme.value.editLink?.pattern
  if (!pattern) return null
  return typeof pattern === 'function'
    ? pattern(page.value)
    : pattern.replace(/:path/g, page.value.relativePath)
})
</script>

<template>
  <footer class="rr-doc-footer">
    <p v-if="editUrl" class="rr-doc-edit">
      <a :href="editUrl" target="_blank" rel="noreferrer">
        Edit this page on GitHub
      </a>
    </p>
    <nav class="rr-doc-prevnext" aria-label="Pagination">
      <a v-if="prev" class="rr-doc-prev" :href="prev.link">
        <span class="rr-doc-pn-label">Previous</span>
        <span class="rr-doc-pn-title">{{ prev.text }}</span>
      </a>
      <span v-else />
      <a v-if="next" class="rr-doc-next" :href="next.link">
        <span class="rr-doc-pn-label">Next</span>
        <span class="rr-doc-pn-title">{{ next.text }}</span>
      </a>
    </nav>
  </footer>
</template>

<style>
.rr-doc-footer {
  margin-top: 3rem;
  padding-top: 1.5rem;
  border-top: 1px solid var(--vp-c-divider);
}
.rr-doc-edit {
  margin: 0 0 1.5rem;
  font-size: 0.9rem;
}
.rr-doc-edit a {
  color: var(--vp-c-brand-1);
  text-decoration: none;
}
.dark .rr-doc-edit a {
  color: var(--vp-c-brand-2);
}
.rr-doc-edit a:hover {
  text-decoration: underline;
}
.rr-doc-prevnext {
  display: flex;
  justify-content: space-between;
  gap: 1rem;
  flex-wrap: wrap;
}
.rr-doc-prevnext a {
  flex: 1 1 240px;
  border: 1px solid var(--vp-c-divider);
  border-radius: 8px;
  padding: 0.85rem 1rem;
  text-decoration: none;
  display: flex;
  flex-direction: column;
  gap: 0.2rem;
}
.rr-doc-next {
  text-align: right;
  align-items: flex-end;
}
.rr-doc-prevnext a:hover {
  border-color: var(--vp-c-brand-1);
}
.rr-doc-pn-label {
  font-size: 0.78rem;
  color: var(--vp-c-text-2);
}
.rr-doc-pn-title {
  color: var(--vp-c-brand-1);
  font-weight: 600;
}
.dark .rr-doc-pn-title {
  color: var(--vp-c-brand-2);
}
</style>
