import { createRouter, createWebHistory } from 'vue-router';
import { DEFAULT_LOCALE, LOCALES, adoptLocale } from './i18n/index.js';

/**
 * One route per workspace tab, in every language.
 *
 * Each page is one entry with one path per language, keyed on the language
 * code. The Dutch paths are the originals and keep working forever; a prefixed
 * language is additive. The English slugs are translated (`/en/laws`, not `/en/wetten`),
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
  { name: 'home', paths: { nl: '/', en: '/en', fy: '/fy' }, component: () => import('./views/HomeView.vue') },
  { name: 'presentatie', paths: { nl: '/presentatie', en: '/en/presentation', fy: '/fy/presintaasje' }, component: () => import('./views/PresentatieView.vue') },
  { name: 'wetten', paths: { nl: '/wetten/:lawId?', en: '/en/laws/:lawId?', fy: '/fy/wetten/:lawId?' }, component: () => import('./views/WettenView.vue') },
  { name: 'graaf', paths: { nl: '/graaf', en: '/en/graph', fy: '/fy/graaf' }, component: () => import('./views/GraafView.vue') },
  { name: 'scenarios', paths: { nl: '/scenarios/:featurePath(.*)?', en: '/en/scenarios/:featurePath(.*)?', fy: '/fy/senarios/:featurePath(.*)?' }, component: () => import('./views/ScenariosView.vue') },
  { name: 'simulatie', paths: { nl: '/simulatie', en: '/en/simulation', fy: '/fy/simulaasje' }, component: () => import('./views/SimulatieView.vue') },
  { name: 'portaal', paths: { nl: '/portaal', en: '/en/portal', fy: '/fy/portaal' }, component: () => import('./views/PortaalView.vue') },
  { name: 'zaaksysteem', paths: { nl: '/zaaksysteem/:caseId?', en: '/en/cases/:caseId?', fy: '/fy/saaksysteem/:caseId?' }, component: () => import('./views/ZaaksysteemView.vue') },
];

// Elke taal uit de tabel moet elke pagina hebben. Een ontbrekend pad zou hier
// een route zonder `path` opleveren, en vue-router maakt daar een match op '/'
// van: de hele app zou dan op de home-pagina uitkomen, zonder foutmelding.
for (const l of LOCALES) {
  const missing = PAGES.filter((p) => !p.paths[l.code]).map((p) => p.name);
  if (missing.length) {
    throw new Error(`router: taal '${l.code}' mist een pad voor ${missing.join(', ')}`);
  }
}

/** The locale a path belongs to, by its prefix. */
export function localeFromPath(path) {
  // De bron heeft een lege prefix en matcht daarom op alles; die slaan we over
  // en gebruiken hem als terugval.
  const hit = LOCALES.find((l) => l.prefix && (path === l.prefix || path.startsWith(`${l.prefix}/`)));
  return hit?.code ?? DEFAULT_LOCALE;
}

// vue-router matches in order and a name may appear once, so the two languages
// are separate records: the Dutch one keeps the bare name (it is the default
// and what `resolve({name})` should find first) and the English one is suffixed.
// `localeRouteName` is the single place that knows about that suffix.
const routes = [
  ...LOCALES.flatMap((l) =>
    PAGES.map((p) => ({
      path: p.paths[l.code],
      name: localeRouteName(p.name, l.code),
      component: p.component,
      meta: { locale: l.code, page: p.name },
    })),
  ),
  // An unknown path keeps the language it was typed in. Sending an English
  // visitor with a typo to the Dutch home page would be a silent demotion.
  //
  // De volgorde binnen deze lijst maakt hier niet uit, en dat is nagemeten en
  // niet aangenomen: vue-router rangschikt zijn matchers op specificiteit, dus
  // `/en/:pathMatch(.*)*` gaat vóór `/:pathMatch(.*)*` ook als hij eronder
  // staat. `/en/typefout` komt dus hoe dan ook op `/en` uit.
  ...LOCALES.filter((l) => l.prefix).map((l) => ({ path: `${l.prefix}/:pathMatch(.*)*`, redirect: l.prefix })),
  { path: '/:pathMatch(.*)*', redirect: '/' },
];

/** The route name for `page` in `locale`. */
export function localeRouteName(page, locale) {
  return locale === DEFAULT_LOCALE ? page : `${page}:${locale}`;
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
    const root = p.paths[DEFAULT_LOCALE].split('/:')[0];
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
