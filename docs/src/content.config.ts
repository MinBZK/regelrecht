import { defineCollection, z } from 'astro:content';
import { glob } from 'astro/loaders';

const docs = defineCollection({
  loader: glob({ pattern: '**/*.{md,mdx}', base: 'src/content/docs' }),
  schema: z.object({
    title: z.string().optional(),
    description: z.string().optional(),
    // Per-page language override. Docs default to English; a Dutch page (e.g.
    // the accessibility statement) sets `lang: nl` so its <html lang> — and
    // thus screen-reader pronunciation — matches the content.
    lang: z.enum(['en', 'nl']).optional(),
  }),
});

const rfcs = defineCollection({
  loader: glob({ pattern: 'rfc-*.md', base: 'src/content/rfcs' }),
  schema: z.object({
    title: z.string().optional(),
    description: z.string().optional(),
    // RFC metadata, in frontmatter so it is structured data rather than a
    // bold-labelled preamble parsed out of the body. Both status and
    // implementation are required enums: every RFC carries both (so an absent
    // implementation tag never reads as "unknown"), and a typo fails the build
    // here rather than rendering a silent grey "Unknown" badge.
    status: z.enum(['Draft', 'Proposed', 'Accepted', 'Rejected', 'Superseded']),
    implementation: z.enum([
      'Implemented',
      'Partially implemented',
      'Not implemented',
    ]),
    // Stored as a 'YYYY-MM-DD' string rather than z.date() so it round-trips
    // through the build without timezone shifts and renders verbatim.
    date: z.string().optional(),
    authors: z.array(z.string()).optional(),
    depends_on: z.array(z.string()).optional(),
    // Sidebar label; falls back to the stripped title when absent.
    short_title: z.string().optional(),
  }),
});

export const collections = { docs, rfcs };
