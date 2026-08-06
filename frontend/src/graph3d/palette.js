/**
 * Colours for the 3D graph, read from the design-system tokens.
 *
 * Same discipline as `graph/graph-styles.css`: every colour is a NDD primitive
 * token, and dark mode comes for free because those tokens are `light-dark()`
 * pairs. The renderer cannot use CSS, so the tokens are read once with
 * `getComputedStyle` and turned into numbers; on a theme switch they are read
 * again and the materials updated. That is exactly what section 7.5 of the
 * design prescribes and it is the reason there is no colour literal below
 * except the fallbacks, which only fire when no stylesheet is present (unit
 * tests, the benchmark harness).
 */

// Seven palette families, in the order `graph-styles.css` uses them for
// regulatory_layer. Here they carry the cluster hue instead - the design moves
// regulatory layer onto geometry - so the two views keep one vocabulary.
export const CLUSTER_FAMILIES = [
  'lintblauw',
  'violet',
  'mintgroen',
  'donkergeel',
  'paars',
  'geel',
  'coolgray',
];

const FALLBACK = {
  'lintblauw-500': '#2b5fd9',
  'lintblauw-700': '#1b3f96',
  'violet-500': '#7a4fd6',
  'violet-700': '#513392',
  'mintgroen-500': '#2fa37a',
  'mintgroen-700': '#1d6b50',
  'donkergeel-500': '#c99700',
  'donkergeel-700': '#8a6800',
  'paars-500': '#a3459b',
  'paars-700': '#6e2e68',
  'geel-500': '#e2c000',
  'geel-700': '#9c8500',
  'coolgray-300': '#9aa5b5',
  'coolgray-500': '#7b8794',
  'coolgray-700': '#4b5563',
  'rood-500': '#d64545',
  'groen-500': '#2f9e44',
  'neutral-0': '#ffffff',
  'neutral-900': '#14181f',
};

/**
 * Read a token and get a *resolved* colour back.
 *
 * `getComputedStyle(el).getPropertyValue('--x')` returns the literal text of
 * the custom property, and the NDD tokens are `light-dark(oklch(...),
 * oklch(...))` pairs. Parsing that here would mean reimplementing the cascade,
 * so instead a probe element gets `color: var(--token)` and its computed
 * `color` is read: the browser then does the resolving, the light/dark pick and
 * the conversion, and hands back an `rgb(...)`. Without a document (unit tests,
 * the benchmark harness) the hard-coded fallback stands in.
 */
let probe = null;

function readToken(name, root) {
  if (typeof getComputedStyle !== 'function' || !root || typeof document === 'undefined') {
    return FALLBACK[name];
  }
  const host = root.ownerDocument?.body ?? root;
  if (!probe) {
    probe = document.createElement('span');
    probe.setAttribute('aria-hidden', 'true');
    probe.style.cssText = 'position:absolute;width:0;height:0;visibility:hidden;';
  }
  if (!probe.isConnected) host.appendChild(probe);
  // The fallback goes inside var(): an unresolvable token then yields the
  // hard-coded colour instead of silently inheriting whatever the parent had,
  // and there is nothing to detect afterwards.
  probe.style.color = `var(--primitives-color-${name}, ${FALLBACK[name]})`;
  return getComputedStyle(probe).color || FALLBACK[name];
}

/** Drop the probe element; call when the view goes away. */
export function releasePaletteProbe() {
  probe?.remove();
  probe = null;
}

/**
 * Ask the browser what a colour string actually is.
 *
 * Chrome serialises a computed `color` in the notation it was written in, so
 * an NDD token comes back as `oklch(0.563 0.04 257.4)` and not as `rgb(...)`.
 * Reimplementing oklch here would be a colour-science project; painting one
 * pixel and reading it back is exact, costs microseconds and handles every
 * notation the design system may switch to later.
 */
let swatch = null;

function resolveViaCanvas(value) {
  if (typeof document === 'undefined') return null;
  if (!swatch) {
    const canvas = document.createElement('canvas');
    canvas.width = 1;
    canvas.height = 1;
    swatch = canvas.getContext('2d', { willReadFrequently: true });
    if (!swatch) return null;
  }
  try {
    swatch.clearRect(0, 0, 1, 1);
    swatch.fillStyle = '#000000';
    swatch.fillStyle = value;
    swatch.fillRect(0, 0, 1, 1);
    const [r, g, b, a] = swatch.getImageData(0, 0, 1, 1).data;
    if (a === 0) return null;
    return (r << 16) | (g << 8) | b;
  } catch {
    return null;
  }
}

/** '#rrggbb' | 'rgb(r, g, b)' | 'oklch(...)' | anything CSS -> 0xrrggbb. */
export function parseColor(value, fallbackHex = 0x808080) {
  if (typeof value !== 'string' || !value) return fallbackHex;
  const s = value.trim();
  if (s.startsWith('#')) {
    const hex = s.slice(1);
    if (hex.length === 3) {
      const r = hex[0];
      const g = hex[1];
      const b = hex[2];
      return parseInt(`${r}${r}${g}${g}${b}${b}`, 16);
    }
    if (hex.length >= 6) return parseInt(hex.slice(0, 6), 16);
    return fallbackHex;
  }
  const m = s.match(/rgba?\(\s*([\d.]+)[\s,]+([\d.]+)[\s,]+([\d.]+)/i);
  if (m) {
    const r = Math.round(Number(m[1])) & 0xff;
    const g = Math.round(Number(m[2])) & 0xff;
    const b = Math.round(Number(m[3])) & 0xff;
    return (r << 16) | (g << 8) | b;
  }
  // Everything else (oklch, lab, color(), a named colour) goes to the browser.
  const resolved = resolveViaCanvas(s);
  return resolved === null ? fallbackHex : resolved;
}

/** Linear blend of two packed colours, t in [0, 1]. */
export function mixColor(a, b, t) {
  const ar = (a >> 16) & 0xff;
  const ag = (a >> 8) & 0xff;
  const ab = a & 0xff;
  const br = (b >> 16) & 0xff;
  const bg = (b >> 8) & 0xff;
  const bb = b & 0xff;
  const r = Math.round(ar + (br - ar) * t) & 0xff;
  const g = Math.round(ag + (bg - ag) * t) & 0xff;
  const bl = Math.round(ab + (bb - ab) * t) & 0xff;
  return (r << 16) | (g << 8) | bl;
}

/**
 * Read the token set into plain numbers.
 * @param {HTMLElement} [root] element to read custom properties from
 */
export function readPalette(root = typeof document !== 'undefined' ? document.documentElement : null) {
  const cluster = CLUSTER_FAMILIES.map((family) =>
    parseColor(readToken(`${family}-500`, root)),
  );
  const clusterDeep = CLUSTER_FAMILIES.map((family) =>
    parseColor(readToken(`${family}-700`, root)),
  );
  const background = parseColor(readToken('neutral-0', root), 0xffffff);
  return {
    cluster,
    clusterDeep,
    background,
    ink: parseColor(readToken('neutral-900', root), 0x14181f),
    // Lines are context, nodes are the thing. On a corpus that is grey by rule
    // the two were the same token, so thirty thousand lines and four thousand
    // nodes competed for the same attention and the lines won on sheer count:
    // one grey mass. A weaker step on the same family keeps the distinction
    // inside the design system's own scale, which runs from the background (0)
    // to the ink (1000) in both light and dark mode.
    edge: parseColor(readToken('coolgray-300', root)),
    edgeTypes: [
      parseColor(readToken('coolgray-300', root)), // citation
      parseColor(readToken('coolgray-700', root)), // definition
      parseColor(readToken('violet-500', root)), // delegation
      parseColor(readToken('mintgroen-500', root)), // applicability
      parseColor(readToken('donkergeel-500', root)), // amendment
    ],
    // The grey pair is what the resting corpus is drawn in. -500 sits far
    // enough from a white background to keep a dense field readable, -700 marks
    // the framework laws inside that same neutral range.
    grey: parseColor(readToken('coolgray-500', root)),
    greyDeep: parseColor(readToken('coolgray-700', root)),
    active: parseColor(readToken('donkergeel-500', root)),
    selection: parseColor(readToken('donkergeel-500', root)),
    inbound: parseColor(readToken('rood-500', root)),
    outbound: parseColor(readToken('groen-500', root)),
  };
}

/**
 * sRGB -> linear for a single channel.
 *
 * three renders in a linear working space and converts to sRGB on output. A
 * `THREE.Color` built from a hex does that conversion itself, but raw numbers
 * written into a vertex or instance attribute do not: they are taken as linear
 * and converted a second time on output, which washes every colour out. The
 * two helpers below do the conversion once, in the right direction.
 */
export function srgbToLinearChannel(v) {
  return v <= 0.04045 ? v / 12.92 : Math.pow((v + 0.055) / 1.055, 2.4);
}

/** Packed sRGB colour -> three floats in linear space. */
export function colorToLinearRgb(c) {
  return [
    srgbToLinearChannel(((c >> 16) & 0xff) / 255),
    srgbToLinearChannel(((c >> 8) & 0xff) / 255),
    srgbToLinearChannel((c & 0xff) / 255),
  ];
}

/** Packed sRGB colour -> three bytes in linear space, for a normalised attribute. */
export function colorToLinearBytes(c) {
  return colorToLinearRgb(c).map((v) => Math.round(v * 255));
}

/**
 * Node colour.
 *
 * The rule is not "a hue per cluster". The rule is **grey is everything that
 * has only been harvested, colour means somebody has enriched this law**. Today
 * that makes the whole map grey with a handful of coloured nodes, and that is
 * the picture we want: it shows at a glance how little is done and where.
 *
 * So colour is spent on exactly one distinction first. Within the coloured set
 * the cluster hue comes back, because there colour is no longer scarce.
 *
 * - harvested  -> coolgray, slightly lifted off the background so four
 *                 thousand of them read as a field of nodes and not as one
 *                 cloud, with framework laws a shade darker.
 * - enriching  -> the attention colour: the enricher is in this law right now.
 * - enriched   -> the cluster hue at -500.
 * - validated  -> the same hue at -700, so the two are one family.
 */
export function nodeColor(palette, clusterIndex, status, framework = 0) {
  if (status === 1) return palette.cluster[clusterIndex % palette.cluster.length];
  if (status === 2) return palette.clusterDeep[clusterIndex % palette.clusterDeep.length];
  if (status === 3) return palette.active;
  return framework ? palette.greyDeep : palette.grey;
}
