// Node's built-in test runner, so this costs no dependency. Run with
// `just precompress-test` (part of `just check`).
//
// What matters here is not the compression ratio — zlib is not under test —
// but the disk hygiene around it. `ServeDir` opens `foo.js.br` without
// checking whether `foo.js` still exists or still matches, so every rule this
// script applies to what lands next to a file is a correctness rule, not an
// optimisation.
import { test } from 'node:test';
import assert from 'node:assert/strict';
import { mkdtemp, mkdir, readFile, writeFile, readdir, rm } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { gunzipSync, brotliDecompressSync } from 'node:zlib';

import { precompress, shouldCompress, MIN_SIZE } from './precompress.mjs';

/** Highly compressible filler of a given size. */
const filler = (n) => 'const x = 1; // padding\n'.repeat(Math.ceil(n / 24)).slice(0, n);

async function fixture() {
  const dir = await mkdtemp(join(tmpdir(), 'precompress-'));
  return dir;
}

test('shouldCompress accepts a normal asset and rejects the uncompressible', () => {
  assert.equal(shouldCompress('app.js', 5000), true);
  assert.equal(shouldCompress('engine_bg.wasm', 5000), true);
  assert.equal(shouldCompress('laws.yaml', 5000), true);

  // Already compressed: a second pass costs build time for nothing.
  assert.equal(shouldCompress('font.woff2', 5000), false);
  assert.equal(shouldCompress('logo.png', 5000), false);

  // Never recurse over our own output.
  assert.equal(shouldCompress('app.js.br', 5000), false);
  assert.equal(shouldCompress('app.js.gz', 5000), false);
});

test('shouldCompress skips files below MIN_SIZE', () => {
  assert.equal(shouldCompress('app.js', MIN_SIZE - 1), false);
  assert.equal(shouldCompress('app.js', MIN_SIZE), true);
});

test('variants sit next to the file and decompress back to the original', async () => {
  const dir = await fixture();
  try {
    const original = filler(4000);
    await writeFile(join(dir, 'app.js'), original);

    const stats = await precompress(dir);
    assert.equal(stats.files, 1);

    const gz = await readFile(join(dir, 'app.js.gz'));
    const br = await readFile(join(dir, 'app.js.br'));
    assert.equal(gunzipSync(gz).toString(), original);
    assert.equal(brotliDecompressSync(br).toString(), original);

    // The plain file must survive: it is what a client that accepts neither
    // encoding gets.
    assert.equal(await readFile(join(dir, 'app.js'), 'utf8'), original);
  } finally {
    await rm(dir, { recursive: true, force: true });
  }
});

test('nested directories are covered', async () => {
  const dir = await fixture();
  try {
    await mkdir(join(dir, 'wasm', 'pkg'), { recursive: true });
    await writeFile(join(dir, 'wasm', 'pkg', 'engine_bg.wasm'), filler(4000));

    await precompress(dir);

    const files = await readdir(join(dir, 'wasm', 'pkg'));
    assert.ok(files.includes('engine_bg.wasm.br'));
  } finally {
    await rm(dir, { recursive: true, force: true });
  }
});

test('incompressible content gets no variant', async () => {
  const dir = await fixture();
  try {
    // Random bytes do not compress; writing a .br that is bigger than the
    // source would cost a disk read per request to send more bytes.
    const noise = Buffer.alloc(4000);
    for (let i = 0; i < noise.length; i++) noise[i] = (Math.random() * 256) | 0;
    await writeFile(join(dir, 'noise.json'), noise);

    await precompress(dir);

    const files = await readdir(dir);
    assert.deepEqual(files, ['noise.json']);
  } finally {
    await rm(dir, { recursive: true, force: true });
  }
});

test('a stale variant whose source shrank below MIN_SIZE is removed', async () => {
  const dir = await fixture();
  try {
    await writeFile(join(dir, 'app.js'), filler(4000));
    await precompress(dir);
    assert.ok((await readdir(dir)).includes('app.js.br'));

    // Rebuild: the file is now too small to be worth compressing. Leaving
    // yesterday's .br behind would make ServeDir answer /app.js with the old
    // content for every client that accepts brotli.
    await writeFile(join(dir, 'app.js'), 'const x = 1;\n');
    const stats = await precompress(dir);

    assert.equal(stats.orphans, 2);
    assert.deepEqual(await readdir(dir), ['app.js']);
  } finally {
    await rm(dir, { recursive: true, force: true });
  }
});

test('a variant whose source disappeared is removed', async () => {
  const dir = await fixture();
  try {
    await writeFile(join(dir, 'old-hash.js'), filler(4000));
    await precompress(dir);

    await rm(join(dir, 'old-hash.js'));
    await writeFile(join(dir, 'new-hash.js'), filler(4000));
    const stats = await precompress(dir);

    assert.equal(stats.orphans, 2);
    const files = (await readdir(dir)).sort();
    assert.deepEqual(files, ['new-hash.js', 'new-hash.js.br', 'new-hash.js.gz']);
  } finally {
    await rm(dir, { recursive: true, force: true });
  }
});

test('no temp files are left behind', async () => {
  const dir = await fixture();
  try {
    await writeFile(join(dir, 'app.js'), filler(4000));
    await precompress(dir);

    const leftovers = (await readdir(dir)).filter((f) => f.endsWith('.tmp'));
    assert.deepEqual(leftovers, []);
  } finally {
    await rm(dir, { recursive: true, force: true });
  }
});
