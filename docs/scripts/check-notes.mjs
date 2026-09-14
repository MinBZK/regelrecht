#!/usr/bin/env node
/*
 * Notes checks that the Astro build cannot make.
 *
 * Three of them:
 *
 *  1. Filename shape and date agreement. The published URL is derived from the
 *     filename, the visible date from frontmatter; if they disagree, a note
 *     lives at a URL that contradicts what it says.
 *
 *  2. URL stability. Every URL that exists on the default branch must still
 *     exist here. `/notes/YYYY/MM/slug` is a published contract, so a rename
 *     that would break a live link fails instead of shipping. Adding and
 *     removing notes is fine; moving an existing one is not.
 *
 *  3. `regulations` ids resolve. The docs image builds from `docs/` alone
 *     (see docs/Dockerfile), so the corpus is not there at build time — this
 *     check has to run in CI, where the whole repository is checked out.
 *
 * Exit 0 when everything holds, 1 with one line per problem otherwise.
 */
import { readdirSync, readFileSync, existsSync } from 'node:fs';
import { join, dirname, relative } from 'node:path';
import { fileURLToPath } from 'node:url';
import { execFileSync } from 'node:child_process';

const HERE = dirname(fileURLToPath(import.meta.url));
const REPO = join(HERE, '..', '..');
const NOTES_DIR = join(REPO, 'docs', 'src', 'content', 'notes');
const CORPUS_DIR = join(REPO, 'corpus', 'regulation');
const POST_PATTERN = /^(\d{4})-(\d{2})-(\d{2})-(.+)\.mdx?$/;

const problems = [];
const fail = (file, message) => problems.push(`${file}: ${message}`);

/* Narrow frontmatter reader: the fields this script checks, nothing more.
 * Astro's own schema validates the rest, with better errors than a
 * hand-rolled parser would give. */
function frontmatter(src) {
  const block = /^---\r?\n([\s\S]*?)\r?\n---/.exec(src);
  if (!block) return null;
  const body = block[1];
  const date = /^date:\s*['"]?(\d{4}-\d{2}-\d{2})['"]?\s*$/m.exec(body)?.[1];
  // A scaffolded note carries TODO in the fields only the author can fill.
  // The schema accepts them (they are strings), so they would publish.
  const todos = [...body.matchAll(/^\s*(?:-\s*)?(\w+):.*\bTODO\b/gm)].map((m) => m[1]);
  if (/^summary:\s*>-?\s*\n\s*TODO\s*$/m.test(body) && !todos.includes('summary')) {
    todos.push('summary');
  }
  const regulations = [];
  const list = /^regulations:\s*\n((?:\s*-\s*.+\n?)+)/m.exec(body);
  if (list) {
    for (const line of list[1].split('\n')) {
      const id = /^\s*-\s*['"]?([^'"\s]+)['"]?\s*$/.exec(line)?.[1];
      if (id) regulations.push(id);
    }
  }
  return { date, regulations, todos };
}

/** Every law `$id` in this repository's corpus. */
function corpusIds() {
  const ids = new Set();
  const walk = (dir) => {
    for (const entry of readdirSync(dir, { withFileTypes: true })) {
      const path = join(dir, entry.name);
      if (entry.isDirectory()) walk(path);
      else if (entry.name.endsWith('.yaml')) {
        // The directory name is not the id (a municipal by-law appends the
        // municipality), so read the field rather than infer it.
        const id = /^\$id:\s*(\S+)\s*$/m.exec(readFileSync(path, 'utf8'))?.[1];
        if (id) ids.add(id);
      }
    }
  };
  if (existsSync(CORPUS_DIR)) walk(CORPUS_DIR);
  return ids;
}

/** `2026-09-10-slug.md` -> `/notes/2026/09/slug`. */
const urlFor = (filename) => {
  const m = POST_PATTERN.exec(filename);
  return m ? `/notes/${m[1]}/${m[2]}/${m[4]}` : null;
};

/**
 * Post filenames on the default branch. Returns null when that cannot be
 * established (no git, shallow clone, no remote) — a missing baseline is not
 * a failure, it just means this run cannot check stability.
 */
function publishedFilenames() {
  const dir = relative(REPO, NOTES_DIR).replaceAll('\\', '/');
  const git = (args) =>
    execFileSync('git', args, {
      cwd: REPO,
      encoding: 'utf8',
      stdio: ['ignore', 'pipe', 'ignore'],
    });
  for (const ref of ['origin/main', 'main']) {
    try {
      git(['rev-parse', '--verify', '--quiet', ref]);
    } catch {
      continue; // ref not available in this checkout
    }
    try {
      const out = git(['ls-tree', '--name-only', `${ref}:${dir}`]);
      return out.split('\n').filter((n) => POST_PATTERN.test(n));
    } catch {
      // The ref exists but carries no notes directory: nothing published yet,
      // so the baseline is empty rather than unknown.
      return [];
    }
  }
  return null;
}

const files = existsSync(NOTES_DIR)
  ? readdirSync(NOTES_DIR).filter((f) => /\.mdx?$/.test(f))
  : [];

const ids = corpusIds();
const urls = new Map();

for (const file of files) {
  if (file === 'README.md') continue;
  const match = POST_PATTERN.exec(file);
  if (!match) {
    fail(file, 'must be named YYYY-MM-DD-slug.md — the URL is derived from it');
    continue;
  }
  const [, year, month, day, slug] = match;
  const url = `/notes/${year}/${month}/${slug}`;

  if (urls.has(url)) {
    fail(file, `collides with ${urls.get(url)} on ${url}`);
  }
  urls.set(url, file);

  const data = frontmatter(readFileSync(join(NOTES_DIR, file), 'utf8'));
  if (!data) {
    fail(file, 'has no frontmatter block');
    continue;
  }
  const filenameDate = `${year}-${month}-${day}`;
  if (!data.date) {
    fail(file, 'has no date in its frontmatter');
  } else if (data.date !== filenameDate) {
    fail(file, `date ${data.date} does not match the filename date ${filenameDate}`);
  }

  if (data.todos.length > 0) {
    fail(file, `still has TODO in: ${[...new Set(data.todos)].join(', ')}`);
  }

  for (const id of data.regulations) {
    if (ids.size === 0) break; // no corpus checked out; reported once below
    if (!ids.has(id)) {
      fail(file, `regulations: "${id}" is not a law $id in corpus/regulation`);
    }
  }
}

const published = publishedFilenames();
if (published === null) {
  console.log('check-notes: no main branch to compare against; URL stability not checked');
} else {
  for (const name of published) {
    const url = urlFor(name);
    if (url && !urls.has(url)) {
      problems.push(
        `${name}: ${url} is published on main but no note produces it any more — ` +
          'note URLs are a contract; keep the filename',
      );
    }
  }
}

if (ids.size === 0) {
  console.log('check-notes: corpus/regulation not present; regulations ids not checked');
}

if (problems.length > 0) {
  console.error('check-notes: problems found\n');
  for (const p of problems) console.error(`  ${p}`);
  process.exit(1);
}

console.log(
  `check-notes: ${urls.size} note(s) OK` +
    (published === null ? '' : `, ${published.length} published URL(s) intact`),
);
