/**
 * useTextSelection — turn a DOM selection in the rendered article into a
 * TextQuoteSelector against the *raw* law text (RFC-018 write path, step 2).
 *
 * The Tekst pane renders the raw article text through marked (list prefixes
 * stripped, whitespace collapsed — see useNotesHighlight.js). A user selecting
 * "zorgtoeslag" with the mouse selects it in that rendered DOM, but a note's
 * TextQuoteSelector must quote the *raw* text the resolver matches against, or
 * it will not round-trip. So a selection is mapped DOM -> raw using the same
 * two-pointer alignment the highlight path uses in reverse, then `exact`,
 * `prefix` and `suffix` are sliced from the raw string.
 *
 * Uniqueness: a bare `exact` often occurs many times ("verzekerde" appears
 * dozens of times in the Zorgtoeslagwet). The selector is only useful if it
 * resolves to exactly one span, so context is grown until the resolver
 * (`engine.resolveNote`) reports a single match, or we run out of article and
 * report it as still ambiguous so the UI can ask the user to select more.
 */
import { buildAlignment } from './useNotesHighlight.js';

// W3C TextQuoteSelector context length. RFC-018 §write-path says 30-50 chars;
// 40 is the midpoint and enough to disambiguate every repeated phrase in the
// current corpus while staying readable in the note YAML.
const CONTEXT = 40;

/**
 * Walk `root`'s text nodes (document order) and return them with text, the
 * shape buildAlignment expects.
 */
function collectTextNodes(root) {
  const walker = document.createTreeWalker(root, NodeFilter.SHOW_TEXT);
  const out = [];
  let n;
  while ((n = walker.nextNode())) {
    out.push({ node: n, text: n.textContent });
  }
  return out;
}

/** UTF-16 offset within `text` -> code-point offset (inverse of cpToUtf16). */
export function utf16ToCp(text, u16Offset) {
  let cp = 0;
  let u = 0;
  for (const ch of text) {
    if (u >= u16Offset) break;
    u += ch.length;
    cp++;
  }
  return cp;
}

/**
 * Build a DOM-position -> raw-char-offset map by inverting the raw->DOM
 * alignment. `buildAlignment` already solves the hard direction (it knows
 * which raw chars marked dropped); inverting it is exact for every DOM char
 * that has a raw counterpart, which is all of them under the resolver's
 * raw-only-transforms assumption.
 *
 * @returns {{ domToRaw: Map<Node, Array<number>>, desynced: boolean }}
 *   domToRaw.get(node)[cpOffset] === raw char offset, or undefined if that DOM
 *   char has no raw counterpart (should not happen for corpus text).
 */
function buildDomToRaw(rawText, domNodes) {
  const { rawToDom, desynced } = buildAlignment(rawText, domNodes);
  const domToRaw = new Map();
  for (let r = 0; r < rawToDom.length; r++) {
    const pos = rawToDom[r];
    if (!pos) continue;
    let arr = domToRaw.get(pos.node);
    if (!arr) {
      arr = [];
      domToRaw.set(pos.node, arr);
    }
    // First raw char wins a DOM slot: collapsed whitespace maps several raw
    // chars to one DOM char, and the selection should anchor at the start of
    // the run, not the end.
    if (arr[pos.offset] === undefined) arr[pos.offset] = r;
  }
  return { domToRaw, desynced };
}

/**
 * First *mappable* text node (present in domToRaw) inside `node`'s subtree,
 * scanning forward (fromEnd=false) or backward (fromEnd=true) in document
 * order. Whitespace-only nodes (marked's inter-tag "\n" in `<ol>\n<li>`) are
 * skipped because buildAlignment skips them, so they have no domToRaw entry;
 * an endpoint landing on one must traverse past it, not dead-end.
 */
function firstMappableTextNode(node, fromEnd, domToRaw) {
  if (!node) return null;
  if (node.nodeType === Node.TEXT_NODE) {
    return domToRaw.has(node) ? node : null;
  }
  const kids = node.childNodes;
  const order = fromEnd ? [...kids.keys()].reverse() : [...kids.keys()];
  for (const i of order) {
    const tn = firstMappableTextNode(kids[i], fromEnd, domToRaw);
    if (tn) return tn;
  }
  return null;
}

/**
 * Mappable text node reached by scanning `parent`'s children from `fromIdx`
 * in document order (forward for a start endpoint, backward for an end one).
 * Unlike firstMappableTextNode this does not dead-end when child `fromIdx` is
 * itself a whitespace-only text node (marked's inter-tag "\n"): it keeps
 * walking siblings. This is the `(ol, 0)`-style element endpoint case.
 */
function mappableFromChild(parent, fromIdx, fromEnd, domToRaw) {
  const kids = parent.childNodes;
  if (kids.length === 0) return null;
  const start = Math.max(0, Math.min(fromIdx, kids.length - 1));
  if (fromEnd) {
    for (let i = start; i >= 0; i--) {
      const tn = firstMappableTextNode(kids[i], true, domToRaw);
      if (tn) return tn;
    }
  } else {
    for (let i = start; i < kids.length; i++) {
      const tn = firstMappableTextNode(kids[i], false, domToRaw);
      if (tn) return tn;
    }
  }
  return null;
}

/**
 * Resolve a selection endpoint (container + UTF-16 offset) to a raw char
 * offset. Returns the offset, or null if nothing maps.
 *
 * Two boundary cases the naive "look in this node only" approach gets wrong,
 * both common with marked's `<ol><li>` structure:
 *  - the end caret sits at offset 0 of a node (drag ended exactly at a
 *    paragraph/list-item start): there is no char before it *in this node*, so
 *    fall through to the previous mappable node's end;
 *  - the endpoint is on an element node (e.g. `(ol, 0)`): descend to the
 *    first/last mappable text node, skipping inter-tag whitespace.
 * `nodeOrder` is the document-order list of mappable text nodes so the scan
 * can cross node boundaries.
 */
function endpointToRaw(domToRaw, nodeOrder, container, u16, isEnd) {
  let node = container;
  let offset = u16;
  if (node.nodeType !== Node.TEXT_NODE) {
    // Element endpoint, e.g. `(ol, 0)`. The DOM child at the boundary index
    // is often marked's inter-tag "\n" (not mappable), so scan siblings from
    // there in the endpoint's direction rather than dead-ending on it. For
    // the end the char is *before* the boundary (offset-1).
    const fromIdx = isEnd ? offset - 1 : offset;
    const tn = mappableFromChild(node, fromIdx, isEnd, domToRaw);
    if (!tn) return null;
    node = tn;
    offset = isEnd ? tn.textContent.length : 0;
  }
  let arr;
  const cp = utf16ToCp(node.textContent, offset);

  const startIdx = nodeOrder.indexOf(node);
  if (startIdx < 0) return null;

  if (isEnd) {
    // Caret is after the last selected char: map char (cp-1), return +1.
    // If this node has no mapped char before the caret (cp === 0, drag ended
    // at a node edge), walk back into the previous mappable nodes.
    for (let ni = startIdx; ni >= 0; ni--) {
      arr = domToRaw.get(nodeOrder[ni]);
      const from = ni === startIdx ? cp - 1 : arr.length - 1;
      for (let i = from; i >= 0; i--) {
        if (arr[i] !== undefined) return arr[i] + 1;
      }
    }
    return null;
  }

  // Start: first mapped char at or after the caret; if none in this node,
  // continue into the following mappable nodes.
  for (let ni = startIdx; ni < nodeOrder.length; ni++) {
    arr = domToRaw.get(nodeOrder[ni]);
    const from = ni === startIdx ? cp : 0;
    for (let i = from; i < arr.length; i++) {
      if (arr[i] !== undefined) return arr[i];
    }
  }
  return null;
}

/**
 * Turn the current window selection (which must lie inside `rootEl`) into a
 * raw `[start, end)` char range, or null if there is no usable selection.
 *
 * @param {string} rawText  the exact article text the resolver matches against
 * @param {Element} rootEl  the element the markdown was rendered into
 */
export function selectionToRawRange(rawText, rootEl) {
  const sel = window.getSelection?.();
  if (!sel || sel.rangeCount === 0 || sel.isCollapsed) return null;
  const range = sel.getRangeAt(0);
  if (!rootEl.contains(range.commonAncestorContainer)) return null;

  const domNodes = collectTextNodes(rootEl);
  if (domNodes.length === 0) return null;
  const { domToRaw, desynced } = buildDomToRaw(rawText, domNodes);
  // If the render diverged from raw beyond the known raw-only transforms the
  // map is untrustworthy; refuse rather than anchor a note at a wrong offset.
  if (desynced) return null;

  // Document-order list of mappable text nodes, so endpointToRaw can cross
  // node boundaries when a caret sits at a node edge.
  const nodeOrder = domNodes
    .map(({ node }) => node)
    .filter((n) => domToRaw.has(n));

  const start = endpointToRaw(
    domToRaw,
    nodeOrder,
    range.startContainer,
    range.startOffset,
    false,
  );
  const end = endpointToRaw(
    domToRaw,
    nodeOrder,
    range.endContainer,
    range.endOffset,
    true,
  );
  if (start == null || end == null || end <= start) return null;
  return { start, end };
}

/**
 * Build a TextQuoteSelector from a raw range, growing prefix/suffix context
 * until the resolver finds exactly the span the user selected.
 *
 * "Found" from the resolver is necessary but not sufficient: the resolver's
 * exact-match prefix/suffix check trims a one-char-slack window
 * (resolver.rs verify_at_position), so when the same `exact` repeats and a
 * grown context happens to satisfy the trimmed comparison at the *other*
 * occurrence, the resolver returns `found` pointing at the wrong span. For a
 * legal-text product that silent mis-anchor is unacceptable, so a `found`
 * result is only accepted when the single match lands in the article the user
 * selected in, at exactly the offsets they selected. Otherwise context keeps
 * growing; if it never converges the status is reported as ambiguous so the
 * UI asks the author to extend the selection rather than persisting a note on
 * the wrong sentence.
 *
 * @param {string} rawText           the selected article's text
 * @param {{start:number,end:number}} range  raw char offsets into rawText
 * @param {string} lawId
 * @param {{ resolveNote: Function }} engine  loaded WASM engine
 * @param {string|number} articleNumber  the article `range` is in; the
 *        resolver returns article-relative offsets + article_number, so the
 *        match is verified against this
 * @returns {{
 *   selector: object,
 *   exact: string,
 *   status: 'found'|'ambiguous'|'orphaned',
 *   reason: 'ok'|'too-common'|'not-found'|'mis-anchor',
 * }}
 *   `reason` is for the UI message only; it does not change accept/reject.
 *   'too-common' = the bare quote already matched several places (RFC-018
 *   §5.4 "common word without context") even if widening context later
 *   degrades to orphaned; 'not-found' = genuinely not locatable; 'mis-anchor'
 *   = a unique match landed off the selection.
 */
export function buildSelector(rawText, range, lawId, engine, articleNumber) {
  const chars = Array.from(rawText);
  const exact = chars.slice(range.start, range.end).join('');
  const wantArticle = String(articleNumber);

  // A whitespace-only quote is meaningless as an anchor: it satisfies the
  // schema's minLength:1 and the resolver's non-empty guard, but a note
  // quoting " " points at nothing a reader can see. selectionToRawRange
  // already rejects an empty/zero-length range; this rejects the
  // collapsed-whitespace-only case (a selection that maps to just the space
  // between two words). Reported as orphaned so the UI asks for real text.
  if (exact.trim() === '') {
    return {
      selector: { type: 'TextQuoteSelector', exact },
      exact,
      status: 'orphaned',
      reason: 'not-found',
    };
  }

  // The resolver returns article-relative `char` offsets. range is into this
  // article's text, so a correct unique match has exactly these offsets in
  // this article.
  const isExactlyOurSelection = (result) => {
    if (result?.status !== 'found') return false;
    const matches = result.matches ?? [];
    if (matches.length !== 1) return false;
    const m = matches[0];
    return (
      String(m.article_number) === wantArticle &&
      m.start === range.start &&
      m.end === range.end
    );
  };

  // Try increasing context: no context, then CONTEXT chars each side, then
  // double. Short-circuit as soon as the match is provably our selection.
  const widths = [0, CONTEXT, CONTEXT * 2];
  let last = null;
  // The bare quote (no context) matching several places is the RFC-018 §5.4
  // "common word" case. Remember it: even if widening context later degrades
  // to orphaned (markdown-stripped prefix/suffix no longer matches), the
  // user-facing reason is still "too common", not "not found".
  let sawMultiple = false;
  for (const w of widths) {
    const prefix =
      w > 0 ? chars.slice(Math.max(0, range.start - w), range.start).join('') : '';
    const suffix =
      w > 0 ? chars.slice(range.end, range.end + w).join('') : '';
    const selector = { type: 'TextQuoteSelector', exact };
    if (prefix) selector.prefix = prefix;
    if (suffix) selector.suffix = suffix;

    let result;
    try {
      result = engine.resolveNote(lawId, selector);
    } catch {
      // Resolver threw (law not loaded etc.) — caller surfaces it.
      return { selector, exact, status: 'orphaned', reason: 'not-found' };
    }
    if (isExactlyOurSelection(result)) {
      return { selector, exact, status: 'found', reason: 'ok' };
    }
    if ((result?.matches?.length ?? 0) > 1) sawMultiple = true;
    // A unique `found` that is NOT our selection is a mis-anchor: treat it as
    // ambiguous (more context may still pin the right one) rather than
    // accepting it. Only a genuine orphaned short-circuits.
    const orphaned = result?.status === 'orphaned';
    const status = orphaned ? 'orphaned' : 'ambiguous';
    const reason = sawMultiple
      ? 'too-common'
      : orphaned
        ? 'not-found'
        : result?.status === 'found'
          ? 'mis-anchor'
          : 'too-common';
    last = { selector, exact, status, reason };
    if (orphaned) return last;
  }
  // Widest context still did not provably pin our selection.
  return last;
}
