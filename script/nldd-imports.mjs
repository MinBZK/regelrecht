// Which @nldd/design-system entry points does a source tree actually need?
//
// Importing the package root pulls in all ~110 components; a site uses a
// fraction of them. This resolves the set of `nldd-*` tags in the source to
// the smallest set of per-component entry points that defines them.
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

/** Tag names (without the `nldd-` prefix) used anywhere in `dir`. */
export function usedTags(dir, extensions) {
  const found = new Set();
  for (const file of sourceFiles(dir, extensions)) {
    const text = readFileSync(file, 'utf8');
    // Markup, plus the quoted form used by createElement/querySelector — the
    // org picker builds its rows in JS, so markup alone would miss them.
    for (const [, tag] of text.matchAll(/<(nldd-[a-z0-9-]+)/g)) found.add(tag.slice(5));
    for (const [, tag] of text.matchAll(/['"`](nldd-[a-z0-9-]+)['"`]/g)) found.add(tag.slice(5));
  }
  return found;
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
