/**
 * Dutch display formatting for law values.
 *
 * Two kinds of "nothing" (RFC-036): `null` is an absence the data states
 * ("geen": no partner, no rent) and the engine's Unknown is a fact nobody
 * supplied ("onbekend"), which names what is missing. They are never shown
 * with the same word.
 */
import { isUnknown, missingFacts } from '@regelrecht/frontend-shared';

export { isUnknown, missingFacts };

const euro = new Intl.NumberFormat('nl-NL', { style: 'currency', currency: 'EUR' });
const number = new Intl.NumberFormat('nl-NL', { maximumFractionDigits: 4 });

/** Find the declared field (input/output/parameter) spec for a name in a law doc. */
export function fieldSpec(lawDoc, name) {
  for (const article of lawDoc?.articles ?? []) {
    const ex = article.machine_readable?.execution;
    if (!ex) continue;
    for (const list of [ex.output, ex.input, ex.parameters]) {
      const hit = (list ?? []).find((f) => f.name === name);
      if (hit) return hit;
    }
  }
  return null;
}

export function isAmountSpec(spec) {
  return spec?.type === 'amount' || spec?.type_spec?.unit === 'eurocent';
}

/** Format one value for display, guided by its declared spec when known. */
export function formatValue(value, spec = null) {
  if (isUnknown(value) || value === undefined) return 'onbekend';
  if (value === null) return 'geen';
  if (typeof value === 'boolean') return value ? 'Ja' : 'Nee';
  if (typeof value === 'number') {
    if (isAmountSpec(spec)) return euro.format(value / 100);
    if (spec?.type_spec?.unit === 'euro') return euro.format(value);
    if (spec?.type_spec?.unit === 'percentage') return `${number.format(value)}%`;
    if (spec?.type_spec?.unit === 'years' || spec?.type_spec?.unit === 'jaar') return `${number.format(value)} jaar`;
    return number.format(value);
  }
  if (typeof value === 'string') {
    if (/^\d{4}-\d{2}-\d{2}$/.test(value)) return formatDate(value);
    return value.replaceAll('_', ' ');
  }
  if (Array.isArray(value)) {
    if (value.length === 0) return 'geen';
    if (value.every((v) => typeof v !== 'object' || v === null)) return value.map((v) => formatValue(v)).join(', ');
    return `${value.length} ${value.length === 1 ? 'item' : 'items'}`;
  }
  if (typeof value === 'object') {
    if (typeof value.iso === 'string') return formatDate(value.iso);
    // A record reads best as its values in order (an address: street, number,
    // postcode, city); the keys are in the tooltip/edit sheet.
    const text = Object.values(value)
      .filter((v) => v !== null && v !== undefined && v !== '')
      .map((v) => formatValue(v))
      .join(' ');
    return text.length > 48 ? `${text.slice(0, 45)}…` : text;
  }
  return String(value);
}

/**
 * What an unknown value lacks: "ontbreekt: huurprijs, spaargeld (Wet
 * inkomstenbelasting)". The law is named only when it is not `ownLaw`; an
 * empty string for anything that is not unknown.
 */
export function formatMissing(value, { ownLaw = null, lawName = (id) => id } = {}) {
  const parts = [];
  for (const fact of missingFacts(value)) {
    const label = humanize(fact.name).toLowerCase();
    const part = fact.law && fact.law !== ownLaw ? `${label} (${lawName(fact.law)})` : label;
    if (!parts.includes(part)) parts.push(part);
  }
  return parts.length ? `ontbreekt: ${parts.join(', ')}` : '';
}

/**
 * The verdict of an evaluation's outputs: `true`/`false` when the law decided,
 * `'unknown'` when it could not for lack of facts, `null` when the law has no
 * `voldoet_aan_voorwaarden` output (a tax, a registration: computed for all).
 * An unknown verdict is never read as a yes.
 */
export function verdictOf(outputs) {
  if (!outputs || !('voldoet_aan_voorwaarden' in outputs)) return null;
  const v = outputs.voldoet_aan_voorwaarden;
  if (isUnknown(v)) return 'unknown';
  return v === true;
}

export function formatDate(iso) {
  const d = new Date(`${iso}T00:00:00`);
  if (Number.isNaN(d.getTime())) return iso;
  return d.toLocaleDateString('nl-NL', { day: 'numeric', month: 'long', year: 'numeric' });
}

export function formatDateTime(iso) {
  const d = new Date(iso);
  if (Number.isNaN(d.getTime())) return iso;
  return d.toLocaleString('nl-NL', { day: 'numeric', month: 'short', hour: '2-digit', minute: '2-digit' });
}

/** `hoogte_toeslag` -> `Hoogte toeslag`. */
export function humanize(name) {
  if (!name) return '';
  const s = String(name).replaceAll('_', ' ').trim();
  return s.charAt(0).toUpperCase() + s.slice(1);
}

/** Eurocent value for a monetary spec; the raw number otherwise. */
export function numericImpact(value, spec) {
  if (typeof value !== 'number') return 0;
  return isAmountSpec(spec) ? value / 100 : value;
}
