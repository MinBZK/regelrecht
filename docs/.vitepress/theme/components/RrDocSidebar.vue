<script setup lang="ts">
import { useRoute } from 'vitepress'

// `groups` comes from VitePress' own useSidebar().sidebarGroups, so the
// structure stays driven by config.ts (single source of truth). Each group
// is { text, items: [{ text, link }] }, items may nest further.
defineProps<{ groups: SidebarGroup[] }>()

interface SidebarItem {
  text?: string
  link?: string
  items?: SidebarItem[]
}
interface SidebarGroup {
  text?: string
  items: SidebarItem[]
}

const route = useRoute()

function normalize(path: string) {
  return path.replace(/(index)?\.(md|html)$/, '').replace(/\/$/, '')
}
function isActive(link?: string) {
  if (!link) return false
  return normalize(route.path) === normalize(link)
}
</script>

<template>
  <nav class="rr-doc-sidebar" aria-label="Documentation navigation">
    <div v-for="group in groups" :key="group.text" class="rr-doc-sidebar-group">
      <p v-if="group.text" class="rr-doc-sidebar-title">{{ group.text }}</p>
      <ul>
        <li v-for="item in group.items" :key="item.text">
          <a
            v-if="item.link"
            :href="item.link"
            :aria-current="isActive(item.link) ? 'page' : undefined"
            :class="{ 'is-active': isActive(item.link) }"
            >{{ item.text }}</a
          >
          <span v-else>{{ item.text }}</span>
          <ul v-if="item.items && item.items.length">
            <li v-for="sub in item.items" :key="sub.text">
              <a
                v-if="sub.link"
                :href="sub.link"
                :aria-current="isActive(sub.link) ? 'page' : undefined"
                :class="{ 'is-active': isActive(sub.link) }"
                >{{ sub.text }}</a
              >
            </li>
          </ul>
        </li>
      </ul>
    </div>
  </nav>
</template>

<style>
.rr-doc-sidebar {
  font-size: 0.9rem;
  position: sticky;
  top: 1.5rem;
  align-self: start;
  max-height: calc(100vh - 6rem);
  overflow-y: auto;
}
.rr-doc-sidebar-group {
  margin-bottom: 1.5rem;
}
.rr-doc-sidebar-title {
  font-weight: 700;
  margin: 0 0 0.5rem;
  color: var(--vp-c-text-1);
}
.rr-doc-sidebar ul {
  list-style: none;
  margin: 0;
  padding: 0;
}
.rr-doc-sidebar ul ul {
  margin-left: 0.75rem;
  border-left: 1px solid var(--vp-c-divider);
  padding-left: 0.75rem;
}
.rr-doc-sidebar li {
  margin: 0.3rem 0;
}
.rr-doc-sidebar a {
  color: var(--vp-c-text-2);
  text-decoration: none;
  display: block;
  padding: 0.15rem 0;
}
.rr-doc-sidebar a:hover {
  color: var(--vp-c-brand-1);
}
.rr-doc-sidebar a.is-active {
  color: var(--vp-c-brand-1);
  font-weight: 600;
}
.dark .rr-doc-sidebar a.is-active {
  color: var(--vp-c-brand-2);
}
</style>
