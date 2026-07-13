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
 * marked applies to corpus law text are raw-only - numbered-list prefixes
 * (`1. `) dropped from the visible text, and whitespace runs collapsed by
 * HTML. Under that assumption every DOM char appears in the raw text in the
 * same order, the scan only ever skips *raw* characters, and a raw span maps
 * to a contiguous DOM range. Inline markdown that *adds* DOM characters
 * (`**bold**`, `[text](url)`, headings) would break this - corpus articles
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
 *   desynced: boolean,
 * }}
 *   rawToDom[i] is the DOM position of raw char i, or null if that raw char
 *   has no DOM counterpart (a list prefix or a collapsed-away space).
 *   desynced is true when the rendered text diverged from raw in a way this
 *   scan cannot model; the caller should then skip highlighting.
 */
export function buildAlignment(rawText, domNodes) {
  const raw = Array.from(rawText); // code points: resolver offsets are char-based
  // Flatten DOM text into a single code-point stream, remembering which node
  // and in-node code-point offset each char came from. Whitespace-only text
  // nodes are skipped entirely: marked emits newlines between tags
  // ("<ol>\n<li>") that become text nodes with no raw counterpart. If a raw
  // char (the "\n\n" paragraph break) were allowed to map onto such a node,
  // a span crossing two list items would produce a slice for it and
  // AnnotatedText would wrap a focusable, styled <mark> around a bare "\n"
  // sitting between <li>s - invalid HTML and an empty highlight blob. The
  // raw paragraph break instead aligns via the normal raw-advance path.
  const dom = [];
  for (const { node, text } of domNodes) {
    if (!text || /^\s*$/.test(text)) continue;
    const cps = Array.from(text);
    for (let k = 0; k < cps.length; k++) {
      dom.push({ ch: cps[k], node, offset: k });
    }
  }

  const rawToDom = new Array(raw.length).fill(null);
  let r = 0;
  let d = 0;
  // Track the longest contiguous run of *significant* (non-whitespace) DOM
  // chars that the scan had to skip. A real divergence this two-pointer scan
  // cannot model - inline markdown that added visible DOM text - shows up as
  // exactly that: a contiguous block of DOM chars with no raw counterpart,
  // after which every offset is shifted (the "smear"). The legitimate
  // raw-only transforms (list prefixes, collapsed whitespace) never leave
  // significant DOM chars unmatched at all, so the tolerable run length is
  // small. A global ratio was the wrong shape: a few wrong chars in a long
  // article stay under any percentage while still smearing every later mark.
  let maxSkippedRun = 0;
  let curSkippedRun = 0;
  while (r < raw.length && d < dom.length) {
    if (raw[r] === dom[d].ch) {
      rawToDom[r] = { node: dom[d].node, offset: dom[d].offset };
      r++;
      d++;
      curSkippedRun = 0;
      continue;
    }
    // Mismatch. Expected, raw-only causes: a numbered-list prefix ("1. "
    // marked turned into <li>) and whitespace the HTML renderer collapsed
    // (raw "x  y" -> dom "x y", or a soft "\n" dropped). These are raw-only
    // and short, so advancing the raw pointer realigns at the next shared
    // character. A DOM char genuinely absent from raw is the divergence
    // case: skip it on the DOM side and count the run.
    if (WS.test(raw[r])) {
      r++;
    } else if (WS.test(dom[d].ch)) {
      d++; // DOM-side whitespace (inter-tag newline already filtered; rare)
    } else if (raw.indexOf(dom[d].ch, r) === -1) {
      // This DOM char does not occur anywhere in the remaining raw text:
      // it is genuinely DOM-only. Skip it and grow the divergence run.
      d++;
      curSkippedRun++;
      if (curSkippedRun > maxSkippedRun) maxSkippedRun = curSkippedRun;
    } else {
      // The DOM char reappears later in raw: the chars between are raw-only
      // (a list prefix). Advance raw to catch up.
      r++;
    }
  }
  // Any DOM text the scan never reached (d stopped short) is also unmatched
  // significant content if non-whitespace.
  let tailRun = 0;
  for (let i = d; i < dom.length; i++) {
    if (WS.test(dom[i].ch)) continue;
    tailRun++;
  }
  if (tailRun > maxSkippedRun) maxSkippedRun = tailRun;
  // Tolerate a tiny run: a stray entity or punctuation artefact should not
  // suppress an otherwise-correct article. Anything longer means inline
  // markup the scan cannot model; drop highlighting rather than smear.
  const desynced = maxSkippedRun > 4;
  return { rawToDom, desynced };
}

// Is at least one raw char in [start, end) mapped to the DOM? A span that
// falls entirely inside skipped prefix/whitespace has no DOM position to
// anchor to. The actual slice geometry is derived from the rawToDom walk in
// spanToNodeSlices; this is only the "anchorable at all" gate.
function hasMappedChar(rawToDom, start, end) {
  for (let i = start; i < end; i++) {
    if (rawToDom[i]) return true;
  }
  return false;
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
  if (!hasMappedChar(rawToDom, span.start, span.end)) return [];

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
