import { describe, it, expect } from 'vitest';
import { selectionToRawRange, buildSelector } from './useTextSelection.js';

// useTextSelection turns a DOM selection in the markdown-rendered article back
// into raw char offsets, then grows TextQuoteSelector context until the
// resolver finds it uniquely. The raw<->DOM gap (stripped "1. " prefixes,
// collapsed whitespace) is the same one useNotesHighlight handles; these tests
// pin the inverse direction and the context-growing loop.

/** Build a DOM tree from HTML and select a substring of one text node. */
function selectIn(html, findText) {
  document.body.innerHTML = `<div id="root">${html}</div>`;
  const root = document.getElementById('root');
  const walker = document.createTreeWalker(root, NodeFilter.SHOW_TEXT);
  let node;
  while ((node = walker.nextNode())) {
    const i = node.textContent.indexOf(findText);
    if (i === -1) continue;
    const range = document.createRange();
    range.setStart(node, i);
    range.setEnd(node, i + findText.length);
    const sel = window.getSelection();
    sel.removeAllRanges();
    sel.addRange(range);
    return root;
  }
  throw new Error(`text ${JSON.stringify(findText)} not found in DOM`);
}

describe('selectionToRawRange', () => {
  it('maps a selection in plain rendered text to raw offsets', () => {
    const raw = 'heeft de verzekerde aanspraak op zorgtoeslag';
    const root = selectIn(`<p>${raw}</p>`, 'zorgtoeslag');
    const range = selectionToRawRange(raw, root);
    expect(range).not.toBeNull();
    expect(raw.slice(range.start, range.end)).toBe('zorgtoeslag');
  });

  it('accounts for a list prefix marked stripped from the DOM', () => {
    // Raw lid: "1. Indien de normpremie"; rendered as <li> without "1. ".
    const raw = '1. Indien de normpremie';
    const root = selectIn('<ol><li>Indien de normpremie</li></ol>', 'normpremie');
    const range = selectionToRawRange(raw, root);
    expect(range).not.toBeNull();
    // The raw offset must include the stripped "1. " (3 chars): "normpremie"
    // starts at raw index 13, not the DOM index 10.
    expect(raw.slice(range.start, range.end)).toBe('normpremie');
    expect(range.start).toBe(13);
  });

  it('maps a selection that starts at an element boundary (ol, 0)', () => {
    // marked renders "<ol>\n<li>..." - childNodes[0] of <ol> is a "\n" text
    // node with no raw counterpart. A drag starting at the very beginning of
    // the list reports start as (ol, 0); the scan must skip the "\n" and not
    // dead-end. Regression for the element-endpoint-on-whitespace bug.
    const raw = '1. eerste lid\n\n2. tweede lid';
    document.body.innerHTML =
      '<div id="root"><ol>\n<li>eerste lid</li>\n<li>tweede lid</li>\n</ol></div>';
    const root = document.getElementById('root');
    const ol = root.querySelector('ol');
    const firstLiText = ol.querySelectorAll('li')[0].firstChild;
    const range = document.createRange();
    range.setStart(ol, 0); // element boundary, before the "\n"
    range.setEnd(firstLiText, 'eerste'.length);
    const sel = window.getSelection();
    sel.removeAllRanges();
    sel.addRange(range);
    const out = selectionToRawRange(raw, root);
    expect(out).not.toBeNull();
    // "eerste" is at raw offset 3 (after the stripped "1. ").
    expect(raw.slice(out.start, out.end)).toBe('eerste');
  });

  it('returns null for a collapsed (empty) selection', () => {
    const raw = 'abc';
    document.body.innerHTML = '<div id="root"><p>abc</p></div>';
    const root = document.getElementById('root');
    window.getSelection().removeAllRanges();
    expect(selectionToRawRange(raw, root)).toBeNull();
  });

  it('returns null when the selection is outside the root', () => {
    const raw = 'abc';
    document.body.innerHTML =
      '<div id="root"><p>abc</p></div><p id="other">def</p>';
    const root = document.getElementById('root');
    const other = document.getElementById('other');
    const range = document.createRange();
    range.selectNodeContents(other);
    const sel = window.getSelection();
    sel.removeAllRanges();
    sel.addRange(range);
    expect(selectionToRawRange(raw, root)).toBeNull();
  });
});

describe('buildSelector', () => {
  const raw = 'Indien de normpremie voor een verzekerde hoger is dan de normpremie';

  function engineReturning(bySelector) {
    return {
      resolveNote(_lawId, selector) {
        return bySelector(selector);
      },
    };
  }

  it('rejects a whitespace-only exact without calling the resolver', () => {
    // A selection that maps to just the space between two words: schema
    // minLength:1 and the resolver's non-empty guard would both pass it,
    // but a note quoting " " anchors nothing visible. Must short-circuit.
    const r = 'de  normpremie'; // two spaces at 2..4
    let called = false;
    const engine = engineReturning(() => {
      called = true;
      return { status: 'found', matches: [{}] };
    });
    const out = buildSelector(r, { start: 2, end: 4 }, 'w', engine, '1');
    expect(out.status).toBe('orphaned');
    expect(called).toBe(false);
  });

  it('accepts a bare selector only when the unique match IS our selection', () => {
    const range = { start: 10, end: 20 }; // "normpremie" (first)
    const engine = engineReturning(() => ({
      status: 'found',
      matches: [{ article_number: '1', start: 10, end: 20 }],
    }));
    const out = buildSelector(raw, range, 'w', engine, '1');
    expect(out.status).toBe('found');
    expect(out.selector.exact).toBe('normpremie');
    expect(out.selector.prefix).toBeUndefined();
    expect(out.selector.suffix).toBeUndefined();
  });

  it('rejects a unique match at the WRONG offsets as a mis-anchor', () => {
    // The resolver found a single match, but at offsets 56..66, not the
    // 10..20 the user selected (the trimmed prefix/suffix check matched the
    // other "normpremie"). This must NOT be accepted: it would silently
    // anchor the note to a different sentence.
    const range = { start: 10, end: 20 };
    const engine = engineReturning(() => ({
      status: 'found',
      matches: [{ article_number: '1', start: 56, end: 66 }],
    }));
    const out = buildSelector(raw, range, 'w', engine, '1');
    expect(out.status).toBe('ambiguous');
  });

  it('rejects a unique match in the WRONG article', () => {
    const range = { start: 10, end: 20 };
    const engine = engineReturning(() => ({
      status: 'found',
      matches: [{ article_number: '3', start: 10, end: 20 }],
    }));
    const out = buildSelector(raw, range, 'w', engine, '1');
    expect(out.status).toBe('ambiguous');
  });

  it('reports reason "too-common" when the bare quote matched many, even if widening degrades to orphaned', () => {
    // The "in"-case: no-context match is ambiguous (occurs everywhere), but
    // with markdown-stripped context the wider attempts can't relocate it and
    // the resolver returns orphaned. The user-facing reason must stay
    // "too-common", not "not-found".
    const range = { start: 10, end: 20 };
    const engine = engineReturning((sel) =>
      !sel.prefix && !sel.suffix
        ? { status: 'ambiguous', matches: [{}, {}, {}] }
        : { status: 'orphaned', matches: [] },
    );
    const out = buildSelector(raw, range, 'w', engine, '1');
    expect(out.reason).toBe('too-common');
  });

  it('reports reason "not-found" when nothing matched at any width', () => {
    const range = { start: 10, end: 20 };
    const engine = engineReturning(() => ({ status: 'orphaned', matches: [] }));
    const out = buildSelector(raw, range, 'w', engine, '1');
    expect(out.status).toBe('orphaned');
    expect(out.reason).toBe('not-found');
  });

  it('grows context until the match lands on our exact selection', () => {
    const range = { start: 10, end: 20 }; // "normpremie", appears twice
    const calls = [];
    const engine = engineReturning((sel) => {
      calls.push(sel);
      if (!sel.prefix && !sel.suffix) {
        return { status: 'ambiguous', matches: [{}, {}] };
      }
      return {
        status: 'found',
        matches: [{ article_number: '1', start: 10, end: 20 }],
      };
    });
    const out = buildSelector(raw, range, 'w', engine, '1');
    expect(out.status).toBe('found');
    expect(out.selector.prefix).toBeTruthy();
    expect(calls.length).toBeGreaterThan(1);
  });

  it('reports still-ambiguous when even wide context does not disambiguate', () => {
    const range = { start: 10, end: 20 };
    const engine = engineReturning(() => ({
      status: 'ambiguous',
      matches: [{}, {}],
    }));
    const out = buildSelector(raw, range, 'w', engine, '1');
    expect(out.status).toBe('ambiguous');
  });

  it('stops immediately on an orphaned result', () => {
    const range = { start: 10, end: 20 };
    let calls = 0;
    const engine = engineReturning(() => {
      calls++;
      return { status: 'orphaned', matches: [] };
    });
    const out = buildSelector(raw, range, 'w', engine, '1');
    expect(out.status).toBe('orphaned');
    expect(calls).toBe(1);
  });
});
