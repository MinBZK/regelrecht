import { describe, it, expect } from 'vitest';
import { readFileSync } from 'node:fs';
import path from 'node:path';
import { DEFAULTS } from './useFeatureFlags.js';

// The backend's DEFAULTS map is both its default set and the allow-list for
// PUT /api/feature-flags/{key}. A flag the frontend knows but the backend does
// not is a toggle that 400s and silently reverts (panel.notes was exactly
// that); a flag only the backend knows is dead weight nobody reads. This test
// reads the Rust source so neither side can move without the other.
const RUST_SOURCE = path.resolve(
  import.meta.dirname,
  '../../../packages/editor-api/src/feature_flags.rs',
);

function backendDefaults() {
  const src = readFileSync(RUST_SOURCE, 'utf8');
  const consts = Object.fromEntries(
    [...src.matchAll(/pub const ([A-Z_]+): &str = "([^"]+)";/g)].map((m) => [m[1], m[2]]),
  );
  const block = src.match(/static DEFAULTS[\s\S]*?HashMap::from\(\[([\s\S]*?)\]\)/);
  if (!block) throw new Error(`DEFAULTS block not found in ${RUST_SOURCE}`);
  const body = block[1].replace(/\/\/.*$/gm, '');
  const entries = [...body.matchAll(/\(\s*(?:"([^"]+)"|([A-Z_]+))\.into\(\),\s*(true|false)\s*\)/g)];
  // Every `.into()` entry must have parsed, or a new entry shape slipped past
  // the regex and the comparison below would be vacuously incomplete.
  expect(entries.length).toBe((body.match(/\.into\(\)/g) ?? []).length);
  return Object.fromEntries(
    entries.map(([, literal, ident, value]) => {
      const key = literal ?? consts[ident];
      if (!key) throw new Error(`unresolved flag constant ${ident}`);
      return [key, value === 'true'];
    }),
  );
}

describe('feature flag defaults', () => {
  it('match the backend registry in keys and default values', () => {
    expect(DEFAULTS).toEqual(backendDefaults());
  });
});
