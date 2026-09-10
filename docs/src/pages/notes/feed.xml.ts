/*
 * RSS 2.0 feed for the notes section (/notes/feed.xml).
 *
 * Hand-assembled rather than pulled in from @astrojs/rss: the feed is a fixed
 * shape over a handful of fields, and the dependency would buy nothing that
 * this file does not already do. Everything interpolated goes through
 * xmlEscape().
 *
 * Absolute URLs come from NOTES_SITE, not from Astro.site: the same build is
 * served on both the apex and the docs subdomain, and the notes section is published on
 * the apex.
 */
import type { APIRoute } from 'astro';
import { getCollection } from 'astro:content';
import { NOTES_SITE, byline, noteUrl, rfc822, xmlEscape } from '~/lib/notes';

const FEED_TITLE = 'RegelRecht notities';
const FEED_DESCRIPTION =
  'Notities van het RegelRecht-team over uitvoerbare wetgeving, het corpus en de techniek eronder.';

export const GET: APIRoute = async () => {
  const notes = (await getCollection('notes')).sort(
    (a, b) => b.data.date.localeCompare(a.data.date) || a.id.localeCompare(b.id),
  );

  const items = notes
    .map((note) => {
      const url = noteUrl(note.id);
      return `    <item>
      <title>${xmlEscape(note.data.title)}</title>
      <link>${xmlEscape(url)}</link>
      <guid isPermaLink="true">${xmlEscape(url)}</guid>
      <pubDate>${rfc822(note.data.date)}</pubDate>
      <dc:creator>${xmlEscape(byline(note.data.authors))}</dc:creator>
      <description>${xmlEscape(note.data.summary)}</description>${note.data.tags
        .map((t) => `\n      <category>${xmlEscape(t)}</category>`)
        .join('')}
    </item>`;
    })
    .join('\n');

  // lastBuildDate follows the newest note rather than the build clock, so an
  // unrelated rebuild does not present itself to a reader as new activity.
  const lastBuild = notes.length > 0 ? rfc822(notes[0].data.date) : undefined;

  const xml = `<?xml version="1.0" encoding="UTF-8"?>
<rss version="2.0"
     xmlns:atom="http://www.w3.org/2005/Atom"
     xmlns:dc="http://purl.org/dc/elements/1.1/">
  <channel>
    <title>${xmlEscape(FEED_TITLE)}</title>
    <link>${NOTES_SITE}/notes</link>
    <description>${xmlEscape(FEED_DESCRIPTION)}</description>
    <language>nl-NL</language>
    <atom:link href="${NOTES_SITE}/notes/feed.xml" rel="self" type="application/rss+xml" />
${lastBuild ? `    <lastBuildDate>${lastBuild}</lastBuildDate>\n` : ''}${items}
  </channel>
</rss>
`;

  return new Response(xml, {
    headers: { 'Content-Type': 'application/rss+xml; charset=utf-8' },
  });
};
