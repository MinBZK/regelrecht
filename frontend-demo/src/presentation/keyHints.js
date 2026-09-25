/**
 * The key hints under the slides, as translatable sentences.
 *
 * A hint is a sentence with keys in it: "← → of Spatie bladeren". The keys are
 * `nldd-keyboard-shortcut` elements, not text, so the sentence cannot go
 * through `v-html` the way the Presentatie tab does it with `<kbd>`. Instead
 * the dictionary holds the whole sentence with a `{placeholder}` per key, and
 * this splits it into text and key segments for the template to render.
 *
 * Whole sentences rather than the words between the keys: a language that puts
 * the verb first ("to browse, press ...") can then do so, and the placeholder
 * check in `i18n.test.js` guards that no translation drops a key.
 */

/** Which key each placeholder stands for, as `nldd-keyboard-shortcut` spells it. */
export const HINT_KEYS = Object.freeze({
  prev: '←',
  next: '→',
  space: 'Space',
  esc: 'Esc',
  f: 'F',
});

/** The dictionary keys of the hints, in the order they appear. */
export const HINTS = Object.freeze(['deck.hint.browse', 'deck.hint.close', 'deck.hint.fullscreen']);

/**
 * "{esc} sluit" → [{ key: 'Esc' }, { text: ' sluit' }].
 *
 * An unknown placeholder stays as text, braces and all: a typo in a dictionary
 * then shows up on screen instead of silently dropping a word.
 */
export function hintSegments(sentence) {
  const text = String(sentence);
  const segments = [];
  let last = 0;
  for (const m of text.matchAll(/\{(\w+)\}/g)) {
    if (!(m[1] in HINT_KEYS)) continue;
    if (m.index > last) segments.push({ text: text.slice(last, m.index) });
    segments.push({ key: HINT_KEYS[m[1]] });
    last = m.index + m[0].length;
  }
  if (last < text.length) segments.push({ text: text.slice(last) });
  return segments;
}
