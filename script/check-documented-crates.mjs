#!/usr/bin/env node
// Elke workspace-crate staat in de componentenlijst van CLAUDE.md.
//
// CLAUDE.md is het eerste dat een agent leest en het enige overzicht van wat
// deze monorepo bevat. Een crate die er niet in staat bestaat voor die agent
// niet: hij bouwt het opnieuw, of hij zoekt het antwoord in de verkeerde crate.
// De lijst liep vijf crates achter voordat deze poort er was.
//
// Alleen de aanwezigheid wordt getoetst, niet de omschrijving — dat laatste is
// een oordeel en hoort bij de review.
import { readFileSync } from 'node:fs';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';

const root = join(dirname(fileURLToPath(import.meta.url)), '..');

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

const claudeMd = readFileSync(join(root, 'CLAUDE.md'), 'utf8');
const missing = members.filter((m) => !claudeMd.includes(`\`packages/${m}/\``));

if (missing.length > 0) {
  console.error('DOCS-CRATES: workspace-crates die CLAUDE.md niet noemt:');
  for (const m of missing) console.error(`  packages/${m}/`);
  console.error('');
  console.error('Voeg een regel toe aan de componentenlijst bovenaan CLAUDE.md.');
  process.exit(1);
}
