const OPERATION_SYMBOLS = {
  EQUALS: '=',
  NOT_EQUALS: '≠',
  GREATER_THAN: '>',
  GREATER_THAN_OR_EQUAL: '>=',
  LESS_THAN: '<',
  LESS_THAN_OR_EQUAL: '<=',
  IN: 'in',
  NOT_IN: 'niet in',
  NOT_NULL: '≠ null',
};

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

  const rootTitle = action.output ? action.output.replace(/_/g, ' ') : 'operatie';
  const tree = [];

  function traverse(node, prefix, isRoot) {
    tree.push({
      number: prefix,
      title: isRoot ? rootTitle : humanizeTitle(node),
      subtitle: describeSubtitle(node),
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

export function describeSubtitle(node) {
  const label = OPERATION_LABELS[node.operation] || node.operation;
  const args = [];

  if (node.subject != null) args.push(formatArgName(node.subject));
  if (node.value !== undefined && !isOperationNode(node.value)) {
    args.push(formatArgName(node.value));
  } else if (isOperationNode(node.value)) {
    args.push('(...)');
  }

  if (Array.isArray(node.values)) {
    for (const v of node.values) {
      args.push(formatArgName(v));
    }
  }

  if (Array.isArray(node.conditions)) {
    for (const c of node.conditions) {
      args.push(formatArgName(c));
    }
  }

  if (node.operation === 'IF' && Array.isArray(node.cases)) {
    args.push(`${node.cases.length} gevallen`);
    if (node.default !== undefined) args.push(`standaard ${formatArgName(node.default)}`);
  }

  if (args.length === 0) return label;
  return `${label}: ${args.join(', ')}`;
}

function formatArgName(v) {
  if (typeof v === 'string') return v.startsWith('$') ? v.slice(1) : v;
  if (typeof v === 'number') return String(v);
  if (typeof v === 'boolean') return String(v);
  if (isOperationNode(v)) return '(...)';
  return '...';
}

export function humanizeTitle(node) {
  const symbol = OPERATION_SYMBOLS[node.operation];
  if (symbol && node.subject != null) {
    const subject = getReadableName(node.subject) || formatArgName(node.subject);
    if (node.value !== undefined && !isOperationNode(node.value)) {
      const value = getReadableName(node.value) ?? formatArgName(node.value);
      return `${subject} ${symbol} ${value}`;
    }
    return `${subject} ${symbol}`;
  }

  const names = [];

  if (Array.isArray(node.values)) {
    for (const v of node.values) {
      const name = getReadableName(v);
      if (name) names.push(name);
    }
  }

  if (node.subject != null) {
    const name = getReadableName(node.subject);
    if (name) names.push(name);
  }

  if (Array.isArray(node.conditions)) {
    for (const c of node.conditions) {
      const name = getReadableName(c);
      if (name) names.push(name);
    }
  }

  if (names.length > 0) return names.join(' en ');
  return OPERATION_LABELS[node.operation] || node.operation;
}

function getReadableName(v) {
  if (typeof v === 'string') return v.startsWith('$') ? v.slice(1).replace(/_/g, ' ') : v;
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
    return node.subject.slice(1).replace(/_/g, ' ');
  }
  if (Array.isArray(node.values)) {
    for (const v of node.values) {
      if (typeof v === 'string' && v.startsWith('$')) return v.slice(1).replace(/_/g, ' ');
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
  if (typeof v === 'string') return v.startsWith('$') ? v.slice(1).replace(/_/g, ' ') : v;
  if (typeof v === 'number') return String(v);
  if (typeof v === 'boolean') return String(v);
  if (isOperationNode(v)) {
    const name = getReadableName(v);
    return name || '(...)';
  }
  return '...';
}
