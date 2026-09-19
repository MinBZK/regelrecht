/**
 * The tunable constants of a law: the numeric `definitions` of its articles
 * (a threshold, a rate, an amount). The simulation lets the presenter change
 * them for one run ("what if the drempelinkomen goes up?") without touching
 * the corpus: the law document is cloned with the new values and reloaded in
 * the engine for the duration of the run.
 */
import * as yaml from 'js-yaml';

/**
 * @param {object} doc parsed law YAML
 * @returns {Array<{key: string, value: number, article: string}>}
 */
export function overridableDefinitions(doc) {
  const out = [];
  const seen = new Set();
  for (const article of doc?.articles ?? []) {
    const defs = article.machine_readable?.definitions ?? {};
    for (const [key, value] of Object.entries(defs)) {
      if (typeof value !== 'number' || !Number.isFinite(value) || seen.has(key)) continue;
      seen.add(key);
      out.push({ key, value, article: String(article.number ?? '') });
    }
  }
  return out;
}

/**
 * A copy of the law document with `definitions` replaced by the overrides,
 * serialised as YAML for `engine.loadLaw`. Keys the law does not define are
 * ignored; a non-numeric override is ignored too.
 */
export function applyOverrides(doc, overrides) {
  const copy = JSON.parse(JSON.stringify(doc));
  for (const article of copy.articles ?? []) {
    const defs = article.machine_readable?.definitions;
    if (!defs) continue;
    for (const [key, value] of Object.entries(overrides ?? {})) {
      if (key in defs && typeof defs[key] === 'number' && typeof value === 'number' && Number.isFinite(value)) defs[key] = value;
    }
  }
  return yaml.dump(copy, { lineWidth: -1, noRefs: true });
}

/** Only the overrides that differ from the law's own values. */
export function effectiveOverrides(doc, overrides) {
  const own = Object.fromEntries(overridableDefinitions(doc).map((d) => [d.key, d.value]));
  const out = {};
  for (const [key, value] of Object.entries(overrides ?? {})) {
    if (key in own && typeof value === 'number' && Number.isFinite(value) && value !== own[key]) out[key] = value;
  }
  return out;
}

/** Guess how a definition is displayed from its name and magnitude. */
export function definitionKind(key, value) {
  const k = key.toLowerCase();
  if (/percentage|percent|factor|tarief_perc|_pct/.test(k)) return 'percentage';
  if (/bedrag|drempel|inkomen|premie|grens|max|min|norm|kop|toeslag|vermogen|huur|kosten|tarief|vrijstelling|franchise|uitkering/.test(k) && Number.isInteger(value) && Math.abs(value) >= 100) {
    return 'eurocent';
  }
  return 'number';
}
