/**
 * A scenario's steps as something to read, not as Gherkin.
 *
 * The `.feature` files stay in the canonical grammar (bdd/grammar.yaml); the
 * Rust runner and the browser runner read the same text. This module turns the
 * parsed steps of one scenario into display items: the reference date and the
 * parameters, one block per data source (organisation, law, key field, and a
 * record per row), what the engine is asked to compute, and what the scenario
 * expects of it. `renderStep` in gherkinNl.js stays the fallback for a step
 * this module has no shape for.
 *
 * Every item keeps `index`, the position of its step in the list it was built
 * from, so the view can put the run status of that step next to it.
 *
 * Two kinds of "nothing" stay apart here, as they do in the engine (RFC-036):
 * a cell that says `null` (or `[]`) is an absence the data states, shown as
 * "geen"; an empty cell states nothing, so the engine sees the field as
 * unknown, shown as "niet opgegeven". Folding the two together would hide the
 * very difference a scenario about missing data is testing.
 */
import { matchStep, tableCellValue, typedArgs } from '@regelrecht/frontend-shared/gherkin';
import { t } from '../i18n/index.js';
import { formatDate, formatValue, humanize, intlLocale, isAmountSpec } from './format.js';
import { renderStep } from './gherkinNl.js';

/** Units `formatValue` knows how to show; a number without one is shown as written. */
const UNITS = new Set(['euro', 'eurocent', 'percentage', 'years', 'jaar']);

/**
 * A number as it stands in the scenario, in the active locale's decimal sign
 * but without digit grouping. A BSN or a KvK number without a declared type is
 * an identifier, and `999.993.653` reads as an amount.
 */
function plainNumber(value) {
  return new Intl.NumberFormat(intlLocale(), { useGrouping: false, maximumFractionDigits: 4 }).format(value);
}

function isEmptyValue(value) {
  return value === null || (Array.isArray(value) && value.length === 0);
}

/**
 * One scenario value for display.
 *
 * Returns `{ text, quiet }`: `quiet` is `'none'` for a stated absence (`null`,
 * `[]`), `'zero'` for a number that is 0, `'not_stated'` for an empty cell and
 * `null` for anything with content. The view gathers the quiet ones into one
 * line per kind, so a record of 25 fields with two amounts in it shows the two
 * amounts.
 *
 * @param {unknown} value - the typed value, or `undefined` for an empty cell
 * @param {object|null} spec - the field's declared spec, when the law has one
 */
export function describeValue(value, spec = null) {
  if (value === undefined) return { text: t('scenario.value.not_stated'), quiet: 'not_stated' };
  if (isEmptyValue(value)) return { text: t('format.none'), quiet: 'none' };
  if (typeof value === 'number') {
    const text = isAmountSpec(spec) || UNITS.has(spec?.type_spec?.unit) ? formatValue(value, spec) : plainNumber(value);
    return { text, quiet: value === 0 ? 'zero' : null };
  }
  if (Array.isArray(value)) {
    // A list of records (children, addresses): each one as its fields, rather
    // than the "2 items" count formatValue gives where there is no room.
    return { text: value.map((item) => describeValue(item).text).join('; '), quiet: null };
  }
  if (typeof value === 'object') {
    // A record in one cell: every field with its name, the absent ones too.
    // formatValue drops the nulls and the keys and cuts at 45 characters,
    // which leaves an all-null record as an empty cell.
    const text = Object.entries(value)
      .map(([k, v]) => `${humanize(k)}: ${describeValue(v).text}`)
      .join(', ');
    return { text, quiet: null };
  }
  if (typeof value === 'string') {
    if (/^\d{4}-\d{2}-\d{2}$/.test(value)) return { text: formatDate(value), quiet: null };
    // As written: a code such as ONBEPAALDE_TIJD_REGULIER is what the engine
    // compares against, and formatValue would turn its underscores into spaces.
    return { text: value, quiet: null };
  }
  return { text: formatValue(value, spec), quiet: null };
}

/** A table cell for display; an empty cell is "not stated", not "none". */
export function describeCell(raw, spec = null) {
  if (raw.trim() === '') return describeValue(undefined, spec);
  return describeValue(tableCellValue(raw.trim()), spec);
}

/**
 * The rows of a header-row table as records of display fields.
 *
 * `key` names the column that identifies a record (`bsn`); it becomes the
 * record's title and leaves the field list. The quiet fields are gathered by
 * kind, in column order.
 */
export function describeRecords(table, { key = null, specFor = () => null } = {}) {
  if (!table || table.length < 2) return [];
  const headers = table[0].map((h) => h.trim());
  return table.slice(1).map((row) => {
    const record = { title: null, fields: [], none: [], zero: [], notStated: [] };
    headers.forEach((name, i) => {
      const cell = row[i] ?? '';
      const value = describeCell(cell, specFor(name));
      const label = humanize(name);
      if (key && name === key) {
        record.title = { label, text: value.text };
        return;
      }
      if (value.quiet === 'none') record.none.push(label);
      else if (value.quiet === 'zero') record.zero.push(label);
      else if (value.quiet === 'not_stated') record.notStated.push(label);
      else record.fields.push({ name, label, text: value.text });
    });
    return record;
  });
}

/**
 * The display items for a list of parsed steps.
 *
 * @param {Array<{keyword: string, text: string, dataTable?: string[][]}>} steps
 * @param {object} options
 * @param {(lawId: string) => string} options.lawName - a law id to its name
 * @param {(code: string) => string} options.serviceName - an organisation code to its name
 * @param {(lawId: string, field: string) => object|null} options.specFor - a field's declared spec
 * @param {string|null} options.law - the law the scenarios are about, for the
 *   specs of expected outputs before (or without) an evaluate step
 */
export function describeSteps(steps, { lawName = (id) => id, serviceName = (code) => code, specFor = () => null, law = null } = {}) {
  let currentLaw = law;
  return (steps ?? []).map((step, index) => {
    const match = matchStep(step.text);
    if (!match) return { kind: 'raw', index, ...renderStep(step) };
    const { entry } = match;
    const a = typedArgs(entry, match.args);
    const table = step.dataTable ?? null;
    switch (entry.id) {
      case 'set_calculation_date':
        return { kind: 'setting', index, icon: 'calendar', label: t('scenario.given.date'), text: describeValue(a[0]).text };
      case 'set_parameter_string':
      case 'set_parameter_number':
        return { kind: 'setting', index, icon: 'tag', label: humanize(a[0]), text: describeValue(a[1]).text };
      case 'set_parameters_table':
        return {
          kind: 'settings',
          index,
          rows: (table ?? []).map((row) => ({ icon: 'tag', label: humanize(row[0]?.trim()), text: describeCell(row[1] ?? '').text })),
        };
      case 'load_law':
        return { kind: 'setting', index, icon: 'book', label: t('scenario.given.law'), text: lawName(a[0]) };
      case 'set_data_source':
      case 'set_data_source_for_law': {
        const [org, key, forLaw = null] = a.map(String);
        return {
          kind: 'source',
          index,
          org,
          title: t('scenario.given.source', { org: serviceName(org) }),
          subtitle: forLaw ? t('scenario.given.source.for_law', { law: lawName(forLaw), key: humanize(key) }) : t('scenario.given.source.key', { key: humanize(key) }),
          records: describeRecords(table, { key, specFor: (name) => (forLaw ? specFor(forLaw, name) : null) }),
        };
      }
      case 'set_parameter_collection':
        return {
          kind: 'collection',
          index,
          title: humanize(a[0]),
          records: describeRecords(table, { specFor: (name) => specFor(currentLaw, name) }),
        };
      case 'evaluate':
      case 'evaluate_outputs': {
        currentLaw = String(a[1]);
        const outputs = String(a[0]).split(',').map((s) => s.trim()).filter(Boolean);
        return { kind: 'evaluate', index, law: currentLaw, title: lawName(currentLaw), outputs: outputs.map((name) => humanize(name, { lawId: currentLaw })) };
      }
      default:
        return describeExpectation(entry.id, a, { index, law: currentLaw, specFor, step });
    }
  });
}

/**
 * A label for the middle of a sentence: "Partner BSN" becomes "partner BSN".
 * Only the first letter, and not when the first word is an abbreviation
 * ("BSN" stays "BSN"); lowercasing the whole label would make that "bsn".
 */
function lowerFirst(label) {
  if (/^\p{Lu}{2}/u.test(label)) return label;
  return label.charAt(0).toLowerCase() + label.slice(1);
}

/** A Then step as "<output>: <expected value>", or a sentence for the steps about the run itself. */
function describeExpectation(id, a, { index, law, specFor, step }) {
  const output = (name) => humanize(name, { lawId: law });
  const expect = (label, text) => ({ kind: 'expect', index, label, text });
  switch (id) {
    case 'assert_boolean_true':
      return expect(output(a[0]), t('format.yes'));
    case 'assert_boolean_false':
      return expect(output(a[0]), t('format.no'));
    case 'assert_equals_number':
    case 'assert_equals_string':
      return expect(output(a[0]), describeValue(a[1], specFor(law, a[0])).text);
    case 'assert_null':
      return expect(output(a[0]), t('format.none'));
    case 'assert_unknown':
      return expect(output(a[0]), t('format.unknown'));
    case 'assert_unknown_for':
      return expect(output(a[0]), t('scenario.then.unknown_for', { fact: lowerFirst(humanize(a[1])) }));
    case 'assert_contains':
      return expect(output(a[0]), t('scenario.then.contains', { value: describeValue(a[1]).text }));
    case 'assert_succeeds':
      return expect(t('scenario.then.succeeds'), null);
    case 'assert_fails':
      return expect(t('scenario.then.fails'), null);
    case 'assert_fails_with':
      return expect(t('scenario.then.fails'), String(a[0]));
    case 'assert_exact_outputs':
      return expect(
        t('scenario.then.exact_outputs'),
        String(a[0])
          .split(',')
          .map((s) => output(s.trim()))
          .join(', '),
      );
    default:
      return { kind: 'raw', index, ...renderStep(step) };
  }
}

/**
 * The items in runs the view shows as one block each, in the order of the
 * scenario: the settings together, the data sources together, and every
 * evaluation with the expectations that follow it. A scenario can evaluate one
 * law, check it, and then evaluate the next (the Archiefwet scenarios do), so
 * the blocks follow the steps rather than sorting them by kind.
 */
export function groupItems(items) {
  const blocks = [];
  const last = () => blocks[blocks.length - 1];
  for (const item of items) {
    if (item.kind === 'setting' || item.kind === 'settings') {
      const rows = item.kind === 'setting' ? [item] : item.rows.map((row) => ({ ...row, index: item.index }));
      if (last()?.kind === 'settings') last().rows.push(...rows);
      else blocks.push({ kind: 'settings', rows });
    } else if (item.kind === 'source' || item.kind === 'collection') {
      if (last()?.kind === 'data') last().sources.push(item);
      else blocks.push({ kind: 'data', sources: [item] });
    } else if (item.kind === 'evaluate') {
      blocks.push({ kind: 'check', evaluate: item, expects: [] });
    } else if (item.kind === 'expect') {
      if (last()?.kind === 'check') last().expects.push(item);
      else blocks.push({ kind: 'check', evaluate: null, expects: [item] });
    } else if (last()?.kind === 'raw') {
      last().items.push(item);
    } else {
      blocks.push({ kind: 'raw', items: [item] });
    }
  }
  return blocks;
}
