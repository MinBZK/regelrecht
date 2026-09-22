/**
 * The routing table in two languages.
 *
 * The properties asserted here are the ones the rest of the app leans on: that
 * a page can be addressed by name and resolve to the active language, that
 * both languages reach the same component (which is what lets `<keep-alive>`
 * survive a switch), and that a wrong path keeps the language it was typed in.
 */
import { describe, expect, it } from 'vitest';
import router, { localeFromPath, localeRouteName, pageForConfigPath } from './router.js';

const PAGE_NAMES = ['home', 'presentatie', 'wetten', 'graaf', 'scenarios', 'simulatie', 'portaal', 'zaaksysteem'];

describe('localeFromPath', () => {
  it.each([
    ['/', 'nl'],
    ['/wetten', 'nl'],
    ['/en', 'en'],
    ['/en/', 'en'],
    ['/en/laws', 'en'],
    // Not every path that begins with the letters "en": only the segment.
    ['/energie', 'nl'],
  ])('%s is %s', (path, expected) => {
    expect(localeFromPath(path)).toBe(expected);
  });
});

describe('the route table', () => {
  it('has both languages for every page', () => {
    for (const name of PAGE_NAMES) {
      expect(router.hasRoute(name), name).toBe(true);
      expect(router.hasRoute(`${name}:en`), `${name}:en`).toBe(true);
    }
  });

  it('resolves a page name to the path of its language', () => {
    expect(router.resolve({ name: 'wetten' }).path).toBe('/wetten');
    expect(router.resolve({ name: 'wetten:en' }).path).toBe('/en/laws');
    expect(router.resolve({ name: 'home' }).path).toBe('/');
    expect(router.resolve({ name: 'home:en' }).path).toBe('/en');
  });

  it('resolves an optional param to a clean path, with no trailing slash', () => {
    // ScenariosView appends the raw file path to this, so a trailing slash
    // would produce `/scenarios//wet/...`.
    for (const name of ['wetten', 'scenarios', 'zaaksysteem']) {
      expect(router.resolve({ name }).path, name).not.toMatch(/\/$/);
      expect(router.resolve({ name: `${name}:en` }).path, name).not.toMatch(/\/$/);
    }
  });

  it('percent-encodes a slash inside a param, which the scenarios path must avoid', () => {
    // `scenarios` catches the rest of the path (`:featurePath(.*)`), and the
    // value is a file path with slashes in it. Passing it as a param turns
    // those into `%2F`, and ScenariosView then cannot find its own file.
    // The view therefore appends the raw path to `localePath('scenarios')`
    // instead of resolving with a param; this test pins why.
    const viaParam = router.resolve({ name: 'scenarios', params: { featurePath: 'wet/scenarios/x.feature' } }).path;
    expect(viaParam).toContain('%2F');
    expect(`${router.resolve({ name: 'scenarios' }).path}/wet/scenarios/x.feature`).toBe('/scenarios/wet/scenarios/x.feature');
    expect(`${router.resolve({ name: 'scenarios:en' }).path}/wet/scenarios/x.feature`).toBe('/en/scenarios/wet/scenarios/x.feature');
  });

  it('switches language on a scenario path without encoding the slashes', () => {
    // What App.vue's switchLocale does: take the tail of the current path
    // literally and hang it off the counterpart's root. Resolving with params
    // instead would percent-encode the slashes, and because the view is kept
    // alive nothing ever rewrites the address afterwards — the presenter is
    // left with an unreadable URL on screen.
    const current = '/scenarios/zorgtoeslagwet/scenarios/x.feature';
    const here = router.resolve({ name: 'scenarios' }).path;
    const tail = current.slice(here.length);
    const target = `${router.resolve({ name: 'scenarios:en' }).path}${tail}`;
    expect(target).toBe('/en/scenarios/zorgtoeslagwet/scenarios/x.feature');
    expect(target).not.toContain('%2F');
    // And it still resolves to the scenarios page with the path intact.
    const resolved = router.resolve(target);
    expect(resolved.meta.page).toBe('scenarios');
    expect(resolved.params.featurePath).toBe('zorgtoeslagwet/scenarios/x.feature');
  });

  it('keeps route params across languages', () => {
    const nl = router.resolve({ name: 'wetten', params: { lawId: 'zorgtoeslagwet' } });
    const en = router.resolve({ name: 'wetten:en', params: { lawId: 'zorgtoeslagwet' } });
    expect(nl.path).toBe('/wetten/zorgtoeslagwet');
    expect(en.path).toBe('/en/laws/zorgtoeslagwet');
  });

  it('points both languages of a page at the same component', () => {
    // This is what makes a language switch keep an opened law tab. App.vue
    // renders `<keep-alive><component :is="Component" /></keep-alive>`, which
    // keys on the resolved component rather than on the path, so as long as
    // both languages resolve to the very same lazy import the mounted view is
    // reused across a switch. Measured in the browser: a marker set on the
    // pane's DOM node before switching from /en/laws/zorgtoeslagwet is still
    // there afterwards, on /wetten/zorgtoeslagwet.
    //
    // Two separate `() => import(…)` arrows would be two distinct functions
    // and would remount, which is why PAGES holds one `component` per page.
    for (const name of PAGE_NAMES) {
      const nl = router.getRoutes().find((r) => r.name === name);
      const en = router.getRoutes().find((r) => r.name === `${name}:en`);
      expect(en.components.default, name).toBe(nl.components.default);
    }
  });

  it('gives both languages of a page the same meta.page', () => {
    // Views watch `meta.page` to decide whether a route is theirs. On
    // `route.name` they would compare against `wetten` while the English route
    // is called `wetten:en`, return early, and leave the tab empty — no error,
    // just a blank pane on every /en/ deep link.
    for (const name of PAGE_NAMES) {
      expect(router.resolve({ name }).meta.page, name).toBe(name);
      expect(router.resolve({ name: `${name}:en` }).meta.page, name).toBe(name);
    }
  });

  it('records the locale on each route', () => {
    expect(router.resolve({ name: 'graaf' }).meta.locale).toBe('nl');
    expect(router.resolve({ name: 'graaf:en' }).meta.locale).toBe('en');
  });

  it('sends an unknown path home in the language it was typed in', () => {
    // `resolve` reports the matched record, and a redirect is applied when
    // navigating, so the redirect target is what to assert here. A typo under
    // /en/ must not silently demote an English visitor to the Dutch demo.
    const nl = router.resolve('/bestaat-niet');
    const en = router.resolve('/en/does-not-exist');
    expect(nl.matched.at(-1).redirect).toBe('/');
    expect(en.matched.at(-1).redirect).toBe('/en');
  });
});

describe('the QR code target', () => {
  it('points at the home page of the language that is on', () => {
    // HomeView builds it as origin + the resolved path of the current route.
    // Someone scanning during an English talk has to land in English; the
    // fixed '/' it used to build would put them in the Dutch demo.
    expect(router.resolve({ name: 'home' }).path).toBe('/');
    expect(router.resolve({ name: 'home:en' }).path).toBe('/en');
  });
});

describe('localeRouteName', () => {
  it('suffixes only English', () => {
    expect(localeRouteName('wetten', 'nl')).toBe('wetten');
    expect(localeRouteName('wetten', 'en')).toBe('wetten:en');
  });
});

describe('pageForConfigPath', () => {
  it.each([
    ['/wetten', 'wetten'],
    ['/wetten/zorgtoeslagwet', 'wetten'],
    ['/graaf', 'graaf'],
    ['/scenarios/nl/wet/x.feature', 'scenarios'],
    ['/zaaksysteem/42', 'zaaksysteem'],
    ['/', 'home'],
  ])('%s is the %s page', (path, expected) => {
    expect(pageForConfigPath(path)).toBe(expected);
  });

  it('does not let the home page swallow every other path', () => {
    // `/` is a prefix of everything, so a naive startsWith would match it
    // first and send every slide to the landing page.
    expect(pageForConfigPath('/simulatie')).toBe('simulatie');
  });

  it('is null for a path no page owns', () => {
    expect(pageForConfigPath('/nergens')).toBe(null);
    expect(pageForConfigPath('')).toBe(null);
  });
});
