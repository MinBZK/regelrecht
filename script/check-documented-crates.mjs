#!/usr/bin/env node
// Elke workspace-crate staat in de mappenlijst van de architectuurpagina.
//
// docs/src/content/docs/guide/architecture.md ("Where the code lives") is het
// overzicht van wat deze monorepo bevat, en AGENTS.md verwijst ernaar in plaats
// van een eigen lijst bij te houden. Een crate die er niet in staat bestaat voor
// een lezer niet: die bouwt het opnieuw, of zoekt het antwoord in de verkeerde
// crate. De lijst in CLAUDE.md, waar dit overzicht eerst stond, liep vijf crates
// achter voordat deze poort er was.
//
// Alleen de aanwezigheid wordt getoetst, niet de omschrijving; dat laatste is
// een oordeel en hoort bij de review.
import { readFileSync } from 'node:fs';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';

const root = join(dirname(fileURLToPath(import.meta.url)), '..');
const PAGE = 'docs/src/content/docs/guide/architecture.md';

const manifest = readFileSync(join(root, 'packages/Cargo.toml'), 'utf8');
// Anker op regelbegin: `default-members` mag deze match niet kapen.
const membersBlock = manifest.match(/^members\s*=\s*\[([^\]]*)\]/m);
if (!membersBlock) {
  console.error('DOCS-CRATES: geen members-lijst in packages/Cargo.toml');
  process.exit(1);
}
const members = membersBlock[1]
  .split('\n')
  .map((l) => l.replace(/#.*$/, ''))
  .flatMap((l) => [...l.matchAll(/"([^"]+)"/g)].map((m) => m[1]));

if (members.length === 0) {
  console.error('DOCS-CRATES: members-lijst in packages/Cargo.toml is leeg; de poort toetst dan niets');
  process.exit(1);
}

// De boom staat in een codeblok: `│   ├── engine/  # ...` onder `├── packages/`.
const page = readFileSync(join(root, PAGE), 'utf8');
const listed = new Set(
  [...page.matchAll(/^│\s+[├└]── ([\w.-]+)\/\s/gm)].map((m) => m[1]),
);
const missing = members.filter((m) => !listed.has(m));

if (missing.length > 0) {
  console.error(`DOCS-CRATES: workspace-crates die ${PAGE} niet noemt:`);
  for (const m of missing) console.error(`  packages/${m}/`);
  console.error('');
  console.error('Voeg een regel toe aan de boom onder "Where the code lives".');
  process.exit(1);
}
