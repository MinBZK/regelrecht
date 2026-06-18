#!/usr/bin/env node
// Generate frontend/src/gherkin/grammar.generated.js from bdd/grammar.yaml.
//
// Single source of truth: bdd/grammar.yaml. Never hand-edit the generated file.
// Run via `just bdd-codegen`, or automatically through the frontend
// prebuild/pretest hooks. Resolves the repo root from THIS file's location
// (two levels up) so it works whether invoked from the repo root or frontend/.
import { existsSync, readFileSync, writeFileSync } from 'node:fs';
import { fileURLToPath } from 'node:url';
import { dirname, join } from 'node:path';
import { createRequire } from 'node:module';

// Repo root = two levels up from bdd/codegen/gen-js.mjs (-> bdd/ -> repo root).
const root = join(dirname(fileURLToPath(import.meta.url)), '..', '..');

// This runs as the frontend `predev`/`pretest` hooks and via `just bdd-codegen`,
// all of which execute in a full repo checkout. The production `build` does NOT
// regenerate (it uses the committed `grammar.generated.js`, whose freshness CI
// guards), because the frontend Docker context is `frontend/` only and lacks the
// repo-root `bdd/` directory. The guard below keeps this safe if it is ever run
// without the grammar present: skip rather than fail.
const grammarPath = join(root, 'bdd', 'grammar.yaml');
if (!existsSync(grammarPath)) {
  console.log(`bdd/grammar.yaml not found at ${grammarPath}; skipping codegen (using committed grammar.generated.js)`);
  process.exit(0);
}

// js-yaml is a frontend dependency; resolve it from frontend/node_modules so
// the generator has a YAML parser without adding a root-level dependency.
const frontendRequire = createRequire(join(root, 'frontend', 'node_modules', 'js-yaml', 'package.json'));
const yaml = frontendRequire('js-yaml');

const grammar = yaml.load(readFileSync(grammarPath, 'utf8'));

const STR_SENTINEL = '\0S\0';
const NUM_SENTINEL = '\0N\0';

function toPattern(step) {
  // Build an anchored JS regex source.
  // "{name}" (quoted string arg) -> "([^"]*)" ; {name} (numeric arg) -> (-?\d+(?:\.\d+)?)
  let working = step.text;
  for (const a of step.args ?? []) {
    if (a.type === 'string') working = working.replace(`"{${a.name}}"`, STR_SENTINEL);
    else working = working.replace(`{${a.name}}`, NUM_SENTINEL);
  }
  const escaped = working.replace(/[.*+?^${}()|[\]\\]/g, '\\$&');
  const body = escaped
    .replaceAll(STR_SENTINEL, '"([^"]*)"')
    .replaceAll(NUM_SENTINEL, '(-?\\d+(?:\\.\\d+)?)');
  return `^${body}$`;
}

function templateFn(step) {
  // Emit a JS arrow function that rebuilds the canonical line from ordered args.
  // Quoted args keep their surrounding quotes; numeric args interpolate bare.
  let text = step.text;
  (step.args ?? []).forEach((a, i) => {
    if (a.type === 'string') text = text.replace(`"{${a.name}}"`, `"\${a[${i}]}"`);
    else text = text.replace(`{${a.name}}`, `\${a[${i}]}`);
  });
  return '(a) => `' + text + '`';
}

const entries = grammar.steps.map((step) => {
  const argTypes = (step.args ?? []).map((a) => a.type);
  const literals = step.literals ?? [];
  return `  { id: ${JSON.stringify(step.id)}, action: ${JSON.stringify(step.action)}, ` +
    `keyword: ${JSON.stringify(step.keyword)}, tier: ${JSON.stringify(step.tier ?? 'core')}, ` +
    `datatable: ${step.datatable ? 'true' : 'false'}, ` +
    `pattern: /${toPattern(step)}/, ` +
    `argTypes: ${JSON.stringify(argTypes)}, ` +
    `literals: ${JSON.stringify(literals)}, ` +
    `template: ${templateFn(step)} }`;
});

const out = `// @generated from bdd/grammar.yaml by bdd/codegen/gen-js.mjs — do not edit.
export const GRAMMAR = [
${entries.join(',\n')},
];
`;

const dest = join(root, 'frontend', 'src', 'gherkin', 'grammar.generated.js');
writeFileSync(dest, out);
console.log(`wrote ${dest} (${grammar.steps.length} steps)`);
