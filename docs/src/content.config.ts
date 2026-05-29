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
  }),
});

export const collections = { docs, rfcs };
