/** Thin client for server/presenterApi.js. Every call throws with the server's message. */

async function call(url, options) {
  const res = await fetch(url, options);
  const body = await res.json().catch(() => ({}));
  if (!res.ok) {
    const err = new Error(body.error ?? `${res.status} ${res.statusText}`);
    err.status = res.status;
    throw err;
  }
  return body;
}

const json = (method, data) => ({ method, headers: { 'Content-Type': 'application/json' }, body: JSON.stringify(data) });
const enc = encodeURIComponent;

export const listDecks = () => call('/api/decks');
export const readDeck = (deck) => call(`/api/decks/${enc(deck)}`);
export const writeFile = (deck, file, content, mtime) => call(`/api/decks/${enc(deck)}/${enc(file)}`, json('PUT', { content, mtime }));
export const createSlide = (deck, after) => call(`/api/decks/${enc(deck)}`, json('POST', { after }));

const wetCache = new Map();
/**
 * One article, from the corpus (`law`) or from a law YAML in the deck folder
 * (`deck` + `file`). Cached per query; `clearWetCache` runs when a deck or the
 * corpus changes on disk.
 */
export function fetchArticle({ law, article, date, deck, file }) {
  const q = file
    ? new URLSearchParams({ deck, file, article: article ?? '' })
    : new URLSearchParams({ law, article: article ?? '', date: date ?? '' });
  const key = q.toString();
  if (!wetCache.has(key)) {
    const p = call(`/api/wet?${key}`);
    p.catch(() => wetCache.delete(key));
    wetCache.set(key, p);
  }
  return wetCache.get(key);
}
export const clearWetCache = () => wetCache.clear();
