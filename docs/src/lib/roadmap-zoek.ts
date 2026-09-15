/*
 * The one normalisation the roadmap's zoekfilter runs, on both sides of the
 * comparison.
 *
 * Its own module, and deliberately so: this is the only part of the roadmap
 * code that ships to the browser. lib/roadmap.ts imports `astro:content` and
 * (through lib/rfcs.ts) node:fs, both server-only, so a client script that
 * imports from there fails the build with "The astro:content module is only
 * available server-side". Keeping the shared function here lets the build-time
 * haystack (zoektekst(), lib/roadmap.ts) and the typed query (the script in
 * pages/roadmap/index.astro) call the very same code.
 *
 * That they are the same code is the point. A search compares two normalised
 * strings, and a normalisation applied to only one side stops matching without
 * failing anything — a second copy that drifts is the same bug on a delay.
 */

/**
 * Lowercase, strip diacritics, collapse whitespace.
 *
 * So "verifieren" finds the capability "Verifiëren en simuleren" and a stray
 * casing or double space never costs a hit. NFD splits a letter from its
 * accent and the range drops the accent; \p{Diacritic} would need a newer
 * target than this build sets.
 *
 * The range stays written as \u escapes. Spelled with the characters
 * themselves it is a run of bare combining accents that renders as a smudge on
 * the preceding bracket, invisible to review and silently destroyed by
 * anything that normalises or trims the file — and a broken range here costs
 * the diacritic folding without failing a build.
 */
export function normaliseerZoekterm(tekst: string): string {
  return tekst
    .normalize('NFD')
    .replace(/[\u0300-\u036f]/g, '')
    .toLowerCase()
    .replace(/\s+/g, ' ')
    .trim();
}
