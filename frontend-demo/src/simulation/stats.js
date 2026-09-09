/**
 * Summaries over simulation results: per law how many subjects meet the
 * conditions and what they get on average, and the same split by a
 * demographic dimension (age, income, partner, business type, size).
 * Pure functions over the run's `results` array.
 */

/**
 * @typedef {object} LawResult
 * @property {boolean} ok        the engine produced outputs
 * @property {boolean|null} met  voldoet_aan_voorwaarden (null when the law has no such output)
 * @property {number|null} amount  primary output in euro (or its raw number), null when absent
 * @property {string|null} error
 */

export function mean(values) {
  const xs = values.filter((v) => typeof v === 'number' && Number.isFinite(v));
  return xs.length ? xs.reduce((s, v) => s + v, 0) / xs.length : null;
}

export function median(values) {
  const xs = values.filter((v) => typeof v === 'number' && Number.isFinite(v)).sort((a, b) => a - b);
  if (!xs.length) return null;
  const mid = Math.floor(xs.length / 2);
  return xs.length % 2 ? xs[mid] : (xs[mid - 1] + xs[mid]) / 2;
}

/**
 * @param {Array<{subject: object, laws: Record<string, LawResult>}>} results
 * @param {string} lawId
 */
export function summariseLaw(results, lawId) {
  const rows = results.map((r) => r.laws[lawId]).filter(Boolean);
  const evaluated = rows.filter((r) => r.ok);
  const eligible = evaluated.filter((r) => r.met !== false);
  const amounts = eligible.map((r) => r.amount).filter((v) => typeof v === 'number');
  return {
    // A law without `voldoet_aan_voorwaarden` (a tax, a registration) has no
    // notion of qualifying; it is computed for everyone.
    hasEligibility: evaluated.some((r) => r.met !== null),
    total: rows.length,
    evaluated: evaluated.length,
    errors: rows.length - evaluated.length,
    eligible: eligible.length,
    eligiblePct: evaluated.length ? (eligible.length / evaluated.length) * 100 : 0,
    avgAmount: mean(amounts),
    medianAmount: median(amounts),
    totalAmount: amounts.reduce((s, v) => s + v, 0),
    withAmount: amounts.length,
  };
}

/** Group results by a function of the subject and summarise one law per group. */
export function breakdown(results, lawId, groupOf, order = null) {
  const groups = new Map();
  for (const r of results) {
    const g = String(groupOf(r.subject));
    if (!groups.has(g)) groups.set(g, []);
    groups.get(g).push(r);
  }
  const keys = order ? order.filter((k) => groups.has(k)) : [...groups.keys()].sort();
  return keys.map((key) => ({ group: key, ...summariseLaw(groups.get(key), lawId) }));
}

export const CITIZEN_DIMENSIONS = [
  {
    id: 'leeftijd',
    label: 'Leeftijd',
    order: ['18-30', '30-45', '45-67', '67-85', '85+'],
    groupOf: (s) => (s.leeftijd < 30 ? '18-30' : s.leeftijd < 45 ? '30-45' : s.leeftijd < 67 ? '45-67' : s.leeftijd < 85 ? '67-85' : '85+'),
  },
  {
    id: 'inkomen',
    label: 'Inkomen',
    order: ['€ 0-20k', '€ 20-40k', '€ 40-60k', '€ 60k+'],
    groupOf: (s) => (s.inkomen < 20000 ? '€ 0-20k' : s.inkomen < 40000 ? '€ 20-40k' : s.inkomen < 60000 ? '€ 40-60k' : '€ 60k+'),
  },
  { id: 'partner', label: 'Partner', order: ['Met partner', 'Zonder partner'], groupOf: (s) => (s.partner ? 'Met partner' : 'Zonder partner') },
  { id: 'wonen', label: 'Wonen', order: ['Huur', 'Koop'], groupOf: (s) => (s.huurder ? 'Huur' : 'Koop') },
  { id: 'kinderen', label: 'Kinderen', order: ['Geen', '1', '2', '3+'], groupOf: (s) => (s.kinderen === 0 ? 'Geen' : s.kinderen >= 3 ? '3+' : String(s.kinderen)) },
];

export const BUSINESS_DIMENSIONS = [
  { id: 'type', label: 'Type bedrijf', order: ['horecabedrijf', 'slijtersbedrijf', 'overig'], groupOf: (s) => s.type },
  { id: 'grootte', label: 'Vloeroppervlakte', order: ['small', 'medium', 'large'], groupOf: (s) => s.grootte },
  { id: 'voedsel', label: 'Voedsel', order: ['Bereidt voedsel', 'Geen voedsel'], groupOf: (s) => (s.voedsel ? 'Bereidt voedsel' : 'Geen voedsel') },
  { id: 'rechtsvorm', label: 'Rechtsvorm', order: null, groupOf: (s) => s.rechtsvorm },
  { id: 'werknemers', label: 'Werknemers', order: ['Geen', '1-9', '10-49', '50+'], groupOf: (s) => (s.werknemers === 0 ? 'Geen' : s.werknemers < 10 ? '1-9' : s.werknemers < 50 ? '10-49' : '50+') },
];

/** Population-level facts for the header of a run. */
export function describePopulation(kind, subjects) {
  if (kind === 'ondernemers') {
    return {
      count: subjects.length,
      horecaPct: pct(subjects, (s) => s.type !== 'overig'),
      voedselPct: pct(subjects, (s) => s.voedsel),
      terrasPct: pct(subjects, (s) => s.terras),
      gemOppervlakte: mean(subjects.map((s) => s.oppervlakte)),
      metWerknemersPct: pct(subjects, (s) => s.werknemers > 0),
    };
  }
  return {
    count: subjects.length,
    gemLeeftijd: mean(subjects.map((s) => s.leeftijd)),
    partnerPct: pct(subjects, (s) => s.partner),
    huurderPct: pct(subjects, (s) => s.huurder),
    kinderenPct: pct(subjects, (s) => s.kinderen > 0),
    studentPct: pct(subjects, (s) => s.student),
    gemInkomen: mean(subjects.map((s) => s.inkomen)),
    medInkomen: median(subjects.map((s) => s.inkomen)),
  };
}

function pct(items, pred) {
  return items.length ? (items.filter(pred).length / items.length) * 100 : 0;
}

/** Flat rows for a CSV/JSON export: subject attributes plus per-law outcome. */
export function flattenResults(results, lawIds) {
  return results.map((r) => {
    const row = { ...r.subject };
    for (const id of lawIds) {
      const l = r.laws[id];
      const short = id.replaceAll('/', '_');
      row[`${short}__voldoet`] = l ? (l.ok ? l.met !== false : null) : null;
      row[`${short}__bedrag`] = l?.amount ?? null;
      if (l && !l.ok) row[`${short}__fout`] = l.error;
    }
    return row;
  });
}

export function toCsv(rows) {
  if (!rows.length) return '';
  const columns = [...new Set(rows.flatMap((r) => Object.keys(r)))];
  const cell = (v) => {
    if (v === null || v === undefined) return '';
    const s = typeof v === 'object' ? JSON.stringify(v) : String(v);
    return /[";\n]/.test(s) ? `"${s.replaceAll('"', '""')}"` : s;
  };
  return [columns.join(';'), ...rows.map((r) => columns.map((c) => cell(r[c])).join(';'))].join('\n');
}
