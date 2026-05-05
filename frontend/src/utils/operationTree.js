export const OPERATION_LABELS = {
  // Rekenkundig
  ADD: 'optellen',
  SUBTRACT: 'aftrekken',
  MULTIPLY: 'vermenigvuldigen',
  DIVIDE: 'delen',
  MIN: 'minimum',
  MAX: 'maximum',
  // Vergelijking
  EQUALS: 'gelijk aan',
  GREATER_THAN: 'groter dan',
  GREATER_THAN_OR_EQUAL: 'groter dan of gelijk',
  LESS_THAN: 'kleiner dan',
  LESS_THAN_OR_EQUAL: 'kleiner dan of gelijk',
  IN: 'in lijst',
  // Logisch
  AND: 'en',
  OR: 'of',
  NOT: 'niet',
  // Conditioneel
  IF: 'als/dan',
  SWITCH: 'keuze',
  // Datum
  AGE: 'leeftijd',
  DATE_ADD: 'datum optellen',
  DATE: 'datum',
  DAY_OF_WEEK: 'dag van de week',
  // Verzameling
  LIST: 'lijst',
};

export function collectAvailableVariables(article) {
  // Engine built-in context variables — always available regardless of
  // article content. `referencedate` is the calculation date injected by
  // the engine from the scenario's "Given the calculation date is ..."
  const vars = [
    { name: 'referencedate', ref: '$referencedate', category: 'Context' },
  ];

  if (!article?.machine_readable) return vars;
  const mr = article.machine_readable;

  if (mr.definitions) {
    for (const name of Object.keys(mr.definitions)) {
      vars.push({ name, ref: `$${name}`, category: 'Definitie' });
    }
  }

  const execution = mr.execution;
  if (!execution) return vars;

  if (execution.input) {
    for (const input of execution.input) {
      vars.push({ name: input.name, ref: `$${input.name}`, category: 'Input' });
    }
  }

  if (execution.parameters) {
    for (const param of execution.parameters) {
      vars.push({ name: param.name, ref: `$${param.name}`, category: 'Parameter' });
    }
  }

  if (execution.actions) {
    for (const action of execution.actions) {
      if (action.output) {
        vars.push({ name: action.output, ref: `$${action.output}`, category: 'Actie' });
      }
    }
  }

  return vars;
}

export function buildOperationTree(action) {
  if (!action) return [];

  let rootNode;
  if (action.operation) {
    rootNode = action;
  } else if (action.value && typeof action.value === 'object' && action.value.operation) {
    rootNode = action.value;
  } else {
    return [];
  }

  // Onderwerp/Waarde rows keep YAML form verbatim — identifiers must match
  // their `$VAR` references. But the Titel row is a human-readable LABEL,
  // not a snippet of YAML; if the user later adds a `title:` field on the
  // operation, it'd never have `$` or underscores, and a derived title
  // shouldn't either. So derivedTitle humanises (strips `$`, swaps `_` for
  // spaces, lowercases all-caps idents).
  //
  // Hierarchy:
  //   - User-supplied `node.title`  → title row
  //   - Else derivedTitle(node)     → title row (and no subtitle, since the
  //                                    derived expression IS the title)
  //   - When user-title exists      → derivedTitle as supporting text below,
  //                                    so the technical detail is still
  //                                    visible underneath the user's label
  const rootTitle = action.title ?? action.output ?? 'operatie';
  const tree = [];

  function traverse(node, prefix, isRoot) {
    const derived = derivedTitle(node);
    const userTitle = node.title;
    const title = isRoot ? rootTitle : (userTitle ?? derived);
    // Show derived as subtitle whenever it adds info beyond the title:
    // - root: always (rootTitle = output name, derived = expression)
    // - non-root: only when user supplied a title that masks the derived form
    const subtitle = isRoot
      ? derived
      : (userTitle ? derived : undefined);

    tree.push({
      number: prefix,
      title,
      subtitle,
      operation: node.operation,
      values: node.values || [],
      node,
    });

    let childIndex = 1;
    for (const child of getChildOperations(node)) {
      traverse(child, `${prefix}.${childIndex}`, false);
      childIndex++;
    }
  }

  traverse(rootNode, '1', true);
  return tree;
}

function getChildOperations(node) {
  const children = [];

  if (Array.isArray(node.values)) {
    for (const v of node.values) {
      if (isOperationNode(v)) children.push(v);
    }
  }

  if (Array.isArray(node.conditions)) {
    for (const c of node.conditions) {
      if (isOperationNode(c)) children.push(c);
    }
  }

  if (isOperationNode(node.when)) children.push(node.when);
  if (isOperationNode(node.value)) children.push(node.value);
  if (isOperationNode(node.then)) children.push(node.then);
  if (isOperationNode(node.else)) children.push(node.else);

  if (Array.isArray(node.cases)) {
    for (const c of node.cases) {
      if (isOperationNode(c.when)) children.push(c.when);
      if (isOperationNode(c.then)) children.push(c.then);
    }
  }
  if (isOperationNode(node.default)) children.push(node.default);

  return children;
}

function isOperationNode(v) {
  return v != null && typeof v === 'object' && v.operation;
}

// Subtitle generation — produces a compact, infix-style description of what
// an operation node does. Examples:
//   EQUALS  → "$x = 0"
//   AND (3) → "$a en $b en $c"
//   AND (5) → "5 condities"
//   ADD (4) → "som van 4 waarden"
//   IF      → "1 conditie + standaard"
// Threshold of 3: ≤3 operands renders as full infix, >3 collapses to a count
// summary so a long arithmetic chain doesn't blow out the row.

const COMPARISON_SYMBOLS = {
  EQUALS: '=',
  NOT_EQUALS: '≠',
  GREATER_THAN: '>',
  LESS_THAN: '<',
  GREATER_THAN_OR_EQUAL: '≥',
  LESS_THAN_OR_EQUAL: '≤',
};
const ARITHMETIC_SYMBOLS = { ADD: '+', SUBTRACT: '−', MULTIPLY: '×', DIVIDE: '÷' };
const ARITHMETIC_NOUNS = { ADD: 'som', SUBTRACT: 'aftrekking', MULTIPLY: 'product', DIVIDE: 'deling' };
const LOGICAL_CONNECTORS = { AND: 'en', OR: 'of' };
const COMPACT_THRESHOLD = 3;

export function describeSubtitle(node) {
  const op = node.operation;

  if (COMPARISON_SYMBOLS[op]) {
    return `${formatArgName(node.subject)} ${COMPARISON_SYMBOLS[op]} ${formatArgName(node.value)}`;
  }

  if (op === 'NOT_NULL') return `${formatArgName(node.subject)} is ingevuld`;

  if (op === 'IN' || op === 'NOT_IN') {
    const verb = op === 'IN' ? 'in' : 'niet in';
    // Two YAML shapes:
    //   subject + value  → list lives elsewhere, value is a ref/literal
    //   subject + values → inline list as array
    if (node.value !== undefined) {
      return `${formatArgName(node.subject)} ${verb} ${formatArgName(node.value)}`;
    }
    const items = Array.isArray(node.values) ? node.values : [];
    if (items.length > COMPACT_THRESHOLD) return `${formatArgName(node.subject)} ${verb} ${items.length} waarden`;
    return `${formatArgName(node.subject)} ${verb} [${items.map(formatArgName).join(', ')}]`;
  }

  if (LOGICAL_CONNECTORS[op]) {
    const conditions = Array.isArray(node.conditions) ? node.conditions : [];
    if (conditions.length > COMPACT_THRESHOLD) return `${conditions.length} condities`;
    const labels = conditions.map(nameableOperand);
    if (labels.some(l => l === null)) return `${conditions.length} condities`;
    return labels.join(` ${LOGICAL_CONNECTORS[op]} `);
  }

  if (op === 'NOT') return `niet ${formatArgName(node.value)}`;

  if (ARITHMETIC_SYMBOLS[op]) {
    const values = Array.isArray(node.values) ? node.values : [];
    if (values.length > COMPACT_THRESHOLD) return `${ARITHMETIC_NOUNS[op]} van ${values.length} waarden`;
    const labels = values.map(nameableOperand);
    if (labels.some(l => l === null)) return `${ARITHMETIC_NOUNS[op]} van ${values.length} waarden`;
    return labels.join(` ${ARITHMETIC_SYMBOLS[op]} `);
  }

  if (op === 'MIN' || op === 'MAX') {
    const fn = op === 'MIN' ? 'min' : 'max';
    const values = Array.isArray(node.values) ? node.values : [];
    if (values.length > COMPACT_THRESHOLD) return `${fn} van ${values.length} waarden`;
    const labels = values.map(nameableOperand);
    if (labels.some(l => l === null)) return `${fn} van ${values.length} waarden`;
    return `${fn}(${labels.join(', ')})`;
  }

  if (op === 'CONCAT') {
    const values = Array.isArray(node.values) ? node.values : [];
    if (values.length > COMPACT_THRESHOLD) return `samenvoegen van ${values.length} waarden`;
    const labels = values.map(nameableOperand);
    if (labels.some(l => l === null)) return `samenvoegen van ${values.length} waarden`;
    return labels.join(' + ');
  }

  // IF/SWITCH have no single subject and multiple branches — any one-line
  // summary leaves out important parts of the operation. The structural
  // count ("1 conditie + standaard") was technically accurate but confusing
  // because it doesn't tell the reader what the operation actually returns.
  // Better to be honest: just label it as the operation type and let the
  // user add a `node.title` when they want a meaningful name.
  if (op === 'IF' || op === 'SWITCH') {
    return `${OPERATION_LABELS[op]} operatie`;
  }

  if (op === 'AGE') {
    return `leeftijd ${formatArgName(node.date_of_birth)} op ${formatArgName(node.reference_date)}`;
  }

  if (op === 'DATE_ADD') return `${formatArgName(node.subject ?? node.value)} + offset`;
  if (op === 'DATE') return formatArgName(node.value ?? node.subject);
  if (op === 'DAY_OF_WEEK') return `dag van ${formatArgName(node.subject ?? node.value)}`;

  if (op === 'LIST') {
    const values = Array.isArray(node.values) ? node.values : [];
    if (values.length > COMPACT_THRESHOLD) return `lijst van ${values.length} items`;
    return `[${values.map(formatArgName).join(', ')}]`;
  }

  return OPERATION_LABELS[op] || op;
}

function formatArgName(v) {
  if (v === null || v === undefined || v === '') return '(leeg)';
  if (typeof v === 'string') return v;
  if (typeof v === 'number') return String(v);
  if (typeof v === 'boolean') return String(v);
  if (isOperationNode(v)) {
    // Prefer the first variable inside the nested op — much more informative
    // than the operation label. So `AND( IN($a, ...), IN($b, ...) )` reads
    // as `$a en $b` instead of `(in lijst) en (in lijst)`. Falls back to
    // `(label)` only when the nested op holds no variable refs at all.
    const varName = findFirstVariable(v);
    if (varName) return varName;
    const label = OPERATION_LABELS[v.operation] || v.operation;
    return `(${label})`;
  }
  return '(leeg)';
}

/**
 * Operand renderer for compositional ops (AND/OR/ADD/SUBTRACT/MIN/MAX/etc.).
 * Returns a clean label when the operand can be cleanly named, or `null`
 * when it's a nameless nested op — `null` signals the caller to fall back
 * to the count-summary form rather than emit half-information like
 * `$a − (als/dan)` where the right side carries multiple operands but only
 * the wrapper type is shown.
 *
 * Nameable means: simple value (variable / literal / empty) OR a nested op
 * with an explicit user-supplied `node.title`. The user can opt complex
 * nested operations into the infix expression by adding a title.
 */
function nameableOperand(v) {
  if (v === null || v === undefined || v === '') return '(leeg)';
  if (typeof v === 'string') return v;
  if (typeof v === 'number') return String(v);
  if (typeof v === 'boolean') return String(v);
  if (isOperationNode(v)) return v.title ?? null;
  return null;
}

/**
 * Human-readable label for an operation, derived from its expression. Strips
 * `$` from variable refs and turns underscores into spaces (and lowercases
 * all-caps idents) so the result reads like prose. Used as the Titel row in
 * OperationSettings, the row text in ActionSheet's parent-ops list, and the
 * (operatie) suffix on dropdown nested options. Falls through to the user-
 * supplied `node.title` (no derivation) when present — that's handled at
 * the buildOperationTree layer.
 *
 * Per-token regex (`$IDENT`) so each variable is humanised independently —
 * a string like `$PERCENTAGE_LID_5 + $aantal_dagen` becomes
 * `percentage lid 5 + aantal dagen` instead of inheriting the all-caps state
 * from one of its tokens.
 */
export function derivedTitle(node) {
  return describeSubtitle(node).replace(
    /\$([A-Za-z_][A-Za-z0-9_]*)/g,
    (_, name) => humanizeIdentifier(name),
  );
}

function humanizeIdentifier(name) {
  const spaced = name.replace(/_/g, ' ');
  return /[A-Z]/.test(spaced) && spaced === spaced.toUpperCase() ? spaced.toLowerCase() : spaced;
}


function getReadableName(v) {
  if (typeof v === 'string') return v;
  if (typeof v === 'number') return String(v);
  if (typeof v === 'boolean') return String(v);
  if (isOperationNode(v)) {
    const varName = findFirstVariable(v);
    if (varName) return varName;
    return OPERATION_LABELS[v.operation] || v.operation;
  }
  return null;
}

function findFirstVariable(node) {
  if (!node || typeof node !== 'object') return null;
  if (typeof node.subject === 'string' && node.subject.startsWith('$')) {
    return node.subject;
  }
  if (Array.isArray(node.values)) {
    for (const v of node.values) {
      if (typeof v === 'string' && v.startsWith('$')) return v;
      if (typeof v === 'object') {
        const found = findFirstVariable(v);
        if (found) return found;
      }
    }
  }
  if (Array.isArray(node.conditions)) {
    for (const c of node.conditions) {
      const found = findFirstVariable(c);
      if (found) return found;
    }
  }
  return null;
}

export function formatValueLabel(v) {
  if (v === null || v === undefined || v === '') return '(leeg)';
  if (typeof v === 'string') return v;
  if (typeof v === 'number') return String(v);
  if (typeof v === 'boolean') return String(v);
  if (isOperationNode(v)) {
    return getReadableName(v) || '(leeg)';
  }
  return '(leeg)';
}
