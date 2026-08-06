#!/usr/bin/env node
/**
 * Write a .gz and .br next to every compressible file in the build output.
 *
 * editor-api serves the editor's static files itself (tower-http `ServeDir`),
 * and `ServeDir` is configured with `precompressed_br()` /
 * `precompressed_gzip()`. That means it looks for `foo.js.br` and `foo.js.gz`
 * on disk and hands one of those to a client whose `Accept-Encoding` allows
 * it, falling back to the plain file otherwise. Compressing here, once per
 * build, keeps that cost out of the request path entirely: the container never
 * spends CPU on compression, no matter how many people load the editor.
 *
 * Both variants are written because the split is worth it — brotli wins on
 * size for every modern browser, gzip covers whatever does not send `br`.
 * The plain file always stays; it is what `ServeDir` serves when neither
 * encoding is accepted, and it is what the `Content-Type`/ETag machinery
 * needs.
 *
 * Node's zlib does gzip and brotli in the standard library, so this adds no
 * dependency to the frontend.
 */
import { readdir, readFile, rename, stat, writeFile, unlink } from 'node:fs/promises';
import { join, extname, basename, dirname } from 'node:path';
import { realpathSync } from 'node:fs';
import { fileURLToPath } from 'node:url';
import { promisify } from 'node:util';
import zlib from 'node:zlib';

const gzip = promisify(zlib.gzip);
const brotli = promisify(zlib.brotliCompress);

/**
 * Extensions worth compressing. Everything here is text or (in the case of
 * wasm) a bytecode format that still gives up ~80% of its size. Deliberately
 * absent: png/jpg/woff2/ico — already compressed, so a second pass costs
 * build time and disk for a rounding error, or makes the file bigger.
 */
export const COMPRESSIBLE = new Set([
  '.js',
  '.mjs',
  '.css',
  '.html',
  '.json',
  '.svg',
  '.wasm',
  '.txt',
  '.xml',
  // Corpus YAML: only present when a local-source registry is configured
  // (scripts/copy-laws.js copies it into public/data/), but it is the most
  // compressible thing the editor can ship, so never skip it.
  '.yaml',
  '.yml',
]);

/** Encoding suffixes this script owns and is therefore allowed to delete. */
const VARIANT_SUFFIXES = ['.gz', '.br'];

/**
 * Below this, compression is pointless: the gzip/brotli framing plus a
 * `Vary`-split cache entry costs more than the handful of bytes saved, and a
 * sub-1KB response fits in one packet either way.
 */
export const MIN_SIZE = 1024;

/**
 * Keep a variant only if it is meaningfully smaller. A file that barely
 * compresses (an already-minified data blob, say) would otherwise cost a
 * second disk read at request time for ~nothing.
 */
export const MIN_RATIO = 0.95;

/**
 * How many files to compress at once. Brotli at max quality is the expensive
 * part and libuv's threadpool caps the real parallelism anyway; the bound is
 * here so that a dist with thousands of files (a local corpus copy under
 * `data/`) cannot open every file at once and blow up on fds or RSS.
 */
export const CONCURRENCY = 8;

export function shouldCompress(name, size) {
  if (VARIANT_SUFFIXES.some((s) => name.endsWith(s))) return false;
  if (!COMPRESSIBLE.has(extname(name))) return false;
  return size >= MIN_SIZE;
}

async function* walk(dir) {
  for (const entry of await readdir(dir, { withFileTypes: true })) {
    const full = join(dir, entry.name);
    if (entry.isDirectory()) {
      yield* walk(full);
    } else if (entry.isFile()) {
      yield full;
    }
  }
}

async function drop(path) {
  await unlink(path).catch((e) => {
    if (e.code !== 'ENOENT') throw e;
  });
}

/**
 * Write atomically. An interrupted build must not leave a truncated `.br`
 * behind: `ServeDir` would serve those bytes as a complete body and the
 * browser would fail to decode them.
 */
async function writeAtomic(path, data) {
  const tmp = join(dirname(path), `.${basename(path)}.tmp`);
  await writeFile(tmp, data);
  await rename(tmp, path);
}

async function compressFile(path) {
  const source = await readFile(path);
  const [gz, br] = await Promise.all([
    gzip(source, { level: zlib.constants.Z_BEST_COMPRESSION }),
    brotli(source, {
      params: {
        [zlib.constants.BROTLI_PARAM_QUALITY]: zlib.constants.BROTLI_MAX_QUALITY,
        [zlib.constants.BROTLI_PARAM_SIZE_HINT]: source.length,
      },
    }),
  ]);

  const written = { raw: source.length, gz: 0, br: 0 };

  for (const [suffix, buf, key] of [
    ['.gz', gz, 'gz'],
    ['.br', br, 'br'],
  ]) {
    if (buf.length < source.length * MIN_RATIO) {
      await writeAtomic(`${path}${suffix}`, buf);
      written[key] = buf.length;
    } else {
      await drop(`${path}${suffix}`);
    }
  }

  return written;
}

/** Run `task` over `items`, at most `limit` at a time. */
async function mapLimit(items, limit, task) {
  const results = new Array(items.length);
  let next = 0;
  const workers = Array.from({ length: Math.min(limit, items.length) }, async () => {
    while (next < items.length) {
      const i = next++;
      results[i] = await task(items[i]);
    }
  });
  await Promise.all(workers);
  return results;
}

export async function precompress(dir) {
  const targets = [];
  const variants = [];
  for await (const path of walk(dir)) {
    if (VARIANT_SUFFIXES.some((s) => path.endsWith(s))) {
      variants.push(path);
      continue;
    }
    const { size } = await stat(path);
    if (shouldCompress(path, size)) targets.push(path);
  }

  // Sweep orphans before compressing. `ServeDir` opens `foo.js.br` without
  // ever checking that `foo.js` is still there, so a variant left over from a
  // previous build is served as if it were current content — under the base
  // file's Content-Type, no less. Vite empties `dist` on every build, so this
  // only bites a hand-run over an existing directory; that is exactly when it
  // is hardest to spot.
  const keep = new Set(targets);
  const orphans = variants.filter((v) => !keep.has(v.slice(0, -3)));
  await Promise.all(orphans.map(drop));

  const results = await mapLimit(targets, CONCURRENCY, compressFile);

  return results.reduce(
    (acc, r) => ({
      files: acc.files + 1,
      raw: acc.raw + r.raw,
      gz: acc.gz + (r.gz || r.raw),
      br: acc.br + (r.br || r.raw),
      orphans: acc.orphans,
    }),
    { files: 0, raw: 0, gz: 0, br: 0, orphans: orphans.length },
  );
}

const kb = (n) => `${(n / 1024).toFixed(0)} kB`;

async function main() {
  const dir = process.argv[2] || 'dist';
  const started = Date.now();
  const { files, raw, gz, br, orphans } = await precompress(dir);
  if (orphans > 0) {
    console.log(`precompress: removed ${orphans} stale variant(s)`);
  }
  if (files === 0) {
    console.log(`precompress: nothing to compress in ${dir}`);
    return;
  }
  const saved = ((1 - br / raw) * 100).toFixed(0);
  console.log(
    `precompress: ${files} files, ${kb(raw)} → ${kb(gz)} gzip, ${kb(br)} brotli ` +
      `(-${saved}% over the wire) in ${((Date.now() - started) / 1000).toFixed(1)}s`,
  );
}

// Run only when invoked directly, so the test can import the functions above
// without kicking off a compression run. Both sides go through realpath: a
// mismatch here would silently skip the whole step and ship an uncompressed
// build with a green exit code.
const invokedDirectly =
  process.argv[1] && realpathSync(fileURLToPath(import.meta.url)) === realpathSync(process.argv[1]);

if (invokedDirectly) {
  main().catch((e) => {
    console.error(e);
    process.exit(1);
  });
}
