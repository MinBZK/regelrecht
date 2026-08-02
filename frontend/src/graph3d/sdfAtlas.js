/**
 * SDF glyph atlas for graph labels.
 *
 * Labels are not DOM. A `CSS2DRenderer` label is a positioned <div>, and a few
 * hundred of those eat the frame budget in layout and style recalculation
 * alone - which is the ceiling the design puts at 150 to 300. Glyphs from a
 * single-channel SDF atlas are instanced quads: one draw call for every label
 * on screen, and they stay sharp at any zoom because the texture stores
 * distance to the glyph edge instead of coverage.
 *
 * The atlas is built once per font, on a 2D canvas, at module level of the
 * view. Glyph coverage goes through `sdfFromAlpha`.
 */

import { sdfFromAlpha } from './sdf.js';

/** Latin-1 range that covers Dutch law titles, plus the punctuation we emit. */
export const DEFAULT_CHARSET =
  ' !"#$%&\'()*+,-./0123456789:;<=>?@' +
  'ABCDEFGHIJKLMNOPQRSTUVWXYZ[\\]^_`' +
  'abcdefghijklmnopqrstuvwxyz{|}~' +
  'àáâäçèéêëìíîïñòóôöùúûüÀÁÂÄÇÈÉÊËÌÍÎÏÑÒÓÔÖÙÚÛÜ€“”‘’…·';

/**
 * @param {object} [opts]
 * @param {string} [opts.charset]
 * @param {number} [opts.fontSize] rasterisation size in px
 * @param {number} [opts.padding]  distance range around the glyph, in px
 * @param {string} [opts.fontFamily]
 * @returns {{texture: {data: Uint8Array, width: number, height: number},
 *            glyphs: Map<string, {u0:number,v0:number,u1:number,v1:number,
 *                                 w:number,h:number,advance:number,
 *                                 bearingX:number,bearingY:number}>,
 *            lineHeight: number, range: number}}
 */
export function buildSdfAtlas({
  charset = DEFAULT_CHARSET,
  fontSize = 32,
  padding = 6,
  fontFamily = 'system-ui, sans-serif',
} = {}) {
  const chars = Array.from(new Set(Array.from(charset)));
  const cell = fontSize + padding * 2;
  const cols = Math.ceil(Math.sqrt(chars.length));
  const rows = Math.ceil(chars.length / cols);
  const width = nextPow2(cols * cell);
  const height = nextPow2(rows * cell);

  const canvas = createCanvas(cell, cell);
  const ctx = canvas.getContext('2d', { willReadFrequently: true });
  ctx.font = `${fontSize}px ${fontFamily}`;
  ctx.textBaseline = 'alphabetic';
  ctx.fillStyle = '#fff';

  const data = new Uint8Array(width * height);
  const glyphs = new Map();
  const baseline = padding + fontSize * 0.8;

  chars.forEach((ch, i) => {
    const col = i % cols;
    const row = (i / cols) | 0;
    ctx.clearRect(0, 0, cell, cell);
    ctx.fillText(ch, padding, baseline);
    const img = ctx.getImageData(0, 0, cell, cell).data;
    const alpha = new Uint8Array(cell * cell);
    for (let p = 0; p < cell * cell; p++) alpha[p] = img[p * 4 + 3];
    const sdf = sdfFromAlpha(alpha, cell, cell, padding);
    const ox = col * cell;
    const oy = row * cell;
    for (let y = 0; y < cell; y++) {
      data.set(sdf.subarray(y * cell, y * cell + cell), (oy + y) * width + ox);
    }
    const advance = ctx.measureText(ch).width / fontSize;
    glyphs.set(ch, {
      u0: ox / width,
      v0: oy / height,
      u1: (ox + cell) / width,
      v1: (oy + cell) / height,
      // Quad size and offset in em, including the padding ring: the shader
      // draws the whole cell, the SDF decides what is ink.
      w: cell / fontSize,
      h: cell / fontSize,
      bearingX: -padding / fontSize,
      bearingY: baseline / fontSize,
      advance,
    });
  });

  // A headless browser without any installed font measures every glyph at
  // zero advance and rasterises nothing. That is an environment gap, not a
  // rendering bug, and the caller has to know: labels are then impossible and
  // the layer is skipped rather than drawing 400 invisible quads.
  const usable = (glyphs.get('n')?.advance ?? 0) > 0;

  return {
    texture: { data, width, height },
    glyphs,
    lineHeight: 1.25,
    range: padding / fontSize,
    usable,
  };
}

function nextPow2(n) {
  let p = 1;
  while (p < n) p *= 2;
  return p;
}

function createCanvas(w, h) {
  if (typeof OffscreenCanvas === 'function') return new OffscreenCanvas(w, h);
  const c = document.createElement('canvas');
  c.width = w;
  c.height = h;
  return c;
}

/**
 * Lay a string out into glyph quads, in em units relative to the label origin.
 * Pure: no canvas, no atlas texture, so the label budget and the truncation
 * rule can be tested without a GPU.
 *
 * @param {string} text
 * @param {Map} glyphs
 * @param {number} [maxEm] truncate with an ellipsis beyond this advance
 * @param {number} [maxGlyphs] truncate with an ellipsis beyond this glyph count
 * @returns {{quads: Array<{glyph: object, x: number}>, width: number}}
 */
export function layoutLabel(text, glyphs, maxEm = Infinity, maxGlyphs = Infinity) {
  const quads = [];
  let x = 0;
  const ellipsis = glyphs.get('…');
  const ellipsisW = ellipsis ? ellipsis.advance : 0;
  for (const ch of text) {
    const g = glyphs.get(ch);
    if (!g) continue;
    // Two caps, because narrow text can stay well inside the width budget and
    // still blow past the per-label glyph reservation, which would silently
    // eat the tail of the whole label selection.
    if ((x + g.advance > maxEm - ellipsisW || quads.length >= maxGlyphs - 1) && ellipsis) {
      quads.push({ glyph: ellipsis, x });
      x += ellipsisW;
      break;
    }
    quads.push({ glyph: g, x });
    x += g.advance;
  }
  return { quads, width: x };
}
