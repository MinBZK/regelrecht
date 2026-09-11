/**
 * Waarden en momenten leesbaar maken, zonder iets te beweren wat er niet staat.
 *
 * Een getal in het beeld draagt geen eenheid — de wereld geeft de waarde zoals
 * de cel haar vastlegde — dus er komt hier geen euroteken bij en er wordt niet
 * door honderd gedeeld. Wat je ziet is wat er in de kroniek staat.
 */
import { isUnknown, missingFacts } from '@regelrecht/frontend-shared';

/** Een ISO-dag (2027-04-01) in de Nederlandse notatie (01-04-2027). */
export function formatMoment(iso) {
  const match = /^(\d{4})-(\d{2})-(\d{2})/.exec(String(iso ?? ''));
  if (!match) return iso === null || iso === undefined ? '' : String(iso);
  const [, year, month, day] = match;
  return `${day}-${month}-${year}`;
}

/**
 * Eén waarde uit een gram of een antwoord, als tekst.
 *
 * Onbekend (RFC-036) is niet leeg: het is een feit dat bestaat en dat niemand
 * heeft aangeleverd, en dat hoort niet op een afwezigheid te lijken.
 */
export function formatValue(value) {
  if (isUnknown(value)) return 'nog niet bekend';
  if (value === null) return 'geen';
  if (value === undefined) return '';
  if (typeof value === 'boolean') return value ? 'ja' : 'nee';
  if (typeof value === 'number') return String(value);
  if (typeof value === 'string') return value;
  if (Array.isArray(value)) return `${value.length} ${value.length === 1 ? 'item' : 'items'}`;
  return JSON.stringify(value);
}

/**
 * Wat er bij een onbekende waarde ontbreekt, als regel; leeg voor elke andere
 * waarde, zodat een aanroeper geen `isUnknown`-wacht nodig heeft.
 */
export function formatMissing(value) {
  const missing = missingFacts(value);
  if (missing.length === 0) return '';
  return `ontbreekt: ${missing.map((fact) => fact.name).join(', ')}`;
}

/** Een veld- of celnaam als woorden: `verwacht_inkomen` → `Verwacht inkomen`. */
export function humanize(name) {
  const text = String(name ?? '').replace(/[_-]+/g, ' ').trim();
  return text ? text.charAt(0).toUpperCase() + text.slice(1) : '';
}
