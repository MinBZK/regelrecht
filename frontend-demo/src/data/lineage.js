/**
 * Turn an execution trace into the "Gebruikte gegevens" tree: which values the
 * law used, where each came from (an organisation's register, or another law
 * that computed it), and which law owns the input so a correction can be
 * registered against the right law.
 *
 * Trace shape (engine `executeMultipleWithTrace`):
 *   { node_type: 'article'|'cross_law_reference'|'resolve'|'action'|'operation',
 *     name, result, resolve_type?, message?, children[] }
 * A `cross_law_reference` is named `<law>#<output>`; a `resolve` with
 * resolve_type DATA_SOURCE carries "Resolving from SOURCE <name>: <value>" in
 * its message, where <name> is the source we registered (the organisation, or
 * `correcties` for an approved claim).
 */
import { CLAIMS_SOURCE } from '../engine/useDemoEngine.js';

const SOURCE_RE = /^Resolving from SOURCE ([^:]+):/;

/**
 * @typedef {object} LineageNode
 * @property {'value'|'law'} kind
 * @property {string} law        owning law id
 * @property {string} name       input name (value) or output name (law)
 * @property {*} value
 * @property {string} [service]  organisation for a value from a register
 * @property {boolean} [corrected]  value came from an approved claim
 * @property {string} [keyField] parameter the law was keyed on (bsn/kvk_nummer)
 * @property {string} [keyValue]
 * @property {LineageNode[]} [children]
 */

function keyOf(children) {
  for (const key of ['bsn', 'kvk_nummer', 'organisatie_id', 'project_id']) {
    const hit = (children ?? []).find((c) => c.node_type === 'resolve' && c.name === key && c.resolve_type === 'PARAMETER');
    if (hit) return { keyField: key, keyValue: hit.result };
  }
  return {};
}

/**
 * @param {object} trace       root trace node
 * @param {string} rootLawId
 * @param {object} rootParams  parameters of the top-level call
 * @returns {LineageNode[]}
 */
export function lineageFromTrace(trace, rootLawId, rootParams) {
  const rootKey = rootParams.bsn ? { keyField: 'bsn', keyValue: rootParams.bsn } : rootParams.kvk_nummer ? { keyField: 'kvk_nummer', keyValue: rootParams.kvk_nummer } : {};
  return collect(trace, rootLawId, rootKey, new Set());
}

function collect(node, lawId, key, seen) {
  const out = [];
  for (const child of node?.children ?? []) {
    if (child.node_type === 'resolve' && child.resolve_type === 'DATA_SOURCE') {
      const dedupe = `${lawId}|${child.name}`;
      if (seen.has(dedupe)) continue;
      seen.add(dedupe);
      const source = SOURCE_RE.exec(child.message ?? '')?.[1] ?? null;
      out.push({
        kind: 'value',
        law: lawId,
        name: child.name,
        value: child.result ?? null,
        service: source === CLAIMS_SOURCE ? null : source,
        corrected: source === CLAIMS_SOURCE,
        ...key,
      });
    } else if (child.node_type === 'cross_law_reference') {
      const [refLaw, output] = String(child.name).split('#');
      const dedupe = `ref|${refLaw}|${output}`;
      if (seen.has(dedupe)) continue;
      seen.add(dedupe);
      const childKey = { ...key, ...keyOf(child.children) };
      out.push({
        kind: 'law',
        law: refLaw,
        name: output,
        value: child.result ?? null,
        children: collect(child, refLaw, childKey, seen),
        ...childKey,
      });
    } else if (child.node_type === 'article' || child.node_type === 'action' || child.node_type === 'operation') {
      // Descend: nested resolutions inside an action tree still count.
      out.push(...collect(child, lawId, key, seen));
    }
  }
  return out;
}

/** Flatten to the register values only (for counting and for a summary). */
export function leafValues(nodes) {
  const out = [];
  for (const n of nodes) {
    if (n.kind === 'value') out.push(n);
    else out.push(...leafValues(n.children ?? []));
  }
  return out;
}
