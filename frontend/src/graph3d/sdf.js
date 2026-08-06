/**
 * Signed distance fields for the label atlas.
 *
 * Pure array maths, no canvas and no WebGL, so it is unit-testable and runs
 * the same in a worker. `sdfAtlas.js` supplies the rasterised glyph coverage;
 * this file turns coverage into a distance field that stays crisp at any zoom,
 * which is the whole reason labels are textured quads instead of DOM nodes.
 *
 * The transform is Felzenszwalb & Huttenlocher's exact Euclidean distance
 * transform: one parabola-envelope pass per axis, O(n) per row.
 */

/**
 * 1D squared-distance transform of `f` (length n) into `d`.
 * `v` and `z` are scratch buffers of length n and n + 1.
 */
export function edt1d(f, d, v, z, n) {
  let k = 0;
  v[0] = 0;
  z[0] = -Infinity;
  z[1] = Infinity;
  for (let q = 1; q < n; q++) {
    let s = (f[q] + q * q - (f[v[k]] + v[k] * v[k])) / (2 * q - 2 * v[k]);
    while (s <= z[k]) {
      k--;
      s = (f[q] + q * q - (f[v[k]] + v[k] * v[k])) / (2 * q - 2 * v[k]);
    }
    k++;
    v[k] = q;
    z[k] = s;
    z[k + 1] = Infinity;
  }
  k = 0;
  for (let q = 0; q < n; q++) {
    while (z[k + 1] < q) k++;
    const dist = q - v[k];
    d[q] = dist * dist + f[v[k]];
  }
}

/** 2D squared-distance transform in place over a width x height grid. */
export function edt2d(grid, width, height) {
  const n = Math.max(width, height);
  const f = new Float64Array(n);
  const d = new Float64Array(n);
  const v = new Int32Array(n);
  const z = new Float64Array(n + 1);
  for (let x = 0; x < width; x++) {
    for (let y = 0; y < height; y++) f[y] = grid[y * width + x];
    edt1d(f, d, v, z, height);
    for (let y = 0; y < height; y++) grid[y * width + x] = d[y];
  }
  for (let y = 0; y < height; y++) {
    for (let x = 0; x < width; x++) f[x] = grid[y * width + x];
    edt1d(f, d, v, z, width);
    for (let x = 0; x < width; x++) grid[y * width + x] = d[x];
  }
  return grid;
}

const INF = 1e20;

/**
 * Turn glyph coverage (alpha 0..255) into an 8-bit signed distance field.
 *
 * The output encodes distance in pixels around the glyph edge, mapped so that
 * 0.5 (128) is exactly on the edge, `range` pixels outside is 0 and `range`
 * pixels inside is 255. That is the convention the label shader expects.
 *
 * @param {Uint8Array|Uint8ClampedArray} alpha coverage, width*height
 * @param {number} width
 * @param {number} height
 * @param {number} range distance range in pixels
 * @returns {Uint8Array}
 */
export function sdfFromAlpha(alpha, width, height, range = 6) {
  const n = width * height;
  const inside = new Float64Array(n);
  const outside = new Float64Array(n);
  for (let i = 0; i < n; i++) {
    const a = alpha[i] / 255;
    // Antialiased coverage would need a subpixel estimate; a 0.5 threshold is
    // enough here because the glyphs are rasterised at 3x the atlas cell.
    if (a > 0.5) {
      outside[i] = 0;
      inside[i] = INF;
    } else {
      outside[i] = INF;
      inside[i] = 0;
    }
  }
  edt2d(outside, width, height);
  edt2d(inside, width, height);
  const out = new Uint8Array(n);
  for (let i = 0; i < n; i++) {
    const d = Math.sqrt(outside[i]) - Math.sqrt(inside[i]);
    const v = 0.5 - d / (range * 2);
    out[i] = Math.max(0, Math.min(255, Math.round(v * 255)));
  }
  return out;
}
