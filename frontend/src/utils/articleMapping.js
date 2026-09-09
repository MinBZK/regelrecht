/**
 * Builds a mapping from output/input/parameter names to article numbers.
 *
 * @param {Array} articles - Articles array from useLaw()
 * @returns {{ outputToArticle: Map<string, string>, inputToArticle: Map<string, string>, paramToArticle: Map<string, string> }}
 */
export function buildArticleMap(articles) {
  const outputToArticle = new Map();
  const inputToArticle = new Map();
  const paramToArticle = new Map();

  for (const article of articles || []) {
    const exec = article.machine_readable?.execution;
    if (!exec) continue;
    const num = String(article.number);
    if (Array.isArray(exec.output)) {
      for (const o of exec.output) outputToArticle.set(o.name, num);
    }
    if (Array.isArray(exec.input)) {
      for (const i of exec.input) inputToArticle.set(i.name, num);
    }
    if (Array.isArray(exec.parameters)) {
      for (const p of exec.parameters) paramToArticle.set(p.name, num);
    }
  }

  return { outputToArticle, inputToArticle, paramToArticle };
}

/**
 * The form-relevant part of a field declaration (parameter or input): its
 * datatype, the unit of an amount, and whether `null` is one of its values
 * (`nullable`, schema v0.5.8, RFC-036). `nullable` is a boolean with schema
 * default false, so a declaration without the key is a field that is never
 * absent; the form then offers a value or a blank cell, not `null`.
 */
function fieldMeta(field) {
  return {
    type: field.type,
    unit: field.type_spec?.unit ?? null,
    nullable: field.nullable === true,
  };
}

/**
 * Builds a name -> datatype map for scenario parameter inputs, so each input
 * can render the control matching its declared type (boolean -> switch,
 * amount -> currency field, etc.). Merges execution.input and
 * execution.parameters; parameter types win on name collision since a
 * scenario `Given parameter` targets an execution parameter most directly.
 * Captures `type_spec.unit` so the amount branch can convert eurocents<->euros,
 * and `nullable` so the form only accepts a stated absence where the law
 * allows one.
 *
 * @param {Array} articles - Articles array from useLaw()
 * @returns {Map<string, { type: string, unit: (string|null), nullable: boolean }>}
 */
export function buildTypeMap(articles) {
  const typeMap = new Map();
  const add = (field) => {
    if (field?.name && field.type) {
      typeMap.set(field.name, fieldMeta(field));
    }
  };

  // Two passes across all articles: collect every input first, then let
  // parameters override on name collision (a scenario `Given parameter`
  // targets an execution parameter most directly). A single per-article pass
  // would let a later article's input clobber an earlier article's parameter.
  for (const article of articles || []) {
    for (const i of article.machine_readable?.execution?.input || []) add(i);
  }
  for (const article of articles || []) {
    for (const p of article.machine_readable?.execution?.parameters || []) add(p);
  }

  return typeMap;
}

/**
 * Builds an outputName -> { type, unit } map from `execution.output`, so result
 * views can format a value by its declared unit instead of guessing from the
 * name. First declaration wins on a name collision, matching the backend's
 * `collect_law_outputs` dedup.
 *
 * @param {Array} articles - Articles array from useLaw()
 * @returns {Map<string, { type: (string|null), unit: (string|null) }>}
 */
export function buildOutputTypeMap(articles) {
  const map = new Map();
  for (const article of articles || []) {
    for (const o of article.machine_readable?.execution?.output || []) {
      if (o?.name && !map.has(o.name)) {
        map.set(o.name, { type: o.type ?? null, unit: o.type_spec?.unit ?? null });
      }
    }
  }
  return map;
}

/**
 * Builds a fieldName -> { type, unit } map for EXTERNAL data-source fields -
 * inputs whose `source` is empty (no `regulation` and no `output`), i.e. raw
 * data the engine resolves by field name (e.g. insurance.verdragsinschrijving).
 * Spans a set of law docs (the current law + its loaded dependencies), because
 * data-source fields are declared by the leaf laws, not the law under test.
 * Last doc wins on a name collision. Drives typed cells in DataSourceTable.
 *
 * @param {Array<{articles?: Array}>} lawDocs - parsed law documents
 * @returns {Map<string, { type: string, unit: (string|null), nullable: boolean }>}
 */
export function buildExternalFieldTypeMap(lawDocs) {
  const map = new Map();
  for (const doc of lawDocs || []) {
    for (const article of doc?.articles || []) {
      for (const f of article.machine_readable?.execution?.input || []) {
        const src = f.source;
        // External data-source fields are declared with an empty `source: {}`
        // (a `regulation` means cross-law, an `output` means internal). Match
        // exactly that - an empty object - rather than merely "no regulation and
        // no output", so a malformed source carrying other keys isn't
        // misclassified as external. (versionsCache YAML is not schema-checked.)
        const isExternal = src && typeof src === 'object' && Object.keys(src).length === 0;
        if (isExternal && f.name && f.type) {
          map.set(f.name, fieldMeta(f));
        }
      }
    }
  }
  return map;
}
