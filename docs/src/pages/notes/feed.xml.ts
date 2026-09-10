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
import { NOTES_SITE, byline, postUrl, rfc822, xmlEscape } from '~/lib/notes';

const FEED_TITLE = 'RegelRecht notities';
const FEED_DESCRIPTION =
  'Notities van het RegelRecht-team over uitvoerbare wetgeving, het corpus en de techniek eronder.';

export const GET: APIRoute = async () => {
  const posts = (await getCollection('notes')).sort((a, b) =>
    b.data.date.localeCompare(a.data.date),
  );

  const items = posts
    .map((post) => {
      const url = postUrl(post.id);
      return `    <item>
      <title>${xmlEscape(post.data.title)}</title>
      <link>${xmlEscape(url)}</link>
      <guid isPermaLink="true">${xmlEscape(url)}</guid>
      <pubDate>${rfc822(post.data.date)}</pubDate>
      <dc:creator>${xmlEscape(byline(post.data.authors))}</dc:creator>
      <description>${xmlEscape(post.data.summary)}</description>
${post.data.tags.map((t) => `      <category>${xmlEscape(t)}</category>`).join('\n')}
    </item>`;
    })
    .join('\n');

  // lastBuildDate follows the newest post rather than the build clock, so an
  // unrelated rebuild does not present itself to a reader as new activity.
  const lastBuild = posts.length > 0 ? rfc822(posts[0].data.date) : undefined;

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
