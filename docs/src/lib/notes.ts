/*
 * Notes helpers: URL shape, note ordering, and the outbound links a note can
 * carry.
 *
 * The URL is the part worth being careful about. `/notes/YYYY/MM/slug` is a
 * published contract: once a note is out, that path has to keep resolving.
 * It is derived here, in one place, from the filename — never from a title, a
 * counter, or anything else that can be edited later. `scripts/check-notes.mjs`
 * pins the set of built URLs so a rename that would break a live link fails
 * CI instead of shipping.
 */

/*
 * The notes section is published on the apex host. `astro.config.mjs` sets `site` to
 * the docs subdomain, and the same build serves both hostnames, so notes pages
 * carry an explicit canonical built from this constant rather than from
 * `Astro.site`.
 */
export const NOTES_SITE = 'https://regelrecht.rijks.app';

/* Public reading environment. `/library/<id>` redirects here, so this is the
 * stable target of a `regulations` entry. */
export const LAW_READER = 'https://editor.regelrecht.rijks.app/corpus-juris';

export interface NoteAuthor {
  /** Optional: a note may be published under a role alone. */
  name?: string;
  role: string;
}

/** Entry id as the glob loader reports it: `YYYY-MM-DD-slug` (no extension). */
const ID_PATTERN = /^(\d{4})-(\d{2})-\d{2}-(.+)$/;

/**
 * Route for a note, derived from its filename.
 *
 * Year and month come from the filename rather than the `date` frontmatter so
 * the file on disk and the published URL can never disagree — a note that is
 * edited later keeps the URL it was published under. `check-notes.mjs` asserts
 * the two agree at build time.
 */
export function notePath(id: string): string {
  const m = ID_PATTERN.exec(id.replace(/\.mdx?$/, ''));
  if (!m) {
    throw new Error(
      `Note "${id}" must be named YYYY-MM-DD-slug.md — the URL is derived from it.`,
    );
  }
  const [, year, month, slug] = m;
  return `/notes/${year}/${month}/${slug}`;
}

/** The `[...slug]` param for a note, i.e. its path without the `/notes/` prefix. */
export function noteParam(id: string): string {
  return notePath(id).replace(/^\/notes\//, '');
}

/** Absolute URL, for the canonical link. */
export function noteUrl(id: string): string {
  return NOTES_SITE + notePath(id);
}

/**
 * Byline: "Name, role" per author, or just the role when a note is published
 * anonymously. Roles carry the weight here — who was speaking matters more
 * than which person typed it, and an author is free to leave the name out.
 */
export function byline(authors: NoteAuthor[]): string {
  return authors
    .map((a) => (a.name ? `${a.name}, ${a.role}` : a.role))
    .join(' · ');
}

/** Dutch long date, e.g. "10 september 2026". Notes are Dutch. */
export function formatDate(date: string): string {
  return new Date(date + 'T00:00:00Z').toLocaleDateString('nl-NL', {
    day: 'numeric',
    month: 'long',
    year: 'numeric',
    timeZone: 'UTC',
  });
}


/**
 * Link to a regulation in the reading environment. The id is the law's `$id`
 * from the corpus (e.g. `wet_op_de_zorgtoeslag`); `check-notes.mjs` verifies it
 * exists, which cannot happen at image-build time because the docs Dockerfile
 * copies `docs/` only.
 */
export function regulationUrl(id: string): string {
  return `${LAW_READER}/${id}`;
}

/** `wet_op_de_zorgtoeslag` -> `wet op de zorgtoeslag`, for link text. */
export function regulationLabel(id: string): string {
  return id.replace(/_/g, ' ');
}
