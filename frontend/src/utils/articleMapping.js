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
 * Builds a name -> datatype map for scenario parameter inputs, so each input
 * can render the control matching its declared type (boolean -> switch,
 * amount -> currency field, etc.). Merges execution.input and
 * execution.parameters; parameter types win on name collision since a
 * scenario `Given parameter` targets an execution parameter most directly.
 * Captures `type_spec.unit` so the amount branch can convert eurocents<->euros.
 *
 * @param {Array} articles - Articles array from useLaw()
 * @returns {Map<string, { type: string, unit: (string|null) }>}
 */
export function buildTypeMap(articles) {
  const typeMap = new Map();
  const add = (field) => {
    if (field?.name && field.type) {
      typeMap.set(field.name, { type: field.type, unit: field.type_spec?.unit ?? null });
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
 * Builds a fieldName -> { type, unit } map for EXTERNAL data-source fields —
 * inputs whose `source` is empty (no `regulation` and no `output`), i.e. raw
 * data the engine resolves by field name (e.g. insurance.verdragsinschrijving).
 * Spans a set of law docs (the current law + its loaded dependencies), because
 * data-source fields are declared by the leaf laws, not the law under test.
 * Last doc wins on a name collision. Drives typed cells in DataSourceTable.
 *
 * @param {Array<{articles?: Array}>} lawDocs - parsed law documents
 * @returns {Map<string, { type: string, unit: (string|null) }>}
 */
export function buildExternalFieldTypeMap(lawDocs) {
  const map = new Map();
  for (const doc of lawDocs || []) {
    for (const article of doc?.articles || []) {
      for (const f of article.machine_readable?.execution?.input || []) {
        const src = f.source;
        // External data-source fields are declared with an empty `source: {}`
        // (a `regulation` means cross-law, an `output` means internal). Match
        // exactly that — an empty object — rather than merely "no regulation and
        // no output", so a malformed source carrying other keys isn't
        // misclassified as external. (versionsCache YAML is not schema-checked.)
        const isExternal = src && typeof src === 'object' && Object.keys(src).length === 0;
        if (isExternal && f.name && f.type) {
          map.set(f.name, { type: f.type, unit: f.type_spec?.unit ?? null });
        }
      }
    }
  }
  return map;
}
