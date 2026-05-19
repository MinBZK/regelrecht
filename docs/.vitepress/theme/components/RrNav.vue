<script setup lang="ts">
import { computed } from 'vue'
import { useData, useRoute } from 'vitepress'
import { content, langFromPath } from './landing/content'
import { docsNav } from '../../navLinks'

const route = useRoute()
const { frontmatter, isDark } = useData()

const isLanding = computed(() => frontmatter.value.layout === 'landing')
const showSignup = computed(() => frontmatter.value.page === 'aanmelden')
const lang = computed(() => langFromPath(route.path))
const t = computed(() => content[lang.value])
const home = computed(() => (lang.value === 'en' ? '/en/' : '/'))

const navLabel = computed(() =>
  lang.value === 'en' ? 'Main navigation' : 'Hoofdnavigatie'
)
const utilLabel = computed(() =>
  lang.value === 'en' ? 'Utilities' : 'Hulpmiddelen'
)

interface Item {
  text: string
  href: string
  current?: boolean
  external?: boolean
}

// Primary links differ per context: marketing sections on the landing,
// documentation sections in the docs.
const primary = computed<Item[]>(() => {
  if (isLanding.value) {
    if (showSignup.value) return []
    return [
      { text: t.value.nav.what, href: home.value + '#what-is-it' },
      { text: t.value.nav.how, href: home.value + '#how-it-works' },
      { text: t.value.nav.tools, href: home.value + '#tools' },
      { text: t.value.nav.example, href: home.value + '#example' },
      { text: t.value.nav.faq, href: home.value + '#faq' },
    ]
  }
  return docsNav.map((n) => ({
    text: n.text,
    href: n.link,
    current: n.match ? route.path.startsWith(n.match) : false,
  }))
})

// Utility links: sign-up + docs/landing cross-link + language + GitHub.
const utility = computed<Item[]>(() => {
  if (isLanding.value) {
    return [
      { text: t.value.nav.signup, href: t.value.feedback.ctaHref },
      { text: t.value.nav.docs, href: '/guide/what-is-regelrecht' },
      {
        text: lang.value === 'en' ? 'Nederlands' : 'English',
        href: lang.value === 'en' ? '/' : '/en/',
      },
      { text: 'GitHub', href: 'https://github.com/MinBZK/regelrecht', external: true },
    ]
  }
  return [
    { text: 'GitHub', href: 'https://github.com/MinBZK/regelrecht', external: true },
  ]
})

const brandTitle = 'RegelRecht'
const brandSubtitle = computed(() =>
  isLanding.value ? t.value.nav.brandTagline : 'Machine-readable Dutch law'
)

const searchLabel = computed(() => (lang.value === 'en' ? 'Search' : 'Zoeken'))
const themeLabel = computed(() =>
  isDark.value
    ? lang.value === 'en'
      ? 'Switch to light mode'
      : 'Schakel naar lichte modus'
    : lang.value === 'en'
      ? 'Switch to dark mode'
      : 'Schakel naar donkere modus'
)

// Re-provide the search trigger that lived in the now-hidden VitePress
// navbar: VitePress' own search button dispatches a synthetic Cmd/Ctrl+K,
// and its global key listener stays mounted, so this opens the local
// search box without re-implementing search.
function openSearch() {
  if (typeof window === 'undefined') return
  const e = new KeyboardEvent('keydown', {
    key: 'k',
    metaKey: true,
    ctrlKey: true,
    bubbles: true,
  })
  window.dispatchEvent(e)
}

function toggleTheme() {
  isDark.value = !isDark.value
}
</script>

<template>
  <div class="rr-topnav">
    <!-- Rich web-component nav (client-only; upgrades when @nldd loads) -->
    <nldd-top-navigation-bar
      class="rr-topnav-wc"
      :website-title="brandTitle"
      :logo-title="brandTitle"
      :logo-subtitle="brandSubtitle"
      :logo-href="home"
      :website-href="home"
      no-logo
    >
      <nldd-menu-bar slot="global" :accessible-label="navLabel">
        <nldd-menu-bar-item
          v-for="item in primary"
          :key="item.href"
          :text="item.text"
          :href="item.href"
          :current="item.current || undefined"
        />
      </nldd-menu-bar>

      <nldd-menu-bar slot="utility" :accessible-label="utilLabel">
        <nldd-menu-bar-item
          :text="searchLabel"
          icon="search"
          @click="openSearch"
        />
        <nldd-menu-bar-item
          :text="themeLabel"
          :icon="isDark ? 'sun' : 'moon'"
          icon-only
          @click="toggleTheme"
        />
        <nldd-menu-bar-item
          v-for="item in utility"
          :key="item.href"
          :text="item.text"
          :href="item.href"
        />
      </nldd-menu-bar>
    </nldd-top-navigation-bar>

    <!-- Semantic fallback: this is what SSR emits and what shows without JS
         or before the web component upgrades. Real links, real landmarks. -->
    <nav class="rr-nav-fallback" :aria-label="navLabel">
      <a class="rr-brand" :href="home">
        <b>{{ brandTitle }}</b>
        <small>{{ brandSubtitle }}</small>
      </a>
      <ul class="rr-nav-links">
        <li v-for="item in primary" :key="item.href">
          <a
            :href="item.href"
            :aria-current="item.current ? 'page' : undefined"
            >{{ item.text }}</a
          >
        </li>
        <li v-for="item in utility" :key="item.href">
          <a :href="item.href">{{ item.text }}</a>
        </li>
        <li>
          <button type="button" class="rr-nav-iconbtn" @click="toggleTheme">
            {{ themeLabel }}
          </button>
        </li>
      </ul>
    </nav>
  </div>
</template>
