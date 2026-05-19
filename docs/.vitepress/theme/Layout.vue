<script setup lang="ts">
import { computed, watch, onMounted, onUnmounted } from 'vue'
import { useData, useRoute, Content } from 'vitepress'
// We replace the DefaultTheme <Layout> entirely with this NLDD-based
// shell, but reuse `useSidebar` (a public vitepress/theme export) so the
// sidebar stays config-driven, and mount VitePress' own search component
// so local search keeps working.
import { useSidebar } from 'vitepress/theme'
import { content, langFromPath } from './components/landing/content'
import { docsNav } from '../navLinks'
import LandingPage from './components/landing/LandingPage.vue'
import RrDocSidebar from './components/RrDocSidebar.vue'
import RrDocFooter from './components/RrDocFooter.vue'
import RrDocOutline from './components/RrDocOutline.vue'
import RrFooter from './components/landing/RrFooter.vue'
import RrNotFound from './components/RrNotFound.vue'

const route = useRoute()
const { frontmatter, isDark, page } = useData()
const { sidebarGroups, hasSidebar } = useSidebar()

const isNotFound = computed(() => page.value.isNotFound === true)
const isLanding = computed(
  () => !isNotFound.value && frontmatter.value.layout === 'landing'
)

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
const navLabel = computed(() =>
  lang.value === 'en' ? 'Main navigation' : 'Hoofdnavigatie'
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

  // VitePress' root app runs its own watchEffect (onMounted) that sets
  // document.documentElement.lang from the site config (always 'en' here,
  // since we intentionally don't use VitePress locales). That effect wins
  // any ordering race, so a plain watcher can't keep <html lang> correct
  // for the Dutch pages. A MutationObserver on the lang attribute
  // deterministically re-corrects it whenever it diverges from the route's
  // actual language — WCAG 3.1.1/3.1.2.
  const applyLang = () => {
    const want = lang.value
    if (document.documentElement.getAttribute('lang') !== want) {
      document.documentElement.setAttribute('lang', want)
    }
  }
  const langObserver = new MutationObserver(applyLang)
  watch(
    () => route.path,
    () => {
      applyLang()
      // Re-assert on the next frame too, after VitePress' own effect.
      requestAnimationFrame(applyLang)
    },
    { immediate: true }
  )
  onMounted(() => {
    applyLang()
    langObserver.observe(document.documentElement, {
      attributes: true,
      attributeFilter: ['lang'],
    })
  })
  onUnmounted(() => langObserver.disconnect())
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

      <!-- Semantic fallback: real <a> links rendered server-side, so the
           site is navigable without JS / before @nldd upgrades. Hidden
           once <nldd-top-navigation-bar> is :defined (see CSS). -->
      <nav class="rr-nav-fallback" :aria-label="navLabel">
        <a class="rr-nav-fallback-brand" :href="home">RegelRecht</a>
        <ul>
          <li v-for="item in globalLinks" :key="item.href">
            <a
              :href="item.href"
              :aria-current="item.current ? 'page' : undefined"
              >{{ item.text }}</a
            >
          </li>
          <li v-if="langToggle">
            <a :href="langToggle.href">{{ langToggle.text }}</a>
          </li>
          <li>
            <a href="https://github.com/MinBZK/regelrecht">GitHub</a>
          </li>
        </ul>
      </nav>
    </div>

    <!-- 404 -->
    <div v-if="isNotFound" id="rr-main" tabindex="-1" :lang="lang">
      <RrNotFound />
    </div>

    <!-- LANDING -->
    <div
      v-else-if="isLanding"
      id="rr-main"
      tabindex="-1"
      class="rr-landing"
      :lang="lang"
    >
      <LandingPage />
      <RrFooter />
    </div>

    <!-- DOCS -->
    <div
      v-else
      id="rr-main"
      tabindex="-1"
      class="rr-docs"
      :class="{ 'has-sidebar': hasSidebar }"
    >
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

  <!-- Mounted only for its search modal and the global Cmd/Ctrl+K
       listener; the visible trigger is the "Search" item in our nav.
       Hidden via display:none (see CSS) — safe because the hotkey
       listener binds to `window` via VueUse onKeyStroke (not to this
       element) and the modal teleports to <body>, so neither depends on
       this wrapper being visible. -->
  <ClientOnly>
    <div class="rr-search-host">
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

/* No-JS / pre-upgrade navigation fallback. The semantic <nav> is the
   SSR-rendered, always-functional navigation. Once @nldd loads and the
   web component upgrades (:defined), show the rich nav and hide the
   fallback. Without JS the fallback simply stays. */
.rr-nav-fallback {
  max-width: 1440px;
  margin-inline: auto;
  display: flex;
  flex-wrap: wrap;
  align-items: center;
  gap: 1rem 1.5rem;
  padding: 0.75rem 1.5rem;
}
.rr-nav-fallback-brand {
  font-weight: 700;
  font-size: 1.1rem;
  color: var(--vp-c-text-1);
  text-decoration: none;
}
.rr-nav-fallback ul {
  display: flex;
  flex-wrap: wrap;
  gap: 0.5rem 1.25rem;
  margin: 0;
  padding: 0;
  list-style: none;
}
.rr-nav-fallback a {
  color: var(--vp-c-text-2);
  text-decoration: none;
}
.rr-nav-fallback a:hover {
  color: var(--vp-c-brand-1);
}
.rr-nav-fallback a[aria-current='page'] {
  color: var(--vp-c-brand-1);
  font-weight: 600;
}
.rr-topbar nldd-top-navigation-bar:not(:defined) {
  display: none;
}
.rr-topbar nldd-top-navigation-bar:defined ~ .rr-nav-fallback {
  display: none;
}

/* Visible focus indicator across the whole shell (was scoped to
   .rr-landing only, leaving the docs sidebar/outline/footer with just
   the weak UA default — WCAG 2.2 SC 2.4.13). */
.rr-shell a:focus-visible,
.rr-shell button:focus-visible,
.rr-shell summary:focus-visible,
.rr-shell [tabindex]:focus-visible {
  outline: 3px solid var(--vp-c-brand-1);
  outline-offset: 2px;
  border-radius: 2px;
}
/* #rr-main is only a programmatic skip-link target; no visible ring. */
#rr-main:focus,
#rr-main:focus-visible {
  outline: none;
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
