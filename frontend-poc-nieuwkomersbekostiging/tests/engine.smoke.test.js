import { describe, it, expect } from 'vitest';
import { sharedEngine } from './helpers/nodeEngine.js';

describe('WASM-engine', () => {
  it('boot en laadt de corpus-wetten', async () => {
    const engine = await sharedEngine();
    const laws = engine.listLaws();
    expect(Array.isArray(laws)).toBe(true);
  });
});
