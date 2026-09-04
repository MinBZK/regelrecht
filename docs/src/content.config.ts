import { defineCollection, z } from 'astro:content';
import { glob } from 'astro/loaders';
import {
  ONDERZOEK_IDS,
  BOUW_IDS,
  PRIORITEIT_IDS,
  OMVANG_IDS,
  CATEGORIE_IDS,
  CAPABILITY_IDS,
} from '~/lib/roadmap';

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

/*
 * Werkpakketten of the roadmap (/roadmap), one markdown file per werkpakket.
 *
 * The files carry frontmatter only. The app that produced them regenerated a
 * markdown body from these same fields on every save; nothing renders that
 * body here, so it was dropped rather than kept as a second copy free to
 * drift. `toelichting` is the prose field, rendered by roadmap-markdown.ts.
 *
 * The enums are built from the id lists in lib/roadmap.ts, which is also what
 * the pages render labels from, so a value the schema accepts always has a
 * label. Repeating the ids here instead would let a value validate while
 * getPrioriteit() returns undefined, and the card would render no tag at all
 * for a value that was set. Empty strings are allowed everywhere they occur
 * in practice: most
 * werkpakketten have no prioriteit, omvang, categorie or capability yet, and
 * an absent value is written as '' rather than omitted.
 *
 * faseId and disciplineId stay plain strings: they point into
 * src/data/roadmap-config.json, which a per-entry schema cannot see. That
 * check, and the samenhangIds one, live in assertReferencesResolve().
 */
const werkpakketten = defineCollection({
  loader: glob({
    pattern: '*.md',
    base: 'src/content/roadmap/werkpakketten',
  }),
  schema: z.object({
    id: z.string().uuid(),
    titel: z.string(),
    faseId: z.string(),
    disciplineId: z.string(),
    prioriteit: z.enum(PRIORITEIT_IDS).or(z.literal('')),
    omvang: z.enum(OMVANG_IDS).or(z.literal('')),
    categorie: z.enum(CATEGORIE_IDS).or(z.literal('')),
    capability: z.enum(CAPABILITY_IDS).or(z.literal('')),
    capaciteit: z.string().default(''),
    toelichting: z.string().default(''),
    // Required, not defaulted: `volgorde` places the werkpakket inside its
    // matrix cell, and a default of 0 would silently sort a file that forgot
    // it to the front rather than say so.
    volgorde: z.number(),
    // A question is either plain text or text with a pointer into the
    // position paper. The union keeps every question that has no counterpart
    // in the paper exactly as it was, so only the files that gain a reference
    // change. Normalised for rendering by onderzoeksvraagLijst().
    onderzoeksvragen: z
      .array(
        z.union([
          z.string(),
          z.object({
            vraag: z.string(),
            // A section anchor from the paper, without the '#'.
            // assertPaperSections() checks it exists.
            paper: z.string().min(1),
          }),
        ]),
      )
      .default([]),
    samenhangIds: z.array(z.string().uuid()).default([]),
    // Two axes that genuinely diverge, so two fields rather than one.
    // A question can be answered without anything being built (onderzoek
    // 'beantwoord', bouw 'niet'), and a thing can be built while the question
    // behind it stays open (the reverse). Collapsing them into one status
    // would force a choice the roadmap cannot make.
    //
    // Both default to the least-claim value, so an existing werkpakket keeps
    // meaning what it meant: nothing asserted is not the same as "nothing
    // done", and the pages render an unset field as "niet bepaald".
    onderzoek: z.enum(ONDERZOEK_IDS).or(z.literal('')).default(''),
    bouw: z.enum(BOUW_IDS).or(z.literal('')).default(''),
    // RFC numbers whose design work belongs to this werkpakket, e.g. [13, 21].
    // The RFC keeps its own `implementation` field; this is a pointer, not a
    // copy of it. assertRfcReferences() checks each number exists.
    rfcs: z.array(z.number().int().positive()).default([]),
  }),
});

export const collections = { docs, rfcs, werkpakketten };
