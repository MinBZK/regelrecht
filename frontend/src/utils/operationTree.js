export const OPERATION_LABELS = {
  ADD: 'optellen',
  SUBTRACT: 'aftrekken',
  MULTIPLY: 'vermenigvuldig',
  MIN: 'min',
  MAX: 'max',
  EQUALS: 'gelijk aan',
  GREATER_THAN: 'groter dan',
  GREATER_THAN_OR_EQUAL: 'groter dan of gelijk',
  LESS_THAN_OR_EQUAL: 'kleiner dan of gelijk',
  IF: 'voorwaarde',
  AND: 'en',
};

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

  // Don't traverse into `when` (condition spec, not structural child)
  // Don't traverse into `value` (comparison value, handled at root level only)
  if (isOperationNode(node.then)) children.push(node.then);
  if (isOperationNode(node.else)) children.push(node.else);

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
