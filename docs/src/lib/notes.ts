/*
 * Notes helpers: URL shape, post ordering, and the outbound links a post can
 * carry.
 *
 * The URL is the part worth being careful about. `/notes/YYYY/MM/slug` is a
 * published contract: once a post is out, that path has to keep resolving.
 * It is derived here, in one place, from the filename — never from a title, a
 * counter, or anything else that can be edited later. `scripts/check-notes.mjs`
 * pins the set of built URLs so a rename that would break a live link fails
 * CI instead of shipping.
 */

/*
 * The notes section is published on the apex host. `astro.config.mjs` sets `site` to
 * the docs subdomain, and the same build serves both hostnames, so notes pages
 * carry an explicit canonical and the feed builds absolute URLs from this
 * constant rather than from `Astro.site`.
 */
export const NOTES_SITE = 'https://regelrecht.rijks.app';

/* Public reading environment. `/library/<id>` redirects here, so this is the
 * stable target of a `regulations` entry. */
export const LAW_READER = 'https://editor.regelrecht.rijks.app/corpus-juris';

export interface NoteAuthor {
  /** Optional: a post may be published under a role alone. */
  name?: string;
  role: string;
}

/** Entry id as the glob loader reports it: `YYYY-MM-DD-slug` (no extension). */
const ID_PATTERN = /^(\d{4})-(\d{2})-\d{2}-(.+)$/;

/**
 * Route for a post, derived from its filename.
 *
 * Year and month come from the filename rather than the `date` frontmatter so
 * the file on disk and the published URL can never disagree — a post that is
 * edited later keeps the URL it was published under. `check-notes.mjs` asserts
 * the two agree at build time.
 */
export function postPath(id: string): string {
  const m = ID_PATTERN.exec(id.replace(/\.mdx?$/, ''));
  if (!m) {
    throw new Error(
      `Note "${id}" must be named YYYY-MM-DD-slug.md — the URL is derived from it.`,
    );
  }
  const [, year, month, slug] = m;
  return `/notes/${year}/${month}/${slug}`;
}

/** The `[...slug]` param for a post, i.e. its path without the `/notes/` prefix. */
export function postParam(id: string): string {
  return postPath(id).replace(/^\/notes\//, '');
}

/** Absolute URL, for the feed and the canonical link. */
export function postUrl(id: string): string {
  return NOTES_SITE + postPath(id);
}

/**
 * Byline: "Name, role" per author, or just the role when a post is published
 * anonymously. Roles carry the weight here — who was speaking matters more
 * than which person typed it, and an author is free to leave the name out.
 */
export function byline(authors: NoteAuthor[]): string {
  return authors
    .map((a) => (a.name ? `${a.name}, ${a.role}` : a.role))
    .join(' · ');
}

/** Dutch long date, e.g. "10 september 2026". Posts are Dutch. */
export function formatDate(date: string): string {
  return new Date(date + 'T00:00:00Z').toLocaleDateString('nl-NL', {
    day: 'numeric',
    month: 'long',
    year: 'numeric',
    timeZone: 'UTC',
  });
}

/** RFC-822 date, as RSS requires. */
export function rfc822(date: string): string {
  return new Date(date + 'T00:00:00Z').toUTCString();
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

/** Escape the five XML entities. The feed is assembled as text. */
export function xmlEscape(s: string): string {
  return s
    .replace(/&/g, '&amp;')
    .replace(/</g, '&lt;')
    .replace(/>/g, '&gt;')
    .replace(/"/g, '&quot;')
    .replace(/'/g, '&apos;');
}
