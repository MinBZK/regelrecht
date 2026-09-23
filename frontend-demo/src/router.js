import { createRouter, createWebHistory } from 'vue-router';
import { DEFAULT_LOCALE, adoptLocale } from './i18n/index.js';

/**
 * One route per workspace tab, in two languages.
 *
 * Each page is one entry with a Dutch path and an English one under `/en/`.
 * The Dutch paths are the originals and keep working forever; `/en/*` is
 * additive. The English slugs are translated (`/en/laws`, not `/en/wetten`),
 * the way the landing page does it (`/en/signup`) — a half-translated URL
 * reads as unfinished work.
 *
 * A page is addressed by name, and that is what makes the rest cheap:
 * `router.resolve({ name: localeRouteName(page, locale), params })` yields the
 * path in the active locale, so the tab bar, the presentation deck and every
 * jump between views can name a page and stay language-agnostic. The docs
 * landing page instead maps counterparts with a hardcoded ternary that has to
 * be edited for every new page; this table is that mapping, derived.
 *
 * The name itself differs per language (`wetten` and `wetten:en`, so
 * vue-router can hold both), but both point at the *same* component, and that
 * is what `<keep-alive>` keys on. A language switch therefore reuses the
 * mounted view: an opened law tab or a running scenario survives it. Two
 * separate `() => import(…)` arrows would be two distinct functions and would
 * remount, which is why a page declares its component once.
 *
 * What the two genuinely share is `meta.page`. Views compare against that, not
 * against the route name.
 */
const PAGES = [
  { name: 'home', nl: '/', en: '/en', component: () => import('./views/HomeView.vue') },
  { name: 'presentatie', nl: '/presentatie', en: '/en/presentation', component: () => import('./views/PresentatieView.vue') },
  { name: 'wetten', nl: '/wetten/:lawId?', en: '/en/laws/:lawId?', component: () => import('./views/WettenView.vue') },
  { name: 'graaf', nl: '/graaf', en: '/en/graph', component: () => import('./views/GraafView.vue') },
  { name: 'scenarios', nl: '/scenarios/:featurePath(.*)?', en: '/en/scenarios/:featurePath(.*)?', component: () => import('./views/ScenariosView.vue') },
  { name: 'simulatie', nl: '/simulatie', en: '/en/simulation', component: () => import('./views/SimulatieView.vue') },
  { name: 'portaal', nl: '/portaal', en: '/en/portal', component: () => import('./views/PortaalView.vue') },
  { name: 'zaaksysteem', nl: '/zaaksysteem/:caseId?', en: '/en/cases/:caseId?', component: () => import('./views/ZaaksysteemView.vue') },
];

/** The locale a path belongs to, by its prefix. */
export function localeFromPath(path) {
  return path === '/en' || path.startsWith('/en/') ? 'en' : DEFAULT_LOCALE;
}

// vue-router matches in order and a name may appear once, so the two languages
// are separate records: the Dutch one keeps the bare name (it is the default
// and what `resolve({name})` should find first) and the English one is suffixed.
// `localeRouteName` is the single place that knows about that suffix.
const routes = [
  ...PAGES.map((p) => ({ path: p.nl, name: p.name, component: p.component, meta: { locale: 'nl', page: p.name } })),
  ...PAGES.map((p) => ({ path: p.en, name: `${p.name}:en`, component: p.component, meta: { locale: 'en', page: p.name } })),
  // An unknown path keeps the language it was typed in. Sending an English
  // visitor with a typo to the Dutch home page would be a silent demotion.
  { path: '/en/:pathMatch(.*)*', redirect: '/en' },
  { path: '/:pathMatch(.*)*', redirect: '/' },
];

/** The route name for `page` in `locale`. */
export function localeRouteName(page, locale) {
  return locale === 'en' ? `${page}:en` : page;
}

/**
 * The page a Dutch path from `demo-config.yaml` refers to.
 *
 * The presentation deck's slides carry literal paths (`route: /wetten`), and
 * those are authored content, not code — translating them in the config would
 * put routing knowledge in a file about slides. Instead the path is read back
 * to its page name here, and the caller resolves that name in whatever locale
 * is active.
 */
export function pageForConfigPath(path) {
  const clean = String(path || '').split('?')[0];
  const match = PAGES.find((p) => {
    const root = p.nl.split('/:')[0];
    return clean === root || clean.startsWith(`${root}/`);
  });
  return match?.name ?? null;
}

const router = createRouter({
  history: createWebHistory(),
  routes,
});

// The URL decides the language, not the other way round: opening a shared
// `/en/...` link has to show English. `adoptLocale` rather than `setLocale`,
// because arriving on a link someone sent you is not you choosing a language,
// so it must not overwrite a preference you set yourself.
router.beforeEach((to) => {
  adoptLocale(to.meta?.locale ?? localeFromPath(to.path));
});

export default router;
