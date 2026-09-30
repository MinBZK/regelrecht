/*
 * The one normalisation the schema reference's filter runs, on both sides of
 * the comparison.
 *
 * Its own module for the same reason lib/roadmap-zoek.ts is: the build-time
 * haystack is written by lib/schema-reference.ts, which imports the schema
 * JSON and would drag all 70 KB of it into the client bundle if a browser
 * script imported from there. Keeping the shared function here lets the
 * haystack and the typed query call the very same code without that.
 *
 * That they are the same code is the point. A search compares two normalised
 * strings, and a normalisation applied to only one side stops matching without
 * failing anything — a second copy that drifts is the same bug on a delay.
 *
 * It is a re-export rather than a second implementation: the rule (lowercase,
 * strip diacritics, collapse whitespace) is the site's, not this page's, and
 * two copies of it would be exactly the drift this file exists to prevent.
 */

export { normaliseerZoekterm } from './roadmap-zoek';
