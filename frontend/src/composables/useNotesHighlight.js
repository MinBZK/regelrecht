/**
 * Project resolver char-offsets onto rendered markdown.
 *
 * The WASM resolver matches against the *raw* article text, so a note's span
 * is a `[start, end)` range of char (code-point) offsets into that exact
 * string. ArticleText renders that text through marked, which drops the
 * numbered-list prefixes (`1. `, `2. `) from the visible text and turns
 * `\n\n` into list/paragraph structure. So the rendered DOM's visible text is
 * the raw text minus those prefixes (and with whitespace runs collapsed by
 * HTML). markRanges() (in useNotes.js) sidesteps this by rendering plain text;
 * this module instead keeps the markdown rendering and re-aligns.
 *
 * Approach: walk the rendered DOM's text nodes in document order, concatenate
 * their text, and align that against the raw text with a two-pointer scan.
 *
 * Assumption (not an invariant the code can enforce): the only transforms
 * marked applies to corpus law text are raw-only — numbered-list prefixes
 * (`1. `) dropped from the visible text, and whitespace runs collapsed by
 * HTML. Under that assumption every DOM char appears in the raw text in the
 * same order, the scan only ever skips *raw* characters, and a raw span maps
 * to a contiguous DOM range. Inline markdown that *adds* DOM characters
 * (`**bold**`, `[text](url)`, headings) would break this — corpus articles
 * are not expected to contain it, but if a DOM char cannot be matched the
 * scan reports `desynced` so the caller can drop highlighting for that
 * article rather than smear every offset onto the wrong words.
 */

const WS = /\s/;

/**
 * Build a map from raw char-offset -> DOM text position.
 *
 * @param {string} rawText      the exact string the resolver matched against
 * @param {Array<{node: Node, text: string}>} domNodes
 *        rendered text nodes in document order, with their textContent
 * @returns {{
 *   rawToDom: Array<{node: Node, offset: number} | null>,
 *   domLen: number,
 * }}
 *   rawToDom[i] is the DOM position of raw char i, or null if that raw char
 *   has no DOM counterpart (a list prefix or a collapsed-away space).
 */
export function buildAlignment(rawText, domNodes) {
  const raw = Array.from(rawText); // code points: resolver offsets are char-based
  // Flatten DOM text into a single code-point stream, remembering which node
  // and in-node code-point offset each char came from.
  const dom = [];
  for (const { node, text } of domNodes) {
    const cps = Array.from(text);
    for (let k = 0; k < cps.length; k++) {
      dom.push({ ch: cps[k], node, offset: k });
    }
  }

  const rawToDom = new Array(raw.length).fill(null);
  let r = 0;
  let d = 0;
  while (r < raw.length && d < dom.length) {
    if (raw[r] === dom[d].ch) {
      rawToDom[r] = { node: dom[d].node, offset: dom[d].offset };
      r++;
      d++;
      continue;
    }
    // Mismatch. The expected, raw-only causes are a numbered-list prefix
    // ("1. " marked turned into <li>) and whitespace the HTML renderer
    // collapsed (raw "x  y" -> dom "x y", or a soft "\n" dropped). All of
    // these are raw-only and short, so advancing the raw pointer realigns
    // at the next shared character. A DOM char that is genuinely absent from
    // raw (inline markdown that *added* DOM text) would instead pin `d` and
    // leave a large unconsumed DOM tail — the caller checks coverage for
    // that, see `desynced`.
    if (WS.test(dom[d].ch) && !WS.test(raw[r])) {
      // Injected layout whitespace on the DOM side (rare): skip it.
      d++;
    } else {
      r++;
    }
  }
  // A trustworthy alignment consumes essentially all of the DOM's *visible*
  // text. Whitespace-only DOM chars are excluded from the denominator: marked
  // emits newlines between tags ("<ol>\n<li>") that become DOM text nodes
  // with no raw counterpart, which is expected and harmless. A genuine
  // divergence this scan cannot model (inline markdown that added *visible*
  // DOM text) leaves non-whitespace DOM chars unmatched; report that so the
  // caller drops highlighting rather than smearing every offset.
  let domSignificant = 0;
  let mappedSignificant = 0;
  for (const { ch } of dom) {
    if (WS.test(ch)) continue;
    domSignificant++;
  }
  for (let i = 0; i < raw.length; i++) {
    if (rawToDom[i] && !WS.test(raw[i])) mappedSignificant++;
  }
  const desynced =
    domSignificant > 0 && mappedSignificant < domSignificant * 0.98;
  return { rawToDom, domLen: dom.length, desynced };
}

// Anchorability probes only: spanToNodeSlices derives the actual slice
// geometry from the rawToDom walk and uses these two solely to decide
// whether the span has *any* mapped char near its start and end (a span
// that falls entirely inside skipped prefix/whitespace is unanchorable).
// anchorStart -> first mapped char at/after the offset; anchorEnd -> the
// position just past the last mapped char before the exclusive end.
function anchorStart(rawToDom, rawOffset) {
  for (let i = rawOffset; i < rawToDom.length; i++) {
    if (rawToDom[i]) return rawToDom[i];
  }
  return null;
}
function anchorEnd(rawToDom, rawEndExclusive) {
  for (let i = rawEndExclusive - 1; i >= 0; i--) {
    if (rawToDom[i]) {
      return { node: rawToDom[i].node, offset: rawToDom[i].offset + 1 };
    }
  }
  return null;
}

/**
 * Turn one resolved span into a list of per-text-node DOM Range descriptors.
 * A span can cross text-node boundaries (e.g. a highlight that spans two
 * paragraphs / list items), so it is sliced per node; each slice becomes its
 * own <mark> wrapper. Returns [] if the span cannot be anchored.
 *
 * `domNodeCpLen` maps a text node to its code-point length so we can clamp a
 * slice to the node end without re-walking. Range offsets are UTF-16 indices,
 * so the caller converts code-point offsets via cpToUtf16().
 *
 * @returns {Array<{node: Node, startCp: number, endCp: number}>}
 */
export function spanToNodeSlices(rawToDom, span, domNodeCpLen) {
  const a = anchorStart(rawToDom, span.start);
  const b = anchorEnd(rawToDom, span.end);
  if (!a || !b) return [];

  // Collect the ordered set of text nodes the span touches by scanning the
  // mapped raw range. Each mapped raw char names a (node, offset); group
  // consecutive chars by node and track min/max in-node offset.
  const slices = [];
  let cur = null;
  for (let i = span.start; i < span.end; i++) {
    const pos = rawToDom[i];
    if (!pos) continue;
    if (!cur || cur.node !== pos.node) {
      if (cur) slices.push(cur);
      cur = { node: pos.node, startCp: pos.offset, endCp: pos.offset + 1 };
    } else {
      cur.endCp = pos.offset + 1;
    }
  }
  if (cur) slices.push(cur);

  // Clamp defensively to the node length.
  for (const s of slices) {
    const len = domNodeCpLen.get(s.node);
    if (len != null && s.endCp > len) s.endCp = len;
  }
  return slices.filter((s) => s.endCp > s.startCp);
}

/** Code-point offset -> UTF-16 offset within `text` (DOM Range uses UTF-16). */
export function cpToUtf16(text, cpOffset) {
  let cp = 0;
  let u = 0;
  for (const ch of text) {
    if (cp >= cpOffset) break;
    u += ch.length;
    cp++;
  }
  return u;
}
