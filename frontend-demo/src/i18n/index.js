/**
 * Taal van de demo: Nederlands (de bron) en Engels (de vertaling).
 *
 * Dutch is the source of truth. Every key exists in `nl.js` first; `en.js` is
 * a translation of it, and `en.sources.js` records which Dutch string each
 * translation was made from, so a changed original shows up as a stale
 * translation instead of silently keeping an old English sentence. See
 * `i18n.test.js` for what that buys.
 *
 * Shaped after `useColorScheme()` from @regelrecht/frontend-shared, which this
 * app already consumes: a module-level ref, a readonly accessor, and writes
 * that go through one setter. The locale deliberately is NOT a component-scoped
 * value, because the strings are needed in plain modules that have no setup()
 * — `format.js` (imported by its own tests), `delegation.js`, `stats.js`.
 * That is also the reason this is not vue-i18n: its `useI18n()` only works
 * inside a component, so every one of those modules would need the parallel
 * `global.t` API instead.
 *
 * Both dictionaries are bundled eagerly. Lazy-loading a locale would be the
 * wrong trade for a tool that is driven live from a laptop: a language switch
 * halfway through a presentation must never show a loading state.
 */
import { computed, ref } from 'vue';
import en from './en.js';
import nl from './nl.js';

export const LOCALES = ['nl', 'en'];
export const DEFAULT_LOCALE = 'nl';

// Same key the docs landing page uses. The two run on separate subdomains
// (separate ZAD components), so they do not in fact share storage; the name
// matches so that anyone reading both finds the same thing.
//
// Alleen geschreven, niet gelezen: het inline script in `index.html` leest hem
// vóór de app start, en alleen op de kale `/`. Een deeplink houdt de taal van
// zijn eigen adres, want een link die iemand doorstuurt hoort niet af te hangen
// van wat de ontvanger ooit in een menu koos.
const STORAGE_KEY = 'rr-lang';

const DICTS = { nl, en };

const locale = ref(DEFAULT_LOCALE);

/** The active locale, for the plain modules that cannot call `useI18n()`. */
export function currentLocale() {
  return locale.value;
}

/**
 * The string for `key`, with `{placeholder}`s filled from `vars`.
 *
 * A key missing from the active locale falls back to Dutch, not to the key
 * itself: during a live demo a Dutch word among English ones is a blemish,
 * while `portaal.heading.lead` on screen is a visible defect. The fallback is
 * only safe because the parity test refuses to let it become a hiding place —
 * without that test this would quietly permit a half-translated app.
 */
export function t(key, vars) {
  const s = DICTS[locale.value]?.[key] ?? nl[key] ?? key;
  if (!vars) return s;
  return s.replace(/\{(\w+)\}/g, (whole, name) => (name in vars ? String(vars[name]) : whole));
}

/**
 * `t.plural(n, 'app.cases.pending')` → key `…​.one` or `…​.other`, with `{n}`.
 *
 * Both languages here are one/other, so this is a ternary rather than a plural
 * engine. The two forms exist on both sides even where one language does not
 * need the distinction (Dutch "1 te beoordelen" / "2 te beoordelen"), because
 * the key shape has to be the same in both dictionaries for the parity check.
 */
t.plural = (n, key, vars) => t(`${key}.${n === 1 ? 'one' : 'other'}`, { n, ...vars });

/**
 * Switch language and remember the choice.
 *
 * `document.documentElement.lang` is not decoration: screen readers pick their
 * voice from it, and it drives hyphenation. The one place that deliberately
 * overrides it back to `nl` is the pane showing verbatim statutory text.
 */
export function setLocale(next) {
  if (!LOCALES.includes(next)) return;
  locale.value = next;
  try {
    window.localStorage?.setItem(STORAGE_KEY, next);
  } catch {
    // Private mode, quota, storage disabled. The only consequence is that the
    // choice is not remembered on the next load.
  }
  if (typeof document !== 'undefined') document.documentElement.lang = next;
}

/**
 * Adopt a locale that came from the URL, without recording it as a choice.
 *
 * Opening a shared `/en/...` link shows English but must not overwrite the
 * reader's own preference — same rule the docs landing page follows, for the
 * same reason: a link someone sends you is not you picking a language.
 */
export function adoptLocale(next) {
  if (!LOCALES.includes(next) || locale.value === next) return;
  locale.value = next;
  if (typeof document !== 'undefined') document.documentElement.lang = next;
}

export function useI18n() {
  return { t, locale: computed(() => locale.value), setLocale };
}
