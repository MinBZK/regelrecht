/**
 * Materialise register data for the demo laws.
 *
 * A demo law declares its external data as `source: {}` inputs; the schema says
 * nothing about where the value comes from. `corpus/demo/bindings.yaml` does:
 * per law and input it names the organisation, the register table, the column
 * and how to select the row. This module applies those bindings to a set of
 * register tables and produces, per law, the flat records the engine's scoped
 * data sources expect: one record per key value (a BSN, a KVK number), one
 * field per input name.
 *
 * The same code runs in the browser (persona data from `profiles.yaml`) and
 * in the scenario converter (data tables from the POC feature files), so the
 * two can never disagree about what a binding means.
 *
 * Pure JS, no dependencies.
 */

/**
 * @typedef {object} Binding
 * @property {'table'|'claim'|'cases'|'events'|'laws'|'reference_data'} kind
 * @property {string} service        organisation owning the data
 * @property {string} [table]        register table name
 * @property {string} [field]        column projected as the value
 * @property {string[]} [fields]     columns bundled into an object value
 * @property {{name: string, value: any}[]} [select_on]  row filter
 */

/**
 * @typedef {object} LawShape
 * @property {string} id
 * @property {string[]} parameters   declared parameter names
 * @property {Record<string, string>} inputTypes  input name -> declared type
 */

/** Read the parameters and input types of a parsed law document. */
export function lawShape(doc) {
  const parameters = new Set();
  const inputTypes = {};
  for (const article of doc.articles ?? []) {
    const execution = article.machine_readable?.execution;
    if (!execution) continue;
    for (const p of execution.parameters ?? []) parameters.add(p.name);
    for (const i of execution.input ?? []) inputTypes[i.name] = i.type ?? 'string';
  }
  return { id: doc.$id, parameters: [...parameters], inputTypes };
}

/** Parameters that identify whom a law is run for. */
const IDENTITY_KEYS = ['bsn', 'kvk_nummer'];

/**
 * Decide which parameter keys a law's records. A law run for a person or a
 * business is keyed on that identity (`bsn`, `kvk_nummer`) and nothing else:
 * an application-form parameter a binding also selects on (`$seizoen`,
 * `$terras_locatie`) would otherwise become a key of its own, and a source
 * keyed on it answers every call that passes the same form value, with a
 * record that never knew the identity. A law without an identity parameter
 * (a register keyed on an address or a year) is keyed on the parameter its
 * bindings select on most; without any, on its first parameter.
 */
export function keyFieldsFor(shape, lawBindings) {
  const counts = new Map();
  for (const binding of Object.values(lawBindings ?? {})) {
    for (const criterion of binding.select_on ?? []) {
      const ref = refName(criterion.value);
      if (ref && shape.parameters.includes(ref)) counts.set(ref, (counts.get(ref) ?? 0) + 1);
    }
  }
  const identity = IDENTITY_KEYS.filter((k) => counts.has(k));
  if (identity.length) return identity;
  if (counts.size === 0) return shape.parameters.length ? [shape.parameters[0]] : [];
  const [best] = [...counts.entries()].sort((a, b) => b[1] - a[1]);
  return [best[0]];
}

/** `$x` -> `x`, `$x.y` -> `x`; anything else -> null. */
function refName(value) {
  if (typeof value !== 'string' || !value.startsWith('$')) return null;
  return value.slice(1).split('.')[0];
}

/** Follow `a.b.c` into an object; undefined when a segment is missing. */
function getPath(obj, path) {
  let cur = obj;
  for (const seg of path.split('.')) {
    if (cur === null || cur === undefined) return undefined;
    cur = cur[seg];
  }
  return cur;
}

/** Loose equality across the string/number boundary a YAML table blurs. */
function sameValue(a, b) {
  if (a === null || a === undefined) return b === null || b === undefined;
  if (b === null || b === undefined) return false;
  if (typeof a === 'boolean' || typeof b === 'boolean') return String(a) === String(b);
  return String(a) === String(b);
}

const UNRESOLVED = Symbol('unresolved');

/**
 * Resolve a `select_on` value against the parameters and the inputs
 * materialised so far. Returns UNRESOLVED when it names an input that has no
 * value yet (dependency order), so the caller can retry later.
 */
function resolveCriterion(value, params, record, pending, ctx, shape) {
  if (typeof value !== 'string' || !value.startsWith('$')) return value;
  const path = value.slice(1);
  const head = path.split('.')[0];
  if (head in params) return getPath(params, path);
  if (head in record) return getPath(record, path);
  if (pending.has(head)) return UNRESOLVED;
  // A cross-law input of this law (`$vestigingsadres` from the KVK law): only
  // the engine knows its value. The caller may hand in a resolver for that;
  // without one there is nothing to select on.
  if (ctx?.resolveRef && shape) {
    const resolved = ctx.resolveRef(shape.id, head, params);
    if (resolved !== undefined) return getPath({ [head]: resolved }, path);
  }
  return undefined;
}

function rowMatches(row, selectOn, params, record, pending, ctx, shape) {
  for (const criterion of selectOn ?? []) {
    if (typeof criterion.value === 'object' && criterion.value !== null) {
      // An operation as filter value (one POC law used `IN`); not supported
      // by the materialiser - treat as "no constraint".
      continue;
    }
    const wanted = resolveCriterion(criterion.value, params, record, pending, ctx, shape);
    if (wanted === UNRESOLVED) return UNRESOLVED;
    if (!sameValue(row[criterion.name], wanted)) return false;
  }
  return true;
}

function project(row, binding) {
  if (binding.fields) {
    const obj = {};
    for (const f of binding.fields) obj[f] = row[f] ?? null;
    return obj;
  }
  if (binding.field) return row[binding.field] ?? null;
  return row;
}

/**
 * Materialise one law for one key value.
 *
 * @param {LawShape} shape
 * @param {Record<string, Binding>} lawBindings
 * @param {Record<string, any>} params    the law's parameters for this record
 * @param {(service: string, table: string) => any[]} rowsFor
 * @param {object} context  { cases?: any[] } for `kind: cases`
 * @returns {{record: Record<string, any>, sources: Record<string, string>}}
 *   record: input name -> value; sources: input name -> service
 */
export function materialiseRecord(shape, lawBindings, params, rowsFor, context = {}) {
  const record = {};
  const sources = {};
  const pending = new Set(Object.keys(lawBindings));

  // Inputs may select on other inputs (`$partner_bsn`); iterate until every
  // binding has been resolved or nothing can be resolved any more.
  let progressed = true;
  while (pending.size > 0 && progressed) {
    progressed = false;
    for (const name of [...pending]) {
      const binding = lawBindings[name];
      const type = shape.inputTypes[name] ?? 'string';
      let value;
      if (binding.kind === 'claim') {
        // Only a citizen can supply this; no register value exists.
        value = null;
      } else if (binding.kind === 'cases') {
        const cases = context.cases ?? [];
        const matched = [];
        let blocked = false;
        for (const c of cases) {
          const m = rowMatches(c, binding.select_on, params, record, pending, context, shape);
          if (m === UNRESOLVED) {
            blocked = true;
            break;
          }
          if (m) matched.push(c);
        }
        if (blocked) continue;
        value = matched;
      } else if (binding.kind === 'table') {
        const rows = rowsFor(binding.service, binding.table);
        const matched = [];
        let blocked = false;
        for (const row of rows) {
          const m = rowMatches(row, binding.select_on, params, record, pending, context, shape);
          if (m === UNRESOLVED) {
            blocked = true;
            break;
          }
          if (m) matched.push(row);
        }
        if (blocked) continue;
        if (type === 'array') {
          // Rows lacking the projected column contribute nothing to a list.
          value = matched.map((row) => project(row, binding)).filter((v) => v !== null && v !== undefined);
          // A single column that itself holds a list (e.g. `kinderen`) is the
          // list, not a list of lists.
          if (binding.field && value.length === 1 && Array.isArray(value[0])) value = value[0];
        } else if (matched.length === 0) {
          // No register row. A missing amount counts as zero, the way the POC's
          // ADD skipped absent operands; anything else stays unknown (null) so
          // the law's own null checks keep working.
          value = type === 'amount' || type === 'number' ? 0 : null;
        } else {
          value = project(matched[0], binding);
        }
      } else {
        // events / laws / reference_data: no register in the demo.
        value = null;
      }
      record[name] = value;
      sources[name] = binding.service;
      pending.delete(name);
      progressed = true;
    }
  }
  for (const name of pending) {
    record[name] = null;
    sources[name] = lawBindings[name].service;
  }
  return { record, sources };
}

/**
 * Materialise every law for every key value, grouped the way the engine wants
 * them registered: one scoped source per (law, service, key field).
 *
 * @param {Record<string, object>} laws           law id -> parsed YAML document
 * @param {Record<string, Record<string, Binding>>} bindings
 * @param {(service: string, table: string) => any[]} rowsFor
 * @param {Record<string, any[]>} keyValues       key field -> values to materialise
 *   (e.g. { bsn: ['100000001', ...], kvk_nummer: ['85234567'] })
 * @param {object} context  { referencedate?: string, cases?: any[],
 *   resolveRef?: (lawId, inputName, params) => value | undefined,
 *   paramsFor?: (keyField, keyValue) => Record<string, any> }  the optional
 *   resolver answers for cross-law inputs a `select_on` refers to; `paramsFor`
 *   supplies further parameters known for one key (an application form's
 *   answers), so bindings that select on them find their row
 * @returns {Array<{law: string, service: string, keyField: string, records: object[]}>}
 */
export function materialiseAll(laws, bindings, rowsFor, keyValues, context = {}) {
  const out = [];
  const year = context.referencedate ? Number(context.referencedate.slice(0, 4)) : undefined;
  for (const [lawId, lawBindings] of Object.entries(bindings)) {
    const doc = laws[lawId];
    if (!doc) continue;
    const shape = lawShape(doc);
    for (const keyField of keyFieldsFor(shape, lawBindings)) {
      const values = keyValues[keyField] ?? [];
      if (values.length === 0) continue;
      // Every binding is materialised under every key the law is keyed on. A
      // criterion that names a parameter other than this key (an application
      // form field like `$terras_locatie`) or a cross-law input has no value at
      // materialisation time and matches no row, so the input reads as null;
      // the engine's null propagation then leaves that branch unknown while the
      // register-backed branches still compute. Registering the same inputs
      // under a second key is harmless: a source whose key is absent from the
      // call's parameters simply finds no record and the registry moves on.
      const byService = new Map();
      for (const keyValue of values) {
        const params = { ...(context.paramsFor?.(keyField, keyValue) ?? {}), [keyField]: keyValue, referencedate: context.referencedate, year };
        const { record, sources } = materialiseRecord(shape, lawBindings, params, rowsFor, context);
        for (const [name, value] of Object.entries(record)) {
          const service = sources[name] ?? 'demo';
          if (!byService.has(service)) byService.set(service, new Map());
          const records = byService.get(service);
          if (!records.has(keyValue)) records.set(keyValue, { [keyField]: keyValue });
          records.get(keyValue)[name] = value;
        }
      }
      for (const [service, records] of byService) {
        out.push({ law: lawId, service, keyField, records: [...records.values()] });
      }
    }
  }
  return out;
}

/**
 * Build a `rowsFor(service, table)` accessor from the `profiles.yaml` shape:
 * `{ globalServices: {S: {T: rows}}, profiles: {bsn: {sources: {S: {T: rows}}}} }`.
 * Rows of every persona are concatenated per table, exactly like the POC did.
 */
export function tablesFromProfiles(profilesDoc) {
  const tables = new Map();
  const add = (service, table, rows) => {
    const key = `${service} ${table}`;
    if (!tables.has(key)) tables.set(key, []);
    const list = tables.get(key);
    for (const row of rows ?? []) if (!list.includes(row)) list.push(row);
  };
  for (const [service, byTable] of Object.entries(profilesDoc.globalServices ?? {})) {
    for (const [table, rows] of Object.entries(byTable ?? {})) add(service, table, rows);
  }
  for (const profile of Object.values(profilesDoc.profiles ?? {})) {
    for (const [service, byTable] of Object.entries(profile.sources ?? {})) {
      for (const [table, rows] of Object.entries(byTable ?? {})) add(service, table, rows);
    }
  }
  return (service, table) => tables.get(`${service} ${table}`) ?? [];
}

/** Collect every distinct value of `column` across all tables of a rowsFor set. */
export function collectKeyValues(rowsForTables, columns) {
  const out = {};
  for (const column of columns) {
    const seen = new Set();
    for (const rows of rowsForTables) {
      for (const row of rows) {
        const v = row[column];
        if (v !== null && v !== undefined && v !== '') seen.add(String(v));
      }
    }
    out[column] = [...seen];
  }
  return out;
}
