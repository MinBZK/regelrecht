<script setup lang="ts">
import { computed } from 'vue'
import { useData, useRoute } from 'vitepress'
import { content, langFromPath } from './landing/content'
import { docsNav } from '../../navLinks'

const route = useRoute()
const { frontmatter, isDark } = useData()

const isLanding = computed(() => frontmatter.value.layout === 'landing')
const showSignup = computed(() => frontmatter.value.page === 'aanmelden')
// The docs are English-only; only the landing is bilingual (path-driven).
const lang = computed<'nl' | 'en'>(() =>
  isLanding.value ? langFromPath(route.path) : 'en'
)
const t = computed(() => content[lang.value])
const home = computed(() => (lang.value === 'en' ? '/en/' : '/'))

const navLabel = computed(() =>
  lang.value === 'en' ? 'Main navigation' : 'Hoofdnavigatie'
)
const brandSubtitle = computed(() =>
  isLanding.value ? t.value.nav.brandTagline : 'Machine-readable Dutch law'
)
const searchLabel = computed(() => (lang.value === 'en' ? 'Search' : 'Zoeken'))
const themeLabel = computed(() =>
  isDark.value
    ? lang.value === 'en'
      ? 'Light mode'
      : 'Lichte modus'
    : lang.value === 'en'
      ? 'Dark mode'
      : 'Donkere modus'
)

interface Item {
  text: string
  href: string
  current?: boolean
}

const links = computed<Item[]>(() => {
  if (isLanding.value) {
    const primary = showSignup.value
      ? []
      : [
          { text: t.value.nav.what, href: home.value + '#what-is-it' },
          { text: t.value.nav.how, href: home.value + '#how-it-works' },
          { text: t.value.nav.tools, href: home.value + '#tools' },
          { text: t.value.nav.example, href: home.value + '#example' },
          { text: t.value.nav.faq, href: home.value + '#faq' },
        ]
    return [
      ...primary,
      { text: t.value.nav.signup, href: t.value.feedback.ctaHref },
      { text: t.value.nav.docs, href: '/guide/what-is-regelrecht' },
      {
        text: lang.value === 'en' ? 'Nederlands' : 'English',
        href: lang.value === 'en' ? '/' : '/en/',
      },
    ]
  }
  return docsNav.map((n) => ({
    text: n.text,
    href: n.link,
    current: n.match ? route.path.startsWith(n.match) : false,
  }))
})

function openSearch() {
  if (typeof window === 'undefined') return
  // Re-trigger VitePress' still-mounted local search via its own hotkey.
  window.dispatchEvent(
    new KeyboardEvent('keydown', {
      key: 'k',
      metaKey: true,
      ctrlKey: true,
      bubbles: true,
    })
  )
}

function toggleTheme() {
  isDark.value = !isDark.value
}
</script>

<template>
  <header class="rr-topnav">
    <div class="rr-topnav-inner">
      <a class="rr-brand" :href="home">
        <b>RegelRecht</b>
        <small>{{ brandSubtitle }}</small>
      </a>

      <!-- Real NLDD web component for the menu; degrades to the semantic
           fallback below until it upgrades / without JS. -->
      <nldd-menu-bar
        class="rr-menubar"
        :accessible-label="navLabel"
      >
        <nldd-menu-bar-item
          v-for="item in links"
          :key="item.href"
          :text="item.text"
          :href="item.href"
          :current="item.current || undefined"
        />
        <nldd-menu-bar-item
          :text="searchLabel"
          icon="search"
          @click="openSearch"
        />
        <nldd-menu-bar-item
          :text="themeLabel"
          :icon="isDark ? 'sun' : 'moon'"
          @click="toggleTheme"
        />
        <nldd-menu-bar-item
          text="GitHub"
          href="https://github.com/MinBZK/regelrecht"
        />
      </nldd-menu-bar>

      <!-- Semantic fallback: SSR-rendered, real links, hidden once the
           web component upgrades (:defined). Works without JS. -->
      <nav class="rr-nav-fallback" :aria-label="navLabel">
        <ul class="rr-nav-links">
          <li v-for="item in links" :key="item.href">
            <a
              :href="item.href"
              :aria-current="item.current ? 'page' : undefined"
              >{{ item.text }}</a
            >
          </li>
          <li>
            <a href="https://github.com/MinBZK/regelrecht">GitHub</a>
          </li>
          <li>
            <button type="button" class="rr-nav-iconbtn" @click="toggleTheme">
              {{ themeLabel }}
            </button>
          </li>
        </ul>
      </nav>
    </div>
  </header>
</template>
