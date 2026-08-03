/**
 * palette — canvas colours, read from the CSS custom properties in styles.css.
 *
 * The prototypes draw on a canvas, which cannot use `var(--kind-struct)`. Rather
 * than keeping a second palette in sync by hand, the tokens are resolved from
 * `<html>` once and re-resolved when the theme changes, so the canvas follows
 * light/dark exactly like the DOM parts of the explorer.
 */
import { EDGE_STYLE } from '../composables/useArchGraph.js';

const FALLBACK = {
  '--bg': '#f6f7f9',
  '--surface': '#ffffff',
  '--surface-2': '#eef1f5',
  '--border': '#d3d9e0',
  '--text': '#1c2430',
  '--text-muted': '#5a6472',
  '--accent': '#6366f1',
  '--kind-crate': '#6366f1',
  '--kind-binary': '#8b5cf6',
  '--kind-module': '#0ea5e9',
  '--kind-trait': '#d946ef',
  '--kind-struct': '#0d9488',
  '--kind-enum': '#ca8a04',
  '--kind-fn': '#64748b',
  '--kind-method': '#64748b',
  '--kind-app': '#16a34a',
  '--kind-dir': '#0891b2',
  '--kind-component': '#42b883',
  '--kind-composable': '#db7734',
};

let cache = null;

/** Drop the cache; call when the theme changes. */
export function invalidatePalette() {
  cache = null;
}

export function palette() {
  if (cache) return cache;
  const out = { ...FALLBACK };
  if (typeof window !== 'undefined' && typeof getComputedStyle === 'function') {
    const style = getComputedStyle(document.documentElement);
    for (const key of Object.keys(FALLBACK)) {
      const v = style.getPropertyValue(key).trim();
      if (v) out[key] = v;
    }
  }
  cache = out;
  return out;
}

/** Colour for a model node kind, falling back to the muted text colour. */
export function kindColor(kind) {
  const p = palette();
  return p[`--kind-${kind}`] || p['--text-muted'];
}

/** Colour for a relation kind — the same three colours the current view uses. */
export function edgeColor(kind) {
  return EDGE_STYLE[kind]?.stroke || EDGE_STYLE.uses.stroke;
}

/** `#rrggbb` (or any CSS colour the browser resolved) with an alpha applied. */
export function withAlpha(color, alpha) {
  const a = Math.max(0, Math.min(1, alpha));
  if (a >= 1) return color;
  const hex = color.trim();
  const m = /^#([0-9a-f]{3}|[0-9a-f]{6})$/i.exec(hex);
  if (m) {
    const h = m[1].length === 3 ? m[1].replace(/./g, (c) => c + c) : m[1];
    const r = parseInt(h.slice(0, 2), 16);
    const g = parseInt(h.slice(2, 4), 16);
    const b = parseInt(h.slice(4, 6), 16);
    return `rgba(${r},${g},${b},${a})`;
  }
  const rgb = /^rgba?\(([^)]+)\)$/i.exec(hex);
  if (rgb) {
    const parts = rgb[1].split(/[,/]/).map((s) => s.trim());
    return `rgba(${parts[0]},${parts[1]},${parts[2]},${a})`;
  }
  return color;
}

/**
 * A stable colour per container, so the same crate keeps the same hue in all
 * three prototypes. Spread over the hue circle by index rather than taken from
 * the kind palette: containers have to be told apart from *each other*, and the
 * kind colours only distinguish crate from app from binary.
 */
export function containerColorFactory(containerIds) {
  const ids = [...containerIds].sort();
  const map = new Map();
  ids.forEach((id, i) => {
    // Golden-ratio hue stepping, so neighbouring containers do not get
    // neighbouring hues even though the ids are sorted.
    map.set(id, hslHex(((i * 137.508) % 360) / 360, 0.58, 0.52));
  });
  return (id) => map.get(id) || palette()['--text-muted'];
}

/** HSL → `#rrggbb`. Hex keeps `withAlpha` simple: one colour syntax to parse. */
function hslHex(h, s, l) {
  const f = (n) => {
    const k = (n + h * 12) % 12;
    const a = s * Math.min(l, 1 - l);
    const v = l - a * Math.max(-1, Math.min(k - 3, 9 - k, 1));
    return Math.round(v * 255)
      .toString(16)
      .padStart(2, '0');
  };
  return `#${f(0)}${f(8)}${f(4)}`;
}

/** Mix a colour toward black. Used where a pale token has to survive as a 2px cell. */
export function darken(color, amount = 0.3) {
  const m = /^#([0-9a-f]{3}|[0-9a-f]{6})$/i.exec(color.trim());
  if (!m) return color;
  const h = m[1].length === 3 ? m[1].replace(/./g, (c) => c + c) : m[1];
  const k = 1 - Math.max(0, Math.min(1, amount));
  const part = (i) =>
    Math.round(parseInt(h.slice(i, i + 2), 16) * k)
      .toString(16)
      .padStart(2, '0');
  return `#${part(0)}${part(2)}${part(4)}`;
}
