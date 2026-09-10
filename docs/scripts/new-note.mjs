#!/usr/bin/env node
/*
 * Scaffold a note: `just note "Title of the note"`.
 *
 * The barrier to writing is the empty file — the name, the date format, the
 * frontmatter fields. This fills those in and stops there. It writes no
 * headings and no sections: what a note says is the author's business, and a
 * template with slots invites filling them in rather than saying something.
 *
 * Everything it guesses is easy to correct afterwards, except the filename,
 * which sets the published URL. That is why it prints the URL before it exits.
 */
import { writeFileSync, existsSync, mkdirSync } from 'node:fs';
import { join, dirname } from 'node:path';
import { fileURLToPath } from 'node:url';
import { execFileSync } from 'node:child_process';

const HERE = dirname(fileURLToPath(import.meta.url));
const NOTES_DIR = join(HERE, '..', 'src', 'content', 'notes');

const title = process.argv.slice(2).join(' ').trim();
if (!title) {
  console.error('Usage: just note "Title of the note"');
  process.exit(2);
}

/** Dutch title -> url slug: strip diacritics, keep letters, digits and dashes. */
function slugify(s) {
  return s
    .normalize('NFD')
    .replace(/[̀-ͯ]/g, '')
    .toLowerCase()
    .replace(/['’]/g, '')
    .replace(/[^a-z0-9]+/g, '-')
    .replace(/^-+|-+$/g, '')
    .slice(0, 60);
}

const slug = slugify(title);
if (!slug) {
  console.error(`Cannot derive a slug from "${title}" — use letters or digits.`);
  process.exit(2);
}

const today = new Date().toISOString().slice(0, 10);
const filename = `${today}-${slug}.md`;
const path = join(NOTES_DIR, filename);

if (existsSync(path)) {
  console.error(`${filename} already exists.`);
  process.exit(1);
}

/* The author's name from git, because it is already configured and correct.
 * The role is left blank on purpose: it is the one field only the author can
 * fill, and a guess would be quietly wrong. A note may also carry a role with
 * no name at all — remove the name line to publish that way. */
let name = '';
try {
  name = execFileSync('git', ['config', 'user.name'], { encoding: 'utf8' }).trim();
} catch {
  /* no git identity configured; leave it to the author */
}

// Quote the title only when YAML would otherwise misread it.
const safeTitle = /^[\w][\w \-.,()]*$/.test(title)
  ? title
  : `'${title.replace(/'/g, "''")}'`;

/* TODO placeholders rather than blanks: an empty `role:` is YAML null and
 * fails the build with a type error, which says nothing useful. check-notes.mjs
 * names the field instead, and a leftover TODO can never reach the site. */
const content = `---
title: ${safeTitle}
date: '${today}'
authors:
  - ${name ? `name: ${name}\n    role: TODO` : 'role: TODO'}
summary: >-
  TODO
tags: []
---

`;

mkdirSync(NOTES_DIR, { recursive: true });
writeFileSync(path, content, 'utf8');

console.log(`
Created  docs/src/content/notes/${filename}
Publishes at  /notes/${today.slice(0, 4)}/${today.slice(5, 7)}/${slug}

Two TODOs to replace:
  role     what you were doing here, e.g. "ontwikkelaar" or "jurist"
  summary  one or two sentences; this is what the overview and the feed show

Then write. No headings required; start them at ## if you use any.
Preview with  just notes
`);
