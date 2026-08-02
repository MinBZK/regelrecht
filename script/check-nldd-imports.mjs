// Fails the build when a site's per-component design-system imports have
// drifted from the nldd-* tags its source actually uses.
//
// Without this, adding a component to a template is silent: the tag renders as
// an unknown element, never upgrades, and the FOUC guard holds the page hidden
// until its 200ms fallback. That is a lot harder to spot than a failing build.
//
//   node script/check-nldd-imports.mjs <source-dir> <imports-file> [--write]
//
// --write regenerates the imports file instead of failing.

import { readFileSync, writeFileSync } from 'node:fs';
import { packageEntries, usedTags, resolveEntries } from './nldd-imports.mjs';

const [sourceDir, importsFile, ...flags] = process.argv.slice(2);
if (!sourceDir || !importsFile) {
  console.error('usage: check-nldd-imports.mjs <source-dir> <imports-file> [--write]');
  process.exit(2);
}

const EXTENSIONS = new Set(['vue', 'js', 'ts', 'astro', 'mdx', 'md']);
const HEADER = `// Design-system components this app renders, one entry point each.
// The package root would pull in all ~110 components; this list is generated
// from the nldd-* tags in the source and checked on every build by
// script/check-nldd-imports.mjs, so a newly used component fails the build
// instead of silently never upgrading.
//
// Regenerate: npm run nldd:imports
`;

const { needed, unresolved } = resolveEntries(usedTags(sourceDir, EXTENSIONS), packageEntries());

if (unresolved.length > 0) {
  console.error(`✗ ${sourceDir}: no design-system entry point defines these tags:`);
  for (const tag of unresolved) console.error(`    nldd-${tag}`);
  console.error('  Either the tag is a typo, or the package needs a new entry point.');
  process.exit(1);
}

const expected = HEADER + needed.map((e) => `import '@nldd/design-system/${e}';`).join('\n') + '\n';

if (flags.includes('--write')) {
  writeFileSync(importsFile, expected);
  console.log(`✓ wrote ${needed.length} imports to ${importsFile}`);
  process.exit(0);
}

const actual = readFileSync(importsFile, 'utf8');
if (actual === expected) {
  console.log(`✓ ${importsFile}: ${needed.length} imports, in sync with ${sourceDir}`);
  process.exit(0);
}

const has = new Set([...actual.matchAll(/@nldd\/design-system\/([a-z0-9-]+)/g)].map((m) => m[1]));
const missing = needed.filter((e) => !has.has(e));
const extra = [...has].filter((e) => !needed.includes(e));

console.error(`✗ ${importsFile} is out of date.`);
if (missing.length) console.error(`  used but not imported: ${missing.join(', ')}`);
if (extra.length) console.error(`  imported but unused:   ${extra.join(', ')}`);
console.error('  Run: npm run nldd:imports');
process.exit(1);
