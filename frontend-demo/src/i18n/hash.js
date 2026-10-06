/**
 * The fingerprint a translation is pinned to.
 *
 * `en.sources.js` records, per key, the hash of the Dutch string that was
 * translated. When the Dutch changes, the hash stops matching and the parity
 * test names the key — which is the one thing a type system cannot do and the
 * gap the docs landing page still has: there, a stale-but-present translation
 * passes the build silently.
 *
 * FNV-1a truncated to four hex characters. A collision would mean one missed
 * staleness warning, never a wrong string on screen, so the short form is
 * worth it for a file a human reads in diffs.
 */
export function hash(text) {
  let h = 0x811c9dc5;
  for (let i = 0; i < text.length; i += 1) {
    h ^= text.charCodeAt(i);
    h = Math.imul(h, 0x01000193) >>> 0;
  }
  return h.toString(16).padStart(8, '0').slice(0, 4);
}
