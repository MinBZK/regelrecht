/**
 * What keeps the two dictionaries honest.
 *
 * The demo is plain JavaScript, so there is no type checker to notice a
 * missing key the way `LandingContent` does for the docs landing page. These
 * tests are that backstop — and they go one step further, because a type
 * system cannot see a translation that is merely *out of date*.
 */
import { describe, expect, it } from 'vitest';
import en from './en.js';
import nl from './nl.js';
import sources from './en.sources.js';
import { hash } from './hash.js';
import { FEATURES } from '../store/demoStore.js';
import { DEFAULT_LOCALE, LOCALES, adoptLocale, currentLocale, setLocale, t } from './index.js';

/**
 * Keys whose English is legitimately identical to the Dutch: proper names, and
 * the language menu, which names each language in that language.
 */
const IDENTICAL_BY_DESIGN = new Set([
  'app.tabs.home',
  'app.demo.label',
  'app.features.label',
  'app.language.nl',
  'app.language.en',
  // "item" and "items" happen to be the same word in both languages. Both
  // forms still exist on both sides, because the key shape has to match.
  'format.items.one',
  'format.items.other',
  // Zelfde woord in beide talen. 'Curator' en 'mentor' zijn allebei juridische
  // termen die het Engels uit het Latijn heeft, net als het Nederlands.
  'sim.dimension.partner',
  'delegation.type.curator',
  'delegation.type.mentor',
  // Het Britse "postcode" is hetzelfde woord als het Nederlandse.
  'sheet.change.field.postcode',
  // Een bestandsformaat en een domeinnaam; die hebben geen vertaling.
  'wet.view.raw',
  'wet.source.link',
  // Alleen opmaak: de woorden zitten in de waarden die erin komen, niet in het
  // sjabloon zelf. Er valt hier niets te vertalen.
  'wet.tree.law_state',
  'wet.tile.outcome.missing',
]);

/** The `{placeholder}` names in a string, sorted. */
function placeholders(s) {
  return [...String(s).matchAll(/\{(\w+)\}/g)].map((m) => m[1]).sort();
}

describe('i18n parity', () => {
  it('every Dutch key has an English translation', () => {
    expect(Object.keys(nl).filter((k) => !(k in en))).toEqual([]);
  });

  it('English carries no key Dutch does not have', () => {
    // A key renamed on the Dutch side and left behind here would otherwise sit
    // in the file forever, looking translated and reaching nothing.
    expect(Object.keys(en).filter((k) => !(k in nl))).toEqual([]);
  });

  it('no English string is still the untranslated Dutch', () => {
    const same = Object.keys(nl).filter((k) => nl[k] === en[k] && !IDENTICAL_BY_DESIGN.has(k));
    expect(same).toEqual([]);
  });

  it('every key listed as identical by design really is identical', () => {
    // Keeps the exception list from outliving its reason: translate one of
    // these later and the entry has to go, rather than silently excusing it.
    expect([...IDENTICAL_BY_DESIGN].filter((k) => nl[k] !== en[k])).toEqual([]);
  });

  it('placeholders survive translation', () => {
    // A dropped `{n}` renders "to review" instead of "3 to review", and does
    // so silently.
    for (const key of Object.keys(nl)) {
      expect(placeholders(en[key]), key).toEqual(placeholders(nl[key]));
    }
  });

  it('no translation is stale', () => {
    const stale = Object.keys(nl).filter((k) => sources[k] !== hash(nl[k]));
    expect(
      stale,
      'De Nederlandse tekst is gewijzigd sinds deze vertaald zijn. Vertaal opnieuw en draai `node scripts/i18n-bless.mjs`.',
    ).toEqual([]);
  });

  it('the source hashes cover exactly the Dutch keys', () => {
    expect(Object.keys(sources).sort()).toEqual(Object.keys(nl).sort());
  });
});

describe('the feature flags', () => {
  it('every flag has a label in both dictionaries', () => {
    // App.vue renders these as `t(`app.features.${f.key}`)`, a template key
    // that check-i18n.mjs cannot see. A flag added to the store without a
    // label would render its own key in the menu.
    for (const f of FEATURES) {
      expect(nl[`app.features.${f.key}`], f.key).toBeTypeOf('string');
      expect(en[`app.features.${f.key}`], f.key).toBeTypeOf('string');
    }
  });

  it('carries no Dutch label of its own any more', () => {
    // The dictionaries are the single source; a `label` here would sit next to
    // them looking authoritative while nothing read it.
    for (const f of FEATURES) expect(f.label, f.key).toBeUndefined();
  });
});

describe('t', () => {
  it('returns the Dutch string by default', () => {
    expect(currentLocale()).toBe(DEFAULT_LOCALE);
    expect(t('app.tabs.wetten')).toBe('Wetten');
  });

  it('returns the English string once switched', () => {
    setLocale('en');
    expect(t('app.tabs.wetten')).toBe('Laws');
    setLocale('nl');
  });

  it('falls back to Dutch for a key the locale lacks', () => {
    // Not to the key itself: a Dutch word among English ones is a blemish,
    // `app.tabs.wetten` on screen is a defect.
    setLocale('en');
    expect(t('app.tabs.home')).toBe('Home');
    setLocale('nl');
  });

  it('returns the key when nothing has it', () => {
    expect(t('does.not.exist')).toBe('does.not.exist');
  });

  it('fills placeholders', () => {
    expect(t('app.cases.pending.other', { n: 3 })).toBe('3 te beoordelen');
  });

  it('leaves an unknown placeholder alone rather than blanking it', () => {
    expect(t('app.cases.pending.other', { other: 1 })).toBe('{n} te beoordelen');
  });

  it('picks the plural form by count', () => {
    setLocale('en');
    expect(t.plural(1, 'app.cases.pending')).toBe('1 to review');
    expect(t.plural(4, 'app.cases.pending')).toBe('4 to review');
    setLocale('nl');
  });

  it('refuses a locale it does not have', () => {
    setLocale('de');
    expect(currentLocale()).toBe('nl');
  });
});

describe('adoptLocale', () => {
  it('switches without recording a choice', () => {
    // A shared `/en/...` link shows English but must not overwrite the
    // reader's own preference.
    window.localStorage.removeItem('rr-lang');
    adoptLocale('en');
    expect(currentLocale()).toBe('en');
    expect(window.localStorage.getItem('rr-lang')).toBe(null);
    adoptLocale('nl');
  });

  it('ignores a locale it does not have', () => {
    adoptLocale('fr');
    expect(LOCALES).not.toContain('fr');
    expect(currentLocale()).toBe('nl');
  });
});
