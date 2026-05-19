<script setup lang="ts">
import { computed, watch } from 'vue'
import { useData, useRoute, Content } from 'vitepress'
// Internal but stable DefaultTheme composables/components. We deliberately
// do NOT use the DefaultTheme <Layout>; we rebuild the docs chrome inside
// the NLDD app-view/page shell (the proven polder pattern), but reuse
// VitePress' own sidebar resolution and search so config stays the single
// source of truth and search keeps working.
import { useSidebar } from 'vitepress/theme'
import { content, langFromPath } from './components/landing/content'
import { docsNav } from '../navLinks'
import LandingPage from './components/landing/LandingPage.vue'
import RrDocSidebar from './components/RrDocSidebar.vue'
import RrDocFooter from './components/RrDocFooter.vue'
import RrDocOutline from './components/RrDocOutline.vue'
import RrFooter from './components/landing/RrFooter.vue'

const route = useRoute()
const { frontmatter, isDark, page } = useData()
const { sidebarGroups, hasSidebar } = useSidebar()

const isLanding = computed(() => frontmatter.value.layout === 'landing')

// Landing is bilingual (path-driven); docs are English-only.
const lang = computed<'nl' | 'en'>(() =>
  isLanding.value ? langFromPath(route.path) : 'en'
)
const t = computed(() => content[lang.value])
const home = computed(() =>
  isLanding.value ? (lang.value === 'en' ? '/en/' : '/') : '/en/'
)
const brandSubtitle = computed(() =>
  isLanding.value ? t.value.nav.brandTagline : 'Machine-readable Dutch law'
)
const showSignup = computed(() => frontmatter.value.page === 'aanmelden')

interface NavItem {
  text: string
  href: string
  current?: boolean
}

const globalLinks = computed<NavItem[]>(() => {
  if (isLanding.value) {
    if (showSignup.value) return []
    return [
      { text: t.value.nav.what, href: home.value + '#what-is-it' },
      { text: t.value.nav.how, href: home.value + '#how-it-works' },
      { text: t.value.nav.tools, href: home.value + '#tools' },
      { text: t.value.nav.example, href: home.value + '#example' },
      { text: t.value.nav.faq, href: home.value + '#faq' },
      { text: t.value.nav.signup, href: t.value.feedback.ctaHref },
      { text: t.value.nav.docs, href: '/guide/what-is-regelrecht' },
    ]
  }
  return docsNav.map((n) => ({
    text: n.text,
    href: n.link,
    current: n.match ? route.path.startsWith(n.match) : false,
  }))
})

// Language switch lives on the right (utility) side of the bar. Only the
// landing is bilingual; docs are English-only so no switch there.
const langToggle = computed<NavItem | null>(() =>
  isLanding.value
    ? {
        text: lang.value === 'en' ? 'Nederlands' : 'English',
        href: lang.value === 'en' ? '/' : '/en/',
      }
    : null
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
const skipText = computed(() =>
  lang.value === 'en' ? 'Skip to content' : 'Direct naar de inhoud'
)

// NLDD colours resolve via CSS light-dark()/data-scheme. VitePress drives
// its own dark mode independently, so mirror isDark onto both so NLDD
// tracks the page exactly (otherwise cards/nav render dark on a light
// page). Single authority — no static color-scheme rule anywhere.
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

function openSearch() {
  if (typeof window === 'undefined') return
  // Trigger VitePress' own local search (its global hotkey listener is
  // mounted by the standalone <VPNavBarSearch> below).
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
  <a class="rr-skip-link" href="#rr-main">{{ skipText }}</a>

  <div class="rr-shell">
    <div class="rr-topbar">
      <nldd-top-navigation-bar
        logo-title="RegelRecht"
        :logo-subtitle="brandSubtitle"
        :logo-href="home"
        :website-href="home"
      >
        <nldd-menu-bar-item
          v-for="item in globalLinks"
          :key="item.href"
          slot="global"
          :href="item.href"
          :text="item.text"
          :current="item.current || undefined"
        />
        <nldd-menu-bar-item
          slot="utility"
          :text="searchLabel"
          icon="search"
          @click="openSearch"
        />
        <nldd-menu-bar-item
          slot="utility"
          :text="themeLabel"
          :icon="isDark ? 'sun' : 'moon'"
          @click="toggleTheme"
        />
        <nldd-menu-bar-item
          v-if="langToggle"
          slot="utility"
          :text="langToggle.text"
          :href="langToggle.href"
        />
        <nldd-menu-bar-item
          slot="utility"
          text="GitHub"
          href="https://github.com/MinBZK/regelrecht"
          icon="code"
        />
      </nldd-top-navigation-bar>
    </div>

    <!-- LANDING -->
    <div v-if="isLanding" id="rr-main" class="rr-landing" :lang="lang">
      <LandingPage />
      <RrFooter />
    </div>

    <!-- DOCS -->
    <div v-else id="rr-main" class="rr-docs" :class="{ 'has-sidebar': hasSidebar }">
      <RrDocSidebar v-if="hasSidebar" :groups="sidebarGroups" />
      <div class="rr-docs-body">
        <main class="vp-doc rr-docs-content">
          <Content />
          <RrDocFooter />
        </main>
      </div>
      <RrDocOutline />
    </div>
  </div>

  <!-- Mounted only for its search modal (teleports to <body>) and the
       global Cmd/Ctrl+K listener. Its own visible button is hidden — the
       visible trigger is the "Search" item in our nav, which dispatches
       the hotkey. Kept in the DOM (not display:none) so the component
       and its listener stay alive; just moved off-screen. -->
  <ClientOnly>
    <div class="rr-search-host" aria-hidden="true">
      <VPNavBarSearch />
    </div>
  </ClientOnly>
</template>

<style>
/* No nldd-app-view / nldd-page: those impose an internal scroll model
   that fights VitePress' #app wrapper (it gave a double scrollbar, then
   no scroll at all). The document scrolls normally — one scrollbar.
   nldd-top-navigation-bar only needs WIDTH for its container-query
   layout, which a plain full-width bar provides. */
html {
  /* Reserve the scrollbar's space permanently. When the VitePress search
     modal opens it sets overflow:hidden on the body; without this the
     scrollbar vanishes, the viewport widens ~15px and content jumps. */
  scrollbar-gutter: stable;
}
html,
body {
  margin: 0;
}
.rr-shell {
  min-height: 100vh;
}
.rr-topbar {
  background: var(--vp-nav-bg-color);
  border-bottom: 1px solid var(--vp-c-divider);
}
.rr-topbar nldd-top-navigation-bar {
  display: block;
  width: 100%;
}
/* Hide VPNavBarSearch's own trigger button (we use our nav's Search
   item). The search modal teleports to <body> so it is unaffected, and
   the Cmd/Ctrl+K listener is registered in script regardless of
   visibility — so display:none on the wrapper is safe and fully hides
   the stray button. */
.rr-search-host,
.rr-search-host .VPNavBarSearch {
  display: none;
}
.rr-docs.has-sidebar {
  display: grid;
  grid-template-columns: 272px minmax(0, 1fr) 240px;
  gap: 2rem;
  max-width: 1440px;
  margin-inline: auto;
  padding: 1.5rem 1.5rem 4rem;
}
.rr-docs:not(.has-sidebar) {
  max-width: 960px;
  margin-inline: auto;
  padding: 1.5rem 1.5rem 4rem;
}
.rr-docs-body {
  min-width: 0;
}
.rr-docs-content {
  min-width: 0;
}
@media (max-width: 1100px) {
  .rr-docs.has-sidebar {
    grid-template-columns: 240px minmax(0, 1fr);
  }
  .rr-docs.has-sidebar .rr-doc-outline {
    display: none;
  }
}
@media (max-width: 768px) {
  .rr-docs.has-sidebar {
    grid-template-columns: minmax(0, 1fr);
  }
  .rr-docs.has-sidebar .rr-doc-sidebar {
    display: none;
  }
}
</style>
