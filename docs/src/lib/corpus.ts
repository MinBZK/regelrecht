/*
 * Where the repository is, for the pages that read the corpus at build time.
 *
 * One rule, in one place: `process.cwd()` is the docs project in `astro dev`,
 * in `astro build` and in the image, and the repository is one level above it.
 * Deriving it from `import.meta.url` does not survive the build, because these
 * modules are bundled and no longer sit where their source does.
 *
 * It lived in ~/lib/landing-demo.ts and was copied into ~/lib/scenario-demo.ts
 * the moment a second page wanted a file out of the corpus. Two copies of the
 * answer to "where is the repository" is one too many: the image lays the
 * pieces out differently from a checkout, and the day that moves, both have to
 * move together or one page silently reads nothing.
 */
import { readFileSync } from 'node:fs';
import { join } from 'node:path';

export const repo = join(process.cwd(), '..');

/** A file elsewhere in the repository, chiefly the corpus. */
export const read = (...parts: string[]) => readFileSync(join(repo, ...parts), 'utf8');
