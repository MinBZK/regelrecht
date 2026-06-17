/**
 * Human-readable fallback for a law id, used when the real name isn't available
 * yet (corpus not loaded, or the law isn't in the active corpus):
 * `algemene_wet_inkomensafhankelijke_regelingen`
 *   → "Algemene Wet Inkomensafhankelijke Regelingen".
 *
 * Title-cased to match the library's existing display; callers prefer the real
 * `name` and only fall back to this.
 */
export function humanizeLawId(id) {
  return String(id ?? '').replace(/_/g, ' ').replace(/\b\w/g, (c) => c.toUpperCase());
}
