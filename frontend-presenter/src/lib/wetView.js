/**
 * Which parts of an article a ```wet block shows. The anchors are names that
 * already stand in the YAML (an output, an input, a definition), not paths
 * like `actions[2]`: a path breaks as soon as someone adds an action above it,
 * a name keeps pointing at the same thing.
 *
 *   uitvoer: [hoogte_zorgtoeslag]     only these outputs, and their rules
 *   invoer: [toetsingsinkomen]        only these inputs (and parameters)
 *   definities: [percentage_...]      only these fixed values
 *   markeer: [standaardpremie]        light these names up wherever they appear
 *   leden: [1, 2]                     only these leden of the text
 *   show: [tekst, regels, ...]        which sections; see `sections` below
 *
 * A name that the article does not have ends up in `unknown`, so a typo shows
 * on the slide while it is being made instead of silently showing nothing.
 */

export const SECTIONS = ['tekst', 'definities', 'invoer', 'uitvoer', 'regels'];

const list = (v) => (v == null ? null : [].concat(v).map(String));

/**
 * The sections to show. An explicit `show` wins. Without it, an anchor brings
 * its own section (an output anchor shows its rules, which is what one points
 * at an output for), `leden` brings the text, and a block with neither shows
 * the text.
 */
export function sections(spec) {
  if (spec.show != null) return new Set(list(spec.show));
  const out = new Set();
  if (spec.leden != null) out.add('tekst');
  if (spec.uitvoer != null) out.add('regels');
  if (spec.invoer != null) out.add('invoer');
  if (spec.definities != null) out.add('definities');
  if (!out.size) out.add('tekst');
  return out;
}

/** `$name` operands directly on an operation node (not in its children). */
export function directRefs(node) {
  const refs = new Set();
  const take = (v) => {
    if (typeof v === 'string' && v.startsWith('$')) refs.add(v.slice(1));
  };
  for (const key of ['subject', 'value', 'when', 'then', 'else', 'from', 'to', 'date', 'collection', 'default']) take(node?.[key]);
  for (const v of node?.values ?? []) take(v);
  for (const v of node?.conditions ?? []) take(v);
  for (const c of node?.cases ?? []) {
    take(c?.when);
    take(c?.then);
  }
  return refs;
}

/**
 * The article filtered by the block's anchors. Everything is returned with its
 * raw YAML name, so the component can humanise for display and still match on
 * the name the author wrote.
 */
export function selectArticle(article, spec) {
  const mr = article?.machine_readable ?? {};
  const exec = mr.execution ?? {};
  const unknown = [];

  const pick = (items, names, kind) => {
    if (!names) return items;
    const have = new Set(items.map((i) => i.name));
    for (const n of names) if (!have.has(n)) unknown.push({ kind, name: n });
    return items.filter((i) => names.includes(i.name));
  };

  const definitions = pick(
    Object.entries(mr.definitions ?? {}).map(([name, def]) => ({ name, def })),
    list(spec.definities),
    'definitie',
  );
  const inputs = pick(
    [...(exec.parameters ?? []).map((p) => ({ ...p, isParameter: true })), ...(exec.input ?? [])],
    list(spec.invoer),
    'invoer',
  );
  const outputs = pick(exec.output ?? [], list(spec.uitvoer), 'uitvoer');
  const outNames = list(spec.uitvoer);
  const actions = (exec.actions ?? []).filter((a) => !outNames || outNames.includes(a.output));

  const highlight = new Set(list(spec.markeer) ?? []);
  if (highlight.size) {
    const known = new Set([
      ...Object.keys(mr.definitions ?? {}),
      ...(exec.parameters ?? []).map((p) => p.name),
      ...(exec.input ?? []).map((i) => i.name),
      ...(exec.output ?? []).map((o) => o.name),
    ]);
    for (const n of highlight) if (!known.has(n)) unknown.push({ kind: 'markeer', name: n });
  }

  return { show: sections(spec), definitions, inputs, outputs, actions, highlight, unknown };
}
