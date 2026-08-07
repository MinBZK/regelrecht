// Assert every npm package the docs source imports is declared in
// docs/package.json.
//
// An undeclared import resolves anyway as long as some dependency happens to
// pull the package in and npm hoists it to the top of docs/node_modules. That
// makes the failure latent: nothing in docs/ has to change for a dedupe in
// Astro's tree to move the package down a level and break the build. This check
// makes the omission fail now instead.

import { readdirSync, statSync, readFileSync } from 'node:fs';
import { join, extname } from 'node:path';
import { fileURLToPath } from 'node:url';
import { builtinModules } from 'node:module';

const ROOT = fileURLToPath(new URL('..', import.meta.url));
const SRC = join(ROOT, 'src');
const EXTENSIONS = new Set(['.ts', '.mts', '.js', '.mjs', '.astro', '.vue']);

const pkg = JSON.parse(readFileSync(join(ROOT, 'package.json'), 'utf8'));
const declared = new Set([
  ...Object.keys(pkg.dependencies ?? {}),
  ...Object.keys(pkg.devDependencies ?? {}),
  ...Object.keys(pkg.peerDependencies ?? {}),
]);

const builtins = new Set([...builtinModules, ...builtinModules.map((m) => `node:${m}`)]);

function sourceFiles(dir) {
  const out = [];
  for (const entry of readdirSync(dir)) {
    const full = join(dir, entry);
    if (statSync(full).isDirectory()) out.push(...sourceFiles(full));
    else if (EXTENSIONS.has(extname(entry))) out.push(full);
  }
  return out;
}

// `import x from 's'`, `import 's'`, `export … from 's'`, `import('s')`,
// `require('s')`. Also catches the type-only forms, which need the package just
// as much: tsc resolves `import type { Root } from 'hast'` through node_modules.
const SPECIFIER = /(?:\bfrom|\bimport|\brequire)\s*\(?\s*['"]([^'"\n]+)['"]/g;

function packageName(specifier) {
  const parts = specifier.split('/');
  return specifier.startsWith('@') ? parts.slice(0, 2).join('/') : parts[0];
}

const missing = new Map();
let seen = 0;

for (const file of sourceFiles(SRC)) {
  const source = readFileSync(file, 'utf8');
  for (const [, specifier] of source.matchAll(SPECIFIER)) {
    // Relative paths, the `~/` src alias, and the `astro:*` / `virtual:*`
    // module namespaces are not npm packages.
    if (/^[./~#]/.test(specifier) || specifier.includes(':')) continue;
    if (builtins.has(specifier)) continue;
    seen++;
    const name = packageName(specifier);
    // A package that ships no runtime code (`hast`, `unist`) is satisfied by
    // its DefinitelyTyped stub, which TypeScript resolves under the same
    // specifier. `@scope/name` maps to `@types/scope__name`.
    const typesName = name.startsWith('@')
      ? `@types/${name.slice(1).replace('/', '__')}`
      : `@types/${name}`;
    if (declared.has(name) || declared.has(typesName)) continue;
    if (!missing.has(name)) missing.set(name, new Set());
    missing.get(name).add(file.replace(ROOT, 'docs/'));
  }
}

if (missing.size) {
  console.error(`Undeclared imports in docs/src (${missing.size}):`);
  for (const [name, files] of [...missing].sort()) {
    console.error(`  ${name} — ${[...files].sort().join(', ')}`);
  }
  console.error('\nAdd them to docs/package.json (types and type-only packages in devDependencies).');
  process.exit(1);
}

// A green run must mean imports were actually inspected. Zero means the walk or
// the regex stopped matching and the check is passing vacuously.
if (seen === 0) {
  console.error('Undeclared-import check found NO package imports in docs/src. Failing rather than passing vacuously.');
  process.exit(1);
}

console.log(`Undeclared-import check passed: ${seen} package import(s), all declared.`);
