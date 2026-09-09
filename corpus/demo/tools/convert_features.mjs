#!/usr/bin/env node
/**
 * Convert the POC Gherkin feature files (Dutch step dialect, `features/steps/steps.py`)
 * into the canonical regelrecht BDD grammar (`bdd/grammar.yaml`) and write them
 * next to the migrated demo laws as `corpus/demo/regulation/nl/<law dir>/scenarios/<name>.feature`.
 *
 * The conversion is deterministic and re-runnable: every judgement call lives in
 * this file as a rule (the step tables below, `ADOPTED`, `WIP`), never in the
 * generated output. Regenerate with:
 *
 *     node corpus/demo/tools/convert_features.mjs <poc>/features corpus/demo/regulation/nl
 *
 * What happens per scenario:
 *  1. The POC Given steps become the calculation date and parameters.
 *  2. The POC register tables (`de volgende <SERVICE> <table> gegevens`) are fed
 *     through the demo materialiser (`frontend-demo/src/data/materialize.js`, the
 *     same code the demo app uses) with `corpus/demo/bindings.yaml`, producing the
 *     per-law records the engine's law-scoped data sources expect. One
 *     `the following "<service>" data with key "<key>" for law "<law>":` step is
 *     emitted for every law in the dependency closure of the evaluated law(s).
 *  3. Every POC When (`de <law> wordt uitgevoerd door <service>`) becomes
 *     `I evaluate outputs "<asserted outputs>" of "<law id>"`, followed by the
 *     assertions the POC Then steps translate to. A scenario may hold several
 *     such phases; parameters a later phase adds (a `met` table, or a claims
 *     table) are emitted right before its When.
 *
 * See corpus/demo/tools/CONVERSION_NOTES.md for the rules, counts and the
 * reasons behind every `@wip`.
 */

import { mkdirSync, readdirSync, readFileSync, statSync, writeFileSync } from 'node:fs';
import { createRequire } from 'node:module';
import { basename, dirname, join, relative, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';

import { keyFieldsFor, lawShape, materialiseRecord } from '../../../frontend-demo/src/data/materialize.js';

const here = dirname(fileURLToPath(import.meta.url));
const root = resolve(here, '..', '..', '..');
// js-yaml is a frontend dependency; resolve it from there (hoisted to the root node_modules).
const yaml = createRequire(join(root, 'frontend', 'package.json'))('js-yaml');

const SCRIPT_REL = 'corpus/demo/tools/convert_features.mjs';

// ---------------------------------------------------------------------------
// Rules that are not derivable from steps.py
// ---------------------------------------------------------------------------

/** POC feature files that test the case-management layer or the web UI, not the engine. */
const SKIP_FILES = [/^integratie\//, /^web\//, /^synthesize\.feature$/];

/** Scenario tags that mark non-engine scenarios in the POC. */
const SKIP_TAGS = new Set(['ui', 'browser', 'skip']);

/** Columns the POC kept as strings (steps.py STRING_FIELDS); `null`/empty become null. */
const STRING_FIELDS = new Set([
  'bsn', 'bsn_eigenaar', 'partner_bsn', 'jaar', 'kind_bsn', 'kvk_nummer', 'ouder_bsn', 'postcode',
  'huisnummer', 'bsn_gezagdrager', 'bsn_kind', 'bsn_mentor', 'bsn_betrokkene', 'bsn_curator',
  'bsn_curandus', 'bsn_bewindvoerder', 'bsn_rechthebbende', 'bsn_gevolmachtigde', 'bsn_volmachtgever',
  'bsn_executeur', 'bsn_erflater', 'bsn_familielid', 'bsn_patient',
]);

/** `een <entity> met ID "X"` -> parameter name (steps.py param_mapping). */
const ENTITY_PARAMS = { 'ICT-project': 'project_id', organisatie: 'organisatie_id', archiefstuk: 'archiefstuk_id', project: 'project_id' };

/** `is het <field> "<n>" eurocent` field aliases (steps.py _check_eurocent_field). */
const EUROCENT_FIELD_ALIASES = {
  bijdrage_inkomen: ['bijdrage_inkomen'],
  werkgeversbijdrage: ['werkgeversbijdrage_awf', 'zvw_werkgeversbijdrage'],
};

/**
 * Values adopted from the engine after a first run, keyed by POC feature path
 * and scenario name, then output name (`name@<phase>` to target one phase of a
 * multi-When scenario). Used where the POC asserted approximately (WW 1%,
 * kindgebonden budget 2%), qualitatively ("lager door hoog inkomen"), or read a
 * missing output as `false`. A `null` value emits `is null`; a boolean emits
 * `is true/false`. Each entry carries the POC's original intent as a comment.
 */
const ADOPTED = {
  'toeslagen/wet_op_het_kindgebonden_budget_TOESLAGEN-2025-01-01.feature': {
    'Paar met 2 kinderen krijgt aangepast bedrag zonder ALO-kop': { kindgebonden_budget_jaar: 392541 },
  },
};

/**
 * Scenarios that run but whose assertions do not hold against the migrated law,
 * for a reason that is not a conversion bug: key `<poc path>::<scenario>` -> reason.
 * They are emitted with `@wip` and the reason as a comment.
 */
const WIP = {
  'toeslagen/zorgtoeslagwet_TOESLAGEN-2025-01-01.feature::Persoon onder 18 heeft geen recht op zorgtoeslag':
    'POC data: born 2007-01-01, so 18 on the calculation date 2025-02-01; the engine finds voldoet_aan_voorwaarden true (leeftijd 18), the POC asserted "onder 18"',
};

// ---------------------------------------------------------------------------
// Gherkin (POC dialect) parsing
// ---------------------------------------------------------------------------

function splitTableRow(line) {
  const cells = [];
  let cur = '';
  for (let i = 1; i < line.length; i++) {
    const c = line[i];
    if (c === '\\') {
      const n = line[i + 1];
      if (n === '|') { cur += '|'; i++; } else if (n === 'n') { cur += '\n'; i++; } else if (n === '\\') { cur += '\\'; i++; } else cur += c;
    } else if (c === '|') {
      cells.push(cur.trim());
      cur = '';
    } else {
      cur += c;
    }
  }
  return cells;
}

function parseFeature(text) {
  const lines = text.split(/\r?\n/);
  const feature = { title: '', description: [], background: null, scenarios: [] };
  let pendingTags = [];
  let current = null;
  let lastStep = null;
  let inDescription = false;
  for (let i = 0; i < lines.length; i++) {
    const raw = lines[i];
    const line = raw.trim();
    if (line === '' || line.startsWith('#')) continue;
    if (line.startsWith('@')) { pendingTags.push(...line.split(/\s+/).map((t) => t.slice(1))); continue; }
    if (line.startsWith('|')) {
      if (!lastStep) throw new Error(`table without step at line ${i + 1}`);
      (lastStep.table ??= []).push(splitTableRow(line));
      continue;
    }
    let m;
    if ((m = /^Feature:\s*(.*)$/.exec(line))) { feature.title = m[1]; inDescription = true; continue; }
    if (/^Background:/.test(line)) { current = { steps: [] }; feature.background = current; lastStep = null; inDescription = false; continue; }
    if ((m = /^(Scenario|Scenario Outline|Example):\s*(.*)$/.exec(line))) {
      if (m[1] === 'Scenario Outline') throw new Error(`Scenario Outline not supported (line ${i + 1})`);
      current = { name: m[2], tags: pendingTags, steps: [] };
      pendingTags = [];
      feature.scenarios.push(current);
      lastStep = null;
      inDescription = false;
      continue;
    }
    if (/^Examples:/.test(line)) throw new Error(`Examples not supported (line ${i + 1})`);
    if ((m = /^(Given|When|Then|And|But|\*)\s+(.*)$/.exec(line))) {
      if (!current) throw new Error(`step outside scenario at line ${i + 1}`);
      let keyword = m[1];
      if (keyword === 'And' || keyword === 'But' || keyword === '*') keyword = lastStep ? lastStep.keyword : 'Given';
      lastStep = { keyword, text: m[2].trim(), table: null, line: i + 1 };
      current.steps.push(lastStep);
      continue;
    }
    if (inDescription) { feature.description.push(line); continue; }
    throw new Error(`unparsed line ${i + 1}: ${raw}`);
  }
  return feature;
}

// ---------------------------------------------------------------------------
// Demo laws
// ---------------------------------------------------------------------------

function walkFiles(dir, ext, out = []) {
  for (const entry of readdirSync(dir).sort()) {
    const p = join(dir, entry);
    if (statSync(p).isDirectory()) walkFiles(p, ext, out);
    else if (entry.endsWith(ext)) out.push(p);
  }
  return out;
}

function isoDate(v) {
  if (v instanceof Date) return v.toISOString().slice(0, 10);
  return String(v);
}

function analyseLaw(doc) {
  const outputs = new Map();
  const inputs = new Map();
  const parameters = [];
  const references = new Set();
  for (const article of doc.articles ?? []) {
    const execution = article.machine_readable?.execution;
    if (!execution) continue;
    for (const o of execution.output ?? []) outputs.set(o.name, o.type ?? 'string');
    for (const i of execution.input ?? []) {
      inputs.set(i.name, i);
      if (i.source?.regulation) references.add(i.source.regulation);
    }
    for (const p of execution.parameters ?? []) if (!parameters.includes(p.name)) parameters.push(p.name);
  }
  return { outputs, inputs, parameters, references };
}

function loadLaws(nlDir) {
  const versions = new Map();
  for (const file of walkFiles(nlDir, '.yaml')) {
    const doc = yaml.load(readFileSync(file, 'utf8'));
    if (!doc || !doc.$id) continue;
    const info = {
      id: doc.$id,
      file,
      relDir: relative(nlDir, dirname(file)),
      doc,
      validFrom: isoDate(doc.valid_from),
      service: doc.service,
      ...analyseLaw(doc),
    };
    if (!versions.has(doc.$id)) versions.set(doc.$id, []);
    versions.get(doc.$id).push(info);
  }
  for (const list of versions.values()) list.sort((a, b) => a.validFrom.localeCompare(b.validFrom));
  return versions;
}

/** The version of `id` valid at `date`: the latest `valid_from` not after it (else the earliest). */
function versionAt(versions, id, date) {
  const list = versions.get(id);
  if (!list) return null;
  let chosen = null;
  for (const v of list) if (v.validFrom <= date) chosen = v;
  return chosen ?? list[0];
}

function lawsAt(versions, date) {
  const out = {};
  for (const id of versions.keys()) out[id] = versionAt(versions, id, date).doc;
  return out;
}

/** Every law reachable from `ids` through `source.regulation`, the ids included. */
function closure(versions, ids, date) {
  const seen = new Set();
  const queue = [...ids];
  while (queue.length) {
    const cur = queue.shift();
    if (seen.has(cur)) continue;
    seen.add(cur);
    const v = versionAt(versions, cur, date);
    if (!v) continue;
    for (const r of v.references) if (!seen.has(r)) queue.push(r);
  }
  return seen;
}

// ---------------------------------------------------------------------------
// Values
// ---------------------------------------------------------------------------

/** steps.py parse_value: JSON, then int, then the string itself. */
function pocParseValue(s) {
  try {
    return JSON.parse(s);
  } catch {
    // not JSON
  }
  if (/^-?\d+$/.test(s)) return parseInt(s, 10);
  return s;
}

function pocCell(column, raw) {
  if (STRING_FIELDS.has(column)) return raw === 'null' || raw === '' ? null : raw;
  return pocParseValue(raw);
}

/** `€3.296,00` -> 329600 (steps.py parse_dutch_currency). */
function parseDutchCurrency(s) {
  return parseInt(s.replace(/€/g, '').replace(/\./g, '').replace(/,/g, '').trim(), 10);
}

/** Euro string -> eurocent integer. */
function euroToCents(s) {
  return Math.round(parseFloat(s) * 100);
}

function escapeCell(s) {
  return s.replace(/\\/g, '\\\\').replace(/\|/g, '\\|').replace(/\n/g, '\\n');
}

function formatCell(v) {
  if (v === null || v === undefined) return 'null';
  if (typeof v === 'boolean' || typeof v === 'number') return String(v);
  if (typeof v === 'object') return escapeCell(JSON.stringify(v));
  return escapeCell(String(v));
}

function renderTable(rows, indent) {
  const widths = [];
  for (const row of rows) row.forEach((c, i) => { widths[i] = Math.max(widths[i] ?? 0, c.length); });
  return rows.map((row) => `${indent}| ${row.map((c, i) => c.padEnd(widths[i])).join(' | ')} |`);
}

/**
 * A string for a quoted Gherkin capture (`"{x}"`). Neither runner unescapes:
 * the step patterns capture `"([^"]*)"` and hand the text over as is, so a
 * quote or a backslash inside the value cannot be written at all. Rather than
 * emit an escape that would be read literally, refuse such a value; it has to
 * go in a data-table cell (or the value has to change). No POC value needed
 * this so far.
 */
function quoted(value) {
  if (/["\\]/.test(value)) throw new Error(`value ${JSON.stringify(value)} contains a quote or backslash and cannot be a quoted Gherkin capture`);
  return value;
}

/** A parameter value as a single `parameter "x" is ...` step, or null when it needs a table cell. */
function parameterStep(name, value) {
  if (typeof value === 'number') return `parameter "${name}" is ${value}`;
  if (value === null) return `parameter "${name}" is "null"`;
  if (typeof value === 'boolean') return `parameter "${name}" is "${value}"`;
  if (typeof value === 'string') return `parameter "${name}" is "${quoted(value)}"`;
  return null;
}

/**
 * A materialised value the way the POC saw it. The materialiser projects a
 * missing column to `null` per matched row, so an array-typed input bound to a
 * column the scenario never supplied comes out as `[null, ...]`; the POC's
 * dataframe lookup gave None for that. FOREACH treats null as empty (RFC-016),
 * so null is the faithful rendering.
 */
function pocMaterialisedValue(v) {
  if (Array.isArray(v) && v.length > 0 && v.every((x) => x === null)) return null;
  return v;
}

// ---------------------------------------------------------------------------
// Step translation
// ---------------------------------------------------------------------------

/** One translated assertion. `op`: true | false | null | equals | contains | succeeds | comment | inexpressible | observe. */
function assertion(output, op, value, comment) {
  return { output, op, value, comment };
}

function comment(text) {
  return { op: 'comment', comment: text };
}

function inexpressible(output, text) {
  return { op: 'inexpressible', output, comment: text };
}

function observe(output, intent) {
  return { op: 'observe', output, comment: intent };
}

/**
 * Translate a `heeft de output "f" waarde "v"`-style expected value the way the
 * POC's `_check_output_field` read it: JSON first, then int, then string.
 */
function valueAssertion(output, raw, note) {
  const v = pocParseValue(raw);
  if (v === true) return assertion(output, 'true', undefined, note);
  if (v === false) return assertion(output, 'false', undefined, note);
  if (v === null) return assertion(output, 'null', undefined, note);
  if (typeof v === 'number') return assertion(output, 'equals', v, note);
  if (typeof v === 'string') return assertion(output, 'equals', v, note);
  return inexpressible(output, `POC: output "${output}" equals ${raw} (array/object literal not expressible in the canonical grammar)`);
}

function boolAssertion(output, negated, note) {
  return assertion(output, negated ? 'false' : 'true', undefined, note);
}

/** Pick the first output name the law actually declares, else the first candidate. */
function firstExisting(law, candidates) {
  return candidates.find((c) => law.outputs.has(c)) ?? candidates[0];
}

/**
 * Translate one POC Then step into assertion items, or null when the step is
 * unknown. `phase.wip` collects reasons the scenario cannot be asserted.
 */
function translateThen(text, law, phase) {
  let m;
  if (text === 'is voldaan aan de voorwaarden') {
    if (law.outputs.has('voldoet_aan_voorwaarden')) return [assertion('voldoet_aan_voorwaarden', 'true')];
    // steps.py: requirements_met = bool(outputs) -> the law produced something.
    return [assertion(null, 'succeeds', undefined, 'POC: requirements met (law has no voldoet_aan_voorwaarden output)')];
  }
  if (text === 'is niet voldaan aan de voorwaarden') {
    if (law.outputs.has('voldoet_aan_voorwaarden')) return [assertion('voldoet_aan_voorwaarden', 'false')];
    phase.wip.push('requirements not met — no voldoet_aan_voorwaarden output');
    return [comment('POC: requirements not met — no voldoet_aan_voorwaarden output')];
  }
  if ((m = /^ontbreken er (geen )?verplichte gegevens$/.exec(text))) {
    if (!m[1]) phase.missingData = true;
    return [comment(`POC: ${m[1] ? 'no ' : ''}required inputs missing (application-level check, not engine behaviour)`)];
  }

  // ----- euro amounts -----
  if ((m = /^is het toeslagbedrag "([^"]+)" euro$/.exec(text))) return [assertion(firstExisting(law, ['hoogte_toeslag', 'jaarbedrag']), 'equals', euroToCents(m[1]))];
  if ((m = /^is het pensioen "([^"]+)" euro$/.exec(text))) return [assertion(firstExisting(law, ['pensioen_uitkering_maandelijks', 'pensioenbedrag']), 'equals', euroToCents(m[1]))];
  if ((m = /^is het bijstandsuitkeringsbedrag "([^"]+)" euro$/.exec(text))) return [assertion('uitkeringsbedrag', 'equals', euroToCents(m[1]))];
  if ((m = /^is de woonkostentoeslag "([^"]+)" euro$/.exec(text))) return [assertion('woonkostentoeslag', 'equals', euroToCents(m[1]))];
  if ((m = /^is het startkapitaal "([^"]+)" euro$/.exec(text))) return [assertion('startkapitaal', 'equals', euroToCents(m[1]))];
  if ((m = /^is het bedrijfskapitaal_max "([^"]+)" euro$/.exec(text))) return [assertion('bedrijfskapitaal_max', 'equals', euroToCents(m[1]))];
  if ((m = /^is de huurtoeslag "([^"]+)" euro$/.exec(text))) return [assertion('subsidiebedrag', 'equals', euroToCents(m[1]))];
  if ((m = /^is (?:het|de) (.+?) "([^"]+)" eurocent$/.exec(text))) {
    const key = m[1].replace(/-/g, '_').toLowerCase();
    const candidates = EUROCENT_FIELD_ALIASES[key] ?? [key];
    return [assertion(firstExisting(law, candidates), 'equals', parseInt(m[2], 10))];
  }
  if ((m = /^zijn de (.+?) "([^"]+)" euro$/.exec(text))) return [assertion(m[1].replace(/ /g, '_').toLowerCase(), 'equals', euroToCents(m[2]))];

  // ----- generic output assertions -----
  if ((m = /^heeft de output "([^"]+)" waarde "(.*)"$/.exec(text))) return [valueAssertion(m[1], m[2])];
  if ((m = /^is het veld "([^"]+)" gelijk aan "(.*)"$/.exec(text))) return [valueAssertion(m[1], m[2])];
  if ((m = /^is de output "([^"]+)" (waar|onwaar)$/.exec(text))) return [boolAssertion(m[1], m[2] === 'onwaar')];
  if ((m = /^is de output "([^"]+)" leeg$/.exec(text)) || (m = /^is het veld "([^"]+)" een lege lijst$/.exec(text))) {
    // An empty list is a JSON literal; the quoted-value helper parses `[]`.
    return [assertion(m[1], 'equals', [], 'POC: output is an empty list')];
  }
  if ((m = /^bevat de output "([^"]+)" niet de waarde "(.*)"$/.exec(text))) {
    return [inexpressible(m[1], `POC: output "${m[1]}" does not contain ${m[2]} (negative membership not expressible in the canonical grammar)`)];
  }
  if ((m = /^bevat de output "([^"]+)" waarde "(.*)"$/.exec(text)) || (m = /^bevat het veld "([^"]+)" de waarde "(.*)"$/.exec(text))) {
    if (law.outputs.get(m[1]) === 'string') return [assertion(m[1], 'contains', m[2])];
    return [inexpressible(m[1], `POC: output "${m[1]}" contains ${m[2]} (array membership not expressible in the canonical grammar)`)];
  }

  // ----- domain sugar -----
  if ((m = /^heeft de persoon (geen )?recht op zorgtoeslag$/.exec(text))) return [boolAssertion('is_verzekerde_zorgtoeslag', Boolean(m[1]))];
  if ((m = /^heeft de persoon (geen )?recht op kinderopvangtoeslag$/.exec(text))) return [boolAssertion('is_gerechtigd', Boolean(m[1]))];
  if ((m = /^heeft de persoon (geen )?recht op WW$/.exec(text))) return [boolAssertion('heeft_recht_op_ww', Boolean(m[1]))];
  if ((m = /^is de WW duur "(\d+)" maanden$/.exec(text))) return [assertion('ww_duur_maanden', 'equals', parseInt(m[1], 10))];
  if ((m = /^is de WW uitkering per maand ongeveer "([^"]+)"$/.exec(text))) {
    return [{ ...assertion('ww_uitkering_per_maand', 'equals', parseDutchCurrency(m[1])), approx: `POC asserted approximately ${m[1]} (1% tolerance)` }];
  }
  if ((m = /^is de WW uitkering per maand maximaal "([^"]+)"$/.exec(text))) return [assertion('ww_uitkering_per_maand', 'equals', parseDutchCurrency(m[1]))];
  if (text === 'is de WW uitkering maximaal omdat het dagloon gemaximeerd is') return [assertion('ww_dagloon', 'equals', 29067, 'POC: dagloon capped at the maximum (€290,67)')];

  if ((m = /^is het ALO-kop bedrag "([^"]+)"$/.exec(text))) return [assertion('alo_kop_bedrag', 'equals', parseDutchCurrency(m[1]))];
  if ((m = /^is het (?:totale )?kindgebonden budget ongeveer "([^"]+)" per jaar$/.exec(text))) {
    return [{ ...assertion('kindgebonden_budget_jaar', 'equals', parseDutchCurrency(m[1])), approx: `POC asserted approximately ${m[1]} per year (2% tolerance)` }];
  }
  if (text === 'is het kindgebonden budget lager door hoog inkomen') return [observe('kindgebonden_budget_jaar', 'POC: budget reduced below the €8.502,00 maximum by the high income')];
  if (text === 'ontvangt de persoon de ALO-kop omdat deze alleenstaand is') {
    return [observe('heeft_partner', 'POC: single parent (no partner)'), observe('alo_kop_bedrag', 'POC: ALO-kop granted (> 0) because the parent is single')];
  }
  if (text === 'is het kindgebonden budget hoog door laag inkomen en meerdere kinderen') {
    return [observe('kindgebonden_budget_jaar', 'POC: budget above €8.000,00 for three children on a low income'), observe('inkomen_afbouw', 'POC: income reduction below €1.000,00')];
  }
  if (text === 'ontvangt de persoon extra bedragen voor kinderen 12+ en 16+') return [observe('kindgebonden_budget_jaar', 'POC: budget above €2.511,00 thanks to the age supplements for children 12+ and 16+')];
  if (text === 'is het kindgebonden budget maximaal door laag inkomen') return [observe('inkomen_afbouw', 'POC: income reduction below €500,00 (low income)')];

  // ----- LAA -----
  if ((m = /^genereert wet_brp\/laa (een|geen) signaal$/.exec(text))) return [boolAssertion('genereer_signaal', m[1] === 'geen')];
  if ((m = /^is het signaal_type "(.*)"$/.exec(text))) return [valueAssertion('signaal_type', m[1])];
  if ((m = /^is de reactietermijn_weken "(.*)"$/.exec(text))) return [valueAssertion('reactietermijn_weken', m[1])];
  if ((m = /^is de onderzoekstermijn_maanden "(.*)"$/.exec(text))) return [valueAssertion('onderzoekstermijn_maanden', m[1])];

  // ----- Archiefwet -----
  if (text === 'moet het archiefstuk overgebracht worden') return [boolAssertion('moet_overgebracht_worden', false)];
  if (text === 'hoeft het archiefstuk niet overgebracht te worden') return [boolAssertion('moet_overgebracht_worden', true)];
  if ((m = /^is de uiterste overbrengdatum "(.*)"$/.exec(text))) return [valueAssertion('uiterste_overbrengdatum', m[1])];
  if ((m = /^is het archiefstuk (niet )?openbaar$/.exec(text))) return [boolAssertion('is_openbaar', Boolean(m[1]))];
  if ((m = /^is de beperking reden "(.*)"$/.exec(text))) return [valueAssertion('beperking_reden', m[1])];
  if ((m = /^is het archiefstuk openbaar vanaf "(.*)"$/.exec(text))) return [valueAssertion(firstExisting(law, ['openbaar_vanaf', 'openbaar_vanaf_datum']), m[1])];
  if ((m = /^mag het archiefstuk (niet )?vernietigd worden$/.exec(text))) return [boolAssertion('mag_vernietigd_worden', Boolean(m[1]))];
  if ((m = /^mag het archiefstuk vernietigd worden vanaf "(.*)"$/.exec(text))) return [valueAssertion(firstExisting(law, ['vernietigingsdatum', 'vernietig_vanaf_datum']), m[1])];
  if ((m = /^is de reden van niet vernietigen "(.*)"$/.exec(text))) return [valueAssertion('reden_niet_vernietigen', m[1])];

  // ----- Bibob -----
  if ((m = /^is er (geen |een )advies uitgebracht$/.exec(text))) return [boolAssertion('advies_uitgebracht', m[1] === 'geen ')];
  if ((m = /^wordt verlening (niet )?geadviseerd$/.exec(text))) return [boolAssertion('verlening_geadviseerd', Boolean(m[1]))];
  if ((m = /^is weigering (niet )?mogelijk$/.exec(text))) return [boolAssertion('weigering_mogelijk', Boolean(m[1]))];
  if ((m = /^zijn voorschriften (niet )?mogelijk$/.exec(text))) return [boolAssertion('voorschriften_mogelijk', Boolean(m[1]))];
  if ((m = /^is de mate van gevaar "(.*)"$/.exec(text))) return [valueAssertion('mate_van_gevaar', m[1])];
  if (text === 'is er sprake van financieringsrisico') return [boolAssertion('financieringsrisico', false)];
  if (text === 'is er een relatie tot strafbare feiten') return [boolAssertion('relatie_strafbare_feiten', false)];

  // ----- adviesplicht / bestuursorgaan -----
  if ((m = /^valt het project (niet )?onder adviesplicht$/.exec(text))) return [boolAssertion('adviesplicht', Boolean(m[1]))];
  if ((m = /^is de organisatie (geen |een )bestuursorgaan$/.exec(text))) return [boolAssertion('is_bestuursorgaan', m[1] === 'geen ')];

  // ----- Bbz -----
  if ((m = /^is de categorie_zelfstandige "(.*)"$/.exec(text))) return [valueAssertion('categorie_zelfstandige', m[1])];
  if ((m = /^is de max_duur_maanden "(.*)"$/.exec(text))) return [valueAssertion('max_duur_maanden', m[1])];
  if ((m = /^is het bedrijfskapitaal_type "(.*)"$/.exec(text))) return [valueAssertion('bedrijfskapitaal_type', m[1])];

  return null;
}

// ---------------------------------------------------------------------------
// Given / When translation
// ---------------------------------------------------------------------------

/** Replace-or-append a parameter in an ordered list. */
function addParam(list, name, value, fromTable = false) {
  const existing = list.findIndex((p) => p.name === name);
  if (existing >= 0) list.splice(existing, 1);
  list.push({ name, value, fromTable });
}

/**
 * Translate one POC Given step. Parameters and the date go to `target`
 * (`{date, params, comments}`); register tables go to the shared `tables`.
 */
function translateGiven(step, tables, target, pocRel, warnings) {
  const { text, table } = step;
  let m;
  if ((m = /^de datum is "([^"]+)"$/.exec(text))) { target.date = m[1]; return; }
  if ((m = /^een persoon met BSN "([^"]+)"$/.exec(text))) { addParam(target.params, 'bsn', m[1]); return; }
  if ((m = /^een organisatie met KVK-nummer "([^"]+)"$/.exec(text)) || (m = /^een onderneming met KVK nummer "([^"]+)"$/.exec(text))) { addParam(target.params, 'kvk_nummer', m[1]); return; }
  if ((m = /^een werkgever met loonheffingennummer "([^"]+)"$/.exec(text))) { addParam(target.params, 'loonheffingennummer', m[1]); return; }
  if ((m = /^een werknemer met bruto jaarloon "([^"]+)" euro$/.exec(text))) { addParam(target.params, 'bruto_loon', parseFloat(m[1])); return; }
  if ((m = /^de aanvraag betreft een "([^"]+)"$/.exec(text))) { addParam(target.params, 'aanvraag_type', m[1]); return; }
  if ((m = /^een (.+) met ID "([^"]+)"$/.exec(text))) {
    const name = ENTITY_PARAMS[m[1]] ?? `${m[1].toLowerCase().replace(/-/g, '_')}_id`;
    addParam(target.params, name, m[2]);
    return;
  }
  if (/^een archiefstuk met de volgende eigenschappen:?$/.test(text)) {
    if (!table) throw new Error(`${pocRel}:${step.line}: archiefstuk step without table`);
    const [headings, ...rows] = table;
    for (const row of rows) headings.forEach((h, i) => addParam(target.params, h, pocParseValue(row[i]), true));
    return;
  }
  if (text === 'er is geen Bibob-advies uitgebracht voor deze onderneming') {
    target.comments.push('POC: no Bibob advice on record for this enterprise (no bibob_adviezen row applies)');
    return;
  }
  if ((m = /^de volgende (\S+) (\S+) gegevens:?$/.exec(text))) {
    if (!table) throw new Error(`${pocRel}:${step.line}: data step without table`);
    const [headings, ...rows] = table;
    // steps.py set_source_dataframe(service, table, df): a table given again
    // (in the scenario after the Background, or twice in one scenario) replaces
    // the earlier rows rather than adding to them.
    const key = `${m[1]} ${m[2]}`;
    const list = [];
    tables.set(key, list);
    for (const row of rows) {
      const record = {};
      headings.forEach((h, i) => { record[h] = pocCell(h, row[i] ?? ''); });
      list.push(record);
    }
    return;
  }
  warnings.push(`${pocRel}:${step.line}: unknown Given step: ${text}`);
  target.comments.push(`POC step not converted: ${text}`);
}

/**
 * Translate a POC When step. Returns `{law, service, params}` for an
 * evaluation, `{params}` for a claims table (the citizen supplies values that
 * the POC merged into the parameters), or null when unknown.
 */
function translateWhen(step) {
  let m;
  if ((m = /^de (\S+) wordt uitgevoerd door (\S+?)(?: met wijzigingen| met:?)?$/.exec(step.text))) {
    const params = [];
    if (step.table) {
      const [headings, ...rows] = step.table;
      if (headings.length === 2 && headings[0] === 'key' && headings[1] === 'value') {
        for (const row of rows) addParam(params, row[0], pocParseValue(row[1]), true);
      } else {
        for (const row of rows) headings.forEach((h, i) => addParam(params, h, pocParseValue(row[i]), true));
      }
    }
    return { law: m[1], service: m[2], params };
  }
  if (/^de burger (?:deze gegevens|een wijziging) indient:?$/.test(step.text) && step.table) {
    // steps.py submits a claim per row (`key` -> `nieuwe_waarde`); an approved
    // claim overrides the input of that name, exactly what a parameter does here.
    const [headings, ...rows] = step.table;
    const keyAt = headings.indexOf('key');
    const valueAt = headings.indexOf('nieuwe_waarde');
    if (keyAt < 0 || valueAt < 0) return null;
    const params = [];
    for (const row of rows) addParam(params, row[keyAt], pocParseValue(row[valueAt]), true);
    return { params, claims: true };
  }
  return null;
}

// ---------------------------------------------------------------------------
// Scenario conversion
// ---------------------------------------------------------------------------

/** Distinct values for every key field a binding can select on. */
function collectKeyValues(tables, params, allKeyFields) {
  const out = {};
  for (const field of allKeyFields) {
    const seen = new Map();
    const add = (v) => {
      if (v === null || v === undefined || v === '') return;
      const k = typeof v === 'object' ? JSON.stringify(v) : String(v);
      if (!seen.has(k)) seen.set(k, typeof v === 'object' ? v : String(v));
    };
    const p = params.find((x) => x.name === field);
    if (p) add(p.value);
    for (const rows of tables.values()) {
      for (const row of rows) {
        for (const [column, value] of Object.entries(row)) {
          if (column === field || (field === 'bsn' && /(^|_)bsn(_|$)/.test(column))) add(value);
        }
      }
    }
    out[field] = [...seen.values()];
  }
  return out;
}

/** `$x` -> `x`, `$x.y` -> `x`; anything else -> null. */
function refName(value) {
  if (typeof value !== 'string' || !value.startsWith('$')) return null;
  return value.slice(1).split('.')[0];
}

function getPath(obj, path) {
  let cur = obj;
  for (const seg of path.split('.')) {
    if (cur === null || cur === undefined) return undefined;
    cur = cur[seg];
  }
  return cur;
}

/**
 * Cross-law inputs a law's bindings select on (`adres: $vestigingsadres` in the
 * APV laws, `bsn: $partner_bsn` in wet_inkomstenbelasting). The materialiser
 * resolves `$x` against parameters and table-bound inputs only, so these are
 * pre-resolved here: the reference is followed to the law that produces the
 * output, and when that output is a plain projection of one of its own
 * table-bound inputs, that input is materialised for the same key. The POC
 * engine did this implicitly by resolving the cross-law input before the
 * register lookup.
 */
function crossLawRefs(law, lawBindings) {
  const refs = new Set();
  for (const binding of Object.values(lawBindings)) {
    for (const criterion of binding.select_on ?? []) {
      const ref = refName(criterion.value);
      if (ref && !law.parameters.includes(ref) && !lawBindings[ref] && law.inputs.get(ref)?.source?.regulation) refs.add(ref);
    }
  }
  return refs;
}

function preResolve(law, name, params, keyValues, versions, bindings, date, rowsFor, depth = 0) {
  if (depth > 3) return undefined;
  const source = law.inputs.get(name)?.source;
  if (!source?.regulation) return undefined;
  const target = versionAt(versions, source.regulation, date);
  if (!target) return undefined;
  const targetParams = { referencedate: date, year: Number(date.slice(0, 4)) };
  for (const [k, v] of Object.entries(source.parameters ?? {})) {
    if (typeof v === 'string' && v.startsWith('$')) {
      let value = getPath(params, v.slice(1));
      // A record keyed on another field (e.g. `terras_locatie`) still belongs to
      // the scenario's one enterprise or person: use that key value when unique.
      const head = v.slice(1).split('.')[0];
      if (value === undefined && keyValues[head]?.length === 1) value = getPath({ [head]: keyValues[head][0] }, v.slice(1));
      if (value === undefined) return undefined;
      targetParams[k] = value;
    } else {
      targetParams[k] = v;
    }
  }
  const outputName = source.output ?? name;
  let action = null;
  for (const article of target.doc.articles ?? []) {
    for (const a of article.machine_readable?.execution?.actions ?? []) if (a.output === outputName) action = a;
  }
  if (!action || typeof action.value !== 'string' || !action.value.startsWith('$')) return undefined;
  const [head, ...path] = action.value.slice(1).split('.');
  const targetBindings = bindings[target.id] ?? {};
  let value;
  if (targetBindings[head]) {
    value = materialiseRecord(lawShape(target.doc), targetBindings, targetParams, rowsFor, {}).record[head];
  } else if (target.inputs.get(head)?.source?.regulation) {
    value = preResolve(target, head, targetParams, keyValues, versions, bindings, date, rowsFor, depth + 1);
  } else {
    return undefined;
  }
  for (const seg of path) value = value === null || value === undefined ? undefined : value[seg];
  return value;
}

/** Property names the law reads on `$name.<prop>`, from every operation in its articles. */
function accessedProperties(law, name) {
  if (!law.accessed) law.accessed = new Map();
  if (!law.accessed.has(name)) {
    const props = new Set();
    const re = new RegExp(`\\$${name}\\.([a-z_][a-z0-9_]*)`, 'g');
    for (const m of JSON.stringify(law.doc.articles ?? []).matchAll(re)) props.add(m[1]);
    law.accessed.set(name, props);
  }
  return law.accessed.get(name);
}

/**
 * One POC reading the engine does not share, applied to a materialised record:
 * a register row was a dict, and `row.get(kolom)` on a column the table did
 * not have gave None; the engine treats a missing property as an author error,
 * so every property the law reads on an object input is present, null when
 * the row lacks it (the row exists, the field is empty).
 *
 * The POC's other reading, a numeric input without a value counting as 0, is
 * no longer applied: what a missing register row means is the binding's
 * `absent:` in bindings.yaml, and the materialiser writes exactly that
 * (RFC-036). An input the data does not state is left out of the record and
 * rendered as an empty cell, which the runner reads as "unknown".
 */
function applyPocValueSemantics(record, law, shape) {
  for (const [name, value] of Object.entries(record)) {
    const type = shape.inputTypes[name];
    if (type === 'object' && value && typeof value === 'object' && !Array.isArray(value)) {
      for (const prop of accessedProperties(law, name)) {
        if (!(prop in value)) value[prop] = null;
      }
    }
  }
}

/**
 * The materialiser's `materialiseAll`, with one POC-fidelity addition: the
 * pre-resolved cross-law selectors above. Same output shape: one entry per
 * (law, service, key field) with its records.
 */
function materialiseScenario(versions, bindings, rowsFor, keyValues, date) {
  const laws = lawsAt(versions, date);
  const year = Number(date.slice(0, 4));
  const out = [];
  for (const [lawId, lawBindings] of Object.entries(bindings)) {
    const doc = laws[lawId];
    if (!doc) continue;
    const law = versionAt(versions, lawId, date);
    const shape = lawShape(doc);
    const refs = crossLawRefs(law, lawBindings);
    for (const keyField of keyFieldsFor(shape, lawBindings)) {
      const values = keyValues[keyField] ?? [];
      if (values.length === 0) continue;
      // Only the bindings that select on this key field, or on no parameter at all.
      const relevant = {};
      for (const [name, binding] of Object.entries(lawBindings)) {
        const onParams = (binding.select_on ?? []).map((c) => refName(c.value)).filter((r) => r && shape.parameters.includes(r));
        if (onParams.length === 0 || onParams.includes(keyField)) relevant[name] = binding;
      }
      const byService = new Map();
      for (const keyValue of values) {
        const params = { [keyField]: keyValue, referencedate: date, year };
        for (const ref of refs) {
          const resolved = preResolve(law, ref, params, keyValues, versions, bindings, date, rowsFor);
          if (resolved !== undefined) params[ref] = resolved;
        }
        const { record, sources } = materialiseRecord(shape, relevant, params, rowsFor, {});
        applyPocValueSemantics(record, law, shape);
        for (const [name, value] of Object.entries(record)) {
          const service = sources[name] ?? 'demo';
          if (!byService.has(service)) byService.set(service, new Map());
          const records = byService.get(service);
          if (!records.has(keyValue)) records.set(keyValue, { [keyField]: keyValue });
          records.get(keyValue)[name] = value;
        }
      }
      for (const [service, records] of byService) out.push({ law: lawId, service, keyField, records: [...records.values()] });
    }
  }
  return out;
}

/** Emit the data-source steps for the materialised records of every law in `reachable`. */
function dataSourceLines(sources, reachable, given, indent) {
  const relevant = sources
    .filter((s) => reachable.has(s.law))
    .sort((a, b) => a.law.localeCompare(b.law) || a.service.localeCompare(b.service) || a.keyField.localeCompare(b.keyField));
  const lines = [];
  for (const s of relevant) {
    const fields = [];
    for (const r of s.records) for (const f of Object.keys(r)) if (f !== s.keyField && !fields.includes(f)) fields.push(f);
    if (fields.length === 0 || s.records.length === 0) continue;
    lines.push(given(`the following "${s.service}" data with key "${s.keyField}" for law "${s.law}":`));
    // A key the materialiser left out of a record (the data states nothing) is
    // an empty cell: the runner omits it and the input is unknown (RFC-036).
    const cell = (r, f) => (f in r ? formatCell(pocMaterialisedValue(r[f])) : '');
    const rows = [[s.keyField, ...fields], ...s.records.map((r) => [formatCell(r[s.keyField]), ...fields.map((f) => cell(r, f))])];
    lines.push(...renderTable(rows, indent));
  }
  return lines;
}

/** Turn translated items plus `ADOPTED` values into the final assertion list of one phase. */
function finaliseItems(items, adopted, phaseIndex) {
  return items.map((item) => {
    if (item.op === 'comment') return item;
    const key = Object.hasOwn(adopted, `${item.output}@${phaseIndex}`) ? `${item.output}@${phaseIndex}` : item.output;
    if (item.output && Object.hasOwn(adopted, key)) {
      const v = adopted[key];
      const note = item.approx ?? item.comment ?? 'POC asserted a different value';
      if (v === null) return assertion(item.output, 'null', undefined, `${note}; the engine leaves this output null`);
      if (typeof v === 'boolean') return assertion(item.output, v ? 'true' : 'false', undefined, note);
      return assertion(item.output, 'equals', v, `${note}; value adopted from the engine`);
    }
    if (item.op === 'observe') {
      // Placeholder that fails on purpose so the first run reports the value to adopt.
      return assertion(item.output, 'equals', -1, `${item.comment} — placeholder, adopt the engine value`);
    }
    return item;
  });
}

function assertionLine(item) {
  switch (item.op) {
    case 'succeeds': return 'the execution succeeds';
    case 'true': return `output "${item.output}" is true`;
    case 'false': return `output "${item.output}" is false`;
    case 'null': return `output "${item.output}" is null`;
    case 'contains': return `output "${item.output}" contains "${item.value}"`;
    case 'equals':
      if (typeof item.value === 'number') return `output "${item.output}" equals ${item.value}`;
      if (typeof item.value === 'object') return `output "${item.output}" equals "${JSON.stringify(item.value)}"`;
      return `output "${item.output}" equals "${quoted(String(item.value))}"`;
    default: throw new Error(`unhandled assertion op ${item.op}`);
  }
}

function convertScenario(scenario, base, pocRel, versions, bindings, allKeyFields, fallbackDate, warnings) {
  const tables = new Map([...base.tables].map(([k, v]) => [k, [...v]]));
  const params = base.params.map((p) => ({ ...p })); // running parameter state
  const scenarioGiven = { date: null, params: [], comments: [] };
  const phases = []; // {when, params, comments, thens, wip, missingData}
  let pending = { params: [], comments: [] };

  for (const step of scenario.steps) {
    if (step.keyword === 'Given') {
      const target = phases.length === 0 ? scenarioGiven : { date: null, params: [], comments: [] };
      translateGiven(step, tables, target, pocRel, warnings);
      if (phases.length > 0) {
        if (target.date) warnings.push(`${pocRel}:${step.line}: date change after a When is not supported`);
        pending.params.push(...target.params);
        pending.comments.push(...target.comments);
      }
    } else if (step.keyword === 'When') {
      const w = translateWhen(step);
      if (!w) return { skip: `unknown When step: ${step.text}` };
      pending.params.push(...w.params);
      if (w.claims) { pending.comments.push('POC: the citizen submitted these values as claims; they override the inputs of the same name'); continue; }
      phases.push({ when: w, params: pending.params, comments: pending.comments, thens: [], wip: [], missingData: false });
      pending = { params: [], comments: [] };
    } else {
      if (phases.length === 0) { warnings.push(`${pocRel}:${step.line}: Then before any When: ${step.text}`); continue; }
      phases[phases.length - 1].thens.push(step);
    }
  }
  if (phases.length === 0) return { skip: 'no When step' };
  for (const p of scenarioGiven.params) addParam(params, p.name, p.value, p.fromTable);
  const date = scenarioGiven.date ?? base.date ?? fallbackDate;

  // ----- resolve laws and translate assertions per phase -----
  const adopted = ADOPTED[pocRel]?.[scenario.name] ?? {};
  const wipReasons = [];
  let totalAssertions = 0;
  const lawIds = new Set();
  phases.forEach((phase, index) => {
    phase.law = versionAt(versions, phase.when.law, date);
    if (!phase.law) { phase.skip = `law "${phase.when.law}" not in the demo corpus`; return; }
    lawIds.add(phase.law.id);
    const items = [];
    for (const step of phase.thens) {
      const translated = translateThen(step.text, phase.law, phase);
      if (!translated) {
        warnings.push(`${pocRel}:${step.line}: unknown Then step: ${step.text}`);
        items.push(comment(`POC step not converted: ${step.text}`));
        continue;
      }
      items.push(...translated);
    }
    if (phase.missingData) {
      // The POC ran the law to establish that required data was still missing, an
      // application-level check. The engine has no equivalent; the phase is dropped
      // and the scenario continues with the data the citizen then supplied.
      phase.dropped = true;
      phase.items = [comment(`POC phase dropped: "${phase.when.law}" was run only to establish that required data was missing (${phase.thens.map((t) => t.text).join('; ')})`)];
      return;
    }
    phase.items = finaliseItems(items, adopted, index);
    phase.assertions = phase.items.filter((i) => i.op !== 'comment' && i.op !== 'inexpressible');
    totalAssertions += phase.assertions.length;
    wipReasons.push(...phase.wip);
    phase.outputs = [];
    for (const a of phase.assertions) if (a.output && !phase.outputs.includes(a.output)) phase.outputs.push(a.output);
    if (phase.outputs.length === 0) phase.outputs.push(phase.law.outputs.has('voldoet_aan_voorwaarden') ? 'voldoet_aan_voorwaarden' : [...phase.law.outputs.keys()][0]);
  });
  const unresolved = phases.find((p) => p.skip);
  if (unresolved) return { skip: unresolved.skip };
  if (totalAssertions === 0) wipReasons.push('no engine-checkable assertion');
  const wipKey = `${pocRel}::${scenario.name}`;
  if (WIP[wipKey]) wipReasons.push(WIP[wipKey]);

  // ----- materialise register data for every law reachable from any phase -----
  const allParams = params.map((p) => ({ ...p }));
  for (const phase of phases) for (const p of phase.params) addParam(allParams, p.name, p.value, p.fromTable);
  const rowsFor = (service, table) => tables.get(`${service} ${table}`) ?? [];
  const keyValues = collectKeyValues(tables, allParams, allKeyFields);
  const sources = materialiseScenario(versions, bindings, rowsFor, keyValues, date);
  const reachable = closure(versions, lawIds, date);

  // ----- emit -----
  const I = '    ';
  const T = '      ';
  const lines = [];
  for (const reason of wipReasons) lines.push(`  # @wip: ${reason}`);
  if (wipReasons.length) lines.push('  @wip');
  lines.push(`  Scenario: ${scenario.name}`);
  for (const c of scenarioGiven.comments) lines.push(`${I}# ${c}`);
  let firstGiven = true;
  const given = (text) => { const l = `${I}${firstGiven ? 'Given' : 'And'} ${text}`; firstGiven = false; return l; };
  if (scenarioGiven.date && scenarioGiven.date !== base.date) lines.push(given(`the calculation date is "${scenarioGiven.date}"`));

  const isBackgroundParam = (p) => base.params.some((b) => b.name === p.name && b.value === p.value && b.fromTable === p.fromTable);
  const emitParams = (list, declaredParams) => {
    const fresh = list.filter((p) => !isBackgroundParam(p));
    // Parameters the law declares but the POC never supplied were None there;
    // the engine has no such default, so they are passed as null explicitly.
    const missing = (declaredParams ?? []).filter((name) => !params.some((p) => p.name === name) && !fresh.some((p) => p.name === name));
    for (const p of fresh) addParam(params, p.name, p.value, p.fromTable);
    for (const name of missing) addParam(params, name, null);
    const scalar = fresh.filter((p) => !p.fromTable && parameterStep(p.name, p.value));
    const tabular = fresh.filter((p) => p.fromTable || !parameterStep(p.name, p.value));
    for (const p of scalar) lines.push(given(parameterStep(p.name, p.value)));
    if (tabular.length) {
      lines.push(given('the following parameters:'));
      lines.push(...renderTable(tabular.map((p) => [p.name, formatCell(p.value)]), T));
    }
    if (missing.length) {
      lines.push(`${I}# POC: parameter${missing.length > 1 ? 's' : ''} ${missing.map((n) => `"${n}"`).join(', ')} not provided by the scenario (None in the POC)`);
      for (const name of missing) lines.push(given(parameterStep(name, null)));
    }
  };

  emitParams(scenarioGiven.params, null);
  lines.push(...dataSourceLines(sources, reachable, given, T));
  phases.forEach((phase, index) => {
    for (const c of phase.comments) lines.push(`${I}# ${c}`);
    if (index > 0) firstGiven = true;
    emitParams(phase.params, phase.law.parameters);
    if (phase.dropped) { for (const item of phase.items) lines.push(`${I}# ${item.comment}`); return; }
    lines.push(`${I}When I evaluate outputs "${phase.outputs.join(', ')}" of "${phase.law.id}"`);
    let firstThen = true;
    const then = (text) => { lines.push(`${I}${firstThen ? 'Then' : 'And'} ${text}`); firstThen = false; };
    for (const item of phase.items) {
      if (item.op === 'comment' || item.op === 'inexpressible') { lines.push(`${I}# ${item.comment}`); continue; }
      if (item.comment) lines.push(`${I}# ${item.comment}`);
      then(assertionLine(item));
    }
    if (firstThen) then('the execution succeeds');
  });

  return { lines, law: phases[0].law, wip: wipReasons };
}

// ---------------------------------------------------------------------------
// Feature conversion
// ---------------------------------------------------------------------------

function convertFeature(pocRoot, pocFile, versions, bindings, allKeyFields, stats) {
  const pocRel = relative(pocRoot, pocFile);
  const feature = parseFeature(readFileSync(pocFile, 'utf8'));
  const fallbackDate = /-(\d{4}-\d{2}-\d{2})\.feature$/.exec(pocFile)?.[1] ?? '2024-01-01';
  const warnings = [];

  // Background: date, parameters and register tables shared by every scenario.
  const base = { date: null, params: [], comments: [], tables: new Map() };
  for (const step of feature.background?.steps ?? []) {
    if (step.keyword !== 'Given') throw new Error(`${pocRel}:${step.line}: non-Given step in Background`);
    translateGiven(step, base.tables, base, pocRel, warnings);
  }

  const body = [];
  let lawDir = null;
  const wipList = [];
  let count = 0;
  for (const scenario of feature.scenarios) {
    if (scenario.tags.some((t) => SKIP_TAGS.has(t))) { stats.skippedScenarios++; continue; }
    const result = convertScenario(scenario, base, pocRel, versions, bindings, allKeyFields, fallbackDate, warnings);
    if (result.skip) {
      warnings.push(`${pocRel}: scenario "${scenario.name}" skipped: ${result.skip}`);
      stats.skippedScenarios++;
      continue;
    }
    if (result.wip.length) { wipList.push({ scenario: scenario.name, reasons: result.wip }); stats.wip++; }
    stats.scenarios++;
    count++;
    lawDir ??= result.law.relDir;
    body.push('', ...result.lines);
  }
  if (!lawDir) return { pocRel, warnings, skipped: true };

  const out = [];
  out.push(`# Converted from ${pocRel} by ${SCRIPT_REL}`);
  out.push(`Feature: ${feature.title}`);
  for (const d of feature.description) out.push(`  ${d}`);
  const bgSteps = [];
  if (base.date) bgSteps.push(`the calculation date is "${base.date}"`);
  const bgTable = [];
  for (const p of base.params) {
    const step = p.fromTable ? null : parameterStep(p.name, p.value);
    if (step) bgSteps.push(step); else bgTable.push([p.name, formatCell(p.value)]);
  }
  if (bgSteps.length || bgTable.length) {
    out.push('', '  Background:');
    for (const c of base.comments) out.push(`    # ${c}`);
    let first = true;
    for (const s of bgSteps) { out.push(`    ${first ? 'Given' : 'And'} ${s}`); first = false; }
    if (bgTable.length) {
      out.push(`    ${first ? 'Given' : 'And'} the following parameters:`);
      out.push(...renderTable(bgTable, '      '));
    }
  }
  out.push(...body, '');
  return { pocRel, warnings, lawDir, name: basename(pocFile), text: out.join('\n'), wipList, scenarioCount: count };
}

// ---------------------------------------------------------------------------
// Main
// ---------------------------------------------------------------------------

function main() {
  const [pocFeaturesArg, nlDirArg] = process.argv.slice(2);
  if (!pocFeaturesArg || !nlDirArg) {
    console.error(`usage: node ${SCRIPT_REL} <poc>/features corpus/demo/regulation/nl`);
    process.exit(2);
  }
  const pocRoot = resolve(pocFeaturesArg);
  const nlDir = resolve(nlDirArg);
  const bindings = yaml.load(readFileSync(join(nlDir, '..', '..', 'bindings.yaml'), 'utf8'));
  const versions = loadLaws(nlDir);

  // Key fields any binding can select on (per law), unioned.
  const allKeyFields = new Set();
  const latestLaws = lawsAt(versions, '9999-12-31');
  for (const [lawId, lawBindings] of Object.entries(bindings)) {
    const doc = latestLaws[lawId];
    if (!doc) continue;
    for (const k of keyFieldsFor(lawShape(doc), lawBindings)) allKeyFields.add(k);
  }

  const stats = { features: 0, scenarios: 0, wip: 0, skippedScenarios: 0, skippedFiles: 0 };
  const warnings = [];
  const written = [];
  for (const pocFile of walkFiles(pocRoot, '.feature')) {
    const pocRel = relative(pocRoot, pocFile);
    if (SKIP_FILES.some((re) => re.test(pocRel))) { stats.skippedFiles++; continue; }
    const result = convertFeature(pocRoot, pocFile, versions, bindings, allKeyFields, stats);
    warnings.push(...result.warnings);
    if (result.skipped) { stats.skippedFiles++; continue; }
    const outDir = join(nlDir, result.lawDir, 'scenarios');
    mkdirSync(outDir, { recursive: true });
    const outFile = join(outDir, result.name);
    writeFileSync(outFile, result.text);
    stats.features++;
    written.push({ pocRel, out: relative(root, outFile), scenarios: result.scenarioCount, wip: result.wipList });
  }

  for (const w of warnings) console.error(`warning: ${w}`);
  for (const w of written) {
    console.log(`${w.pocRel} -> ${w.out} (${w.scenarios} scenarios, ${w.wip.length} @wip)`);
    for (const s of w.wip) console.log(`    @wip ${s.scenario}: ${s.reasons.join('; ')}`);
  }
  console.log(`\nfeatures: ${stats.features}, scenarios: ${stats.scenarios}, @wip: ${stats.wip}, skipped scenarios: ${stats.skippedScenarios}, skipped files: ${stats.skippedFiles}, warnings: ${warnings.length}`);
}

main();
