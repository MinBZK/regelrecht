// Which @nldd/design-system entry points does a source tree actually need?
//
// Importing the package root pulls in all ~110 components; a site uses a
// fraction of them. This resolves the set of `nldd-*` tags in the source to
// the smallest set of per-component entry points that defines them.
//
// Not every mention of a tag is a use. A name in markdown inline code is
// prose — documentation that describes another app's markup — and only has to
// name a component that exists. Everything else needs its entry point.
//
// Sub-components ship with their parent (nldd-menu-item lives in ./menu,
// nldd-toolbar-item in ./toolbar), so a tag without its own entry maps to the
// longest entry that is a prefix of it.
//
// Used by check-nldd-imports.mjs, which fails the build when the import list
// in the source has drifted from what the markup uses.

import { readFileSync, readdirSync, statSync } from 'node:fs';
import { join, extname } from 'node:path';

import { fileURLToPath } from 'node:url';



/** The file types a source tree can render `nldd-*` tags from. */
export const EXTENSIONS = new Set(['vue', 'js', 'ts', 'astro', 'mdx', 'md', 'html']);

/** Where a backtick means inline code rather than a string. */
const MARKDOWN = new Set(['.md', '.mdx']);

/** Every `./x` entry the package exposes, without the leading `./`. */
export function packageEntries() {
  // The package does not export ./package.json, so locate it through a known
  // entry point and read it off disk.
  const button = fileURLToPath(import.meta.resolve('@nldd/design-system/button'));
  const root = button.slice(0, button.indexOf('/dist/'));
  const pkg = JSON.parse(readFileSync(join(root, 'package.json'), 'utf8'));
  return new Set(
    Object.keys(pkg.exports ?? {})
      .filter((k) => k.startsWith('./'))
      .map((k) => k.slice(2))
      // Stylesheets and helpers are not custom elements.
      .filter((k) => !k.startsWith('styles') && k !== 'breakpoints'),
  );
}

function* sourceFiles(dir, extensions) {
  for (const name of readdirSync(dir)) {
    if (name === 'node_modules' || name.startsWith('.')) continue;
    const path = join(dir, name);
    if (statSync(path).isDirectory()) yield* sourceFiles(path, extensions);
    else if (extensions.has(extname(path).slice(1))) yield path;
  }
}

/**
 * Tag names (without the `nldd-` prefix) found in `dir`, split by what the use
 * implies: `rendered` needs an import, `mentioned` only has to exist.
 */
export function usedTags(dir, extensions) {
  const rendered = new Set();
  const mentioned = new Set();
  for (const file of sourceFiles(dir, extensions)) {
    const text = readFileSync(file, 'utf8');
    for (const [, tag] of text.matchAll(/<(nldd-[a-z0-9-]+)/g)) rendered.add(tag.slice(5));
    // The quoted form used by createElement/querySelector — the org picker
    // builds its rows in JS, so markup alone would miss them.
    //
    // Except between backticks in markdown: there a backtick is inline-code
    // formatting, not a string. The docs describe which components the
    // *frontend* renders, and importing those into the docs site only grows a
    // bundle that never uses them. Such a name still has to resolve to a real
    // entry point, so prose that has gone stale — a component the design
    // system dropped — keeps failing the build. Everywhere else a backtick
    // delimits a template literal, so the exception stays in markdown.
    const prose = MARKDOWN.has(extname(file));
    for (const [, quote, tag] of text.matchAll(/(['"`])(nldd-[a-z0-9-]+)\1/g)) {
      (prose && quote === '`' ? mentioned : rendered).add(tag.slice(5));
    }
  }
  return { rendered, mentioned };
}

/**
 * What a source tree's tags mean for its import list: `needed` is the entry
 * points it must import, `unknown` every name — rendered or merely mentioned —
 * that resolves to no entry point at all.
 */
export function resolveUsage({ rendered, mentioned }, entries) {
  const { needed, unresolved } = resolveEntries(rendered, entries);
  const unknown = [
    ...new Set([...unresolved, ...resolveEntries(mentioned, entries).unresolved]),
  ].sort();
  return { needed, unknown };
}

/** Map tags onto entry points; `unresolved` holds anything the package cannot serve. */
export function resolveEntries(tags, entries) {
  const needed = new Set();
  const unresolved = [];
  for (const tag of tags) {
    if (entries.has(tag)) {
      needed.add(tag);
      continue;
    }
    const parents = [...entries].filter((e) => tag.startsWith(`${e}-`));
    if (parents.length === 0) {
      unresolved.push(tag);
      continue;
    }
    // Longest match wins: button-bar-divider belongs to button-bar, not button.
    needed.add(parents.sort((a, b) => b.length - a.length)[0]);
  }
  return { needed: [...needed].sort(), unresolved: unresolved.sort() };
}
