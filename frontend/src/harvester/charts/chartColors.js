/**
 * Resolve the NLDD design tokens the charts need to concrete rgb() strings.
 *
 * ECharts renders to canvas, so it can't consume CSS custom properties or
 * light-dark() directly. Each token is resolved by computing it on a probe
 * element and normalizing through a 1×1 canvas — which also converts oklch()
 * (the NLDD primitive format, unsupported by zrender) to plain rgb.
 */

// succeeded/failed use the same expressions as the .status-bar__segment--*
// rules in harvester.css, so the charts stay color-consistent with the rest
// of the dashboard. added is hemelblauw rather than the accent: the accent
// blue fails the chart-palette checks (chroma floor, dark-mode contrast),
// hemelblauw is the nearest passing NLDD hue.
const TOKEN_EXPRESSIONS = {
  succeeded:
    'light-dark(var(--primitives-color-success-450), var(--primitives-color-success-550))',
  failed: 'var(--primitives-color-critical-450)',
  added:
    'light-dark(var(--primitives-color-hemelblauw-500), var(--primitives-color-hemelblauw-550))',
  text: 'var(--semantics-content-color)',
  textSecondary: 'var(--semantics-content-secondary-color)',
  grid: 'light-dark(var(--primitives-color-neutral-150), var(--primitives-color-neutral-250))',
  surface: 'var(--semantics-surfaces-background-color)',
};

export function resolveChartColors(referenceEl = document.body) {
  const probe = document.createElement('span');
  probe.style.display = 'none';
  referenceEl.appendChild(probe);

  const canvas = document.createElement('canvas');
  canvas.width = 1;
  canvas.height = 1;
  const ctx = canvas.getContext('2d', { willReadFrequently: true });

  const colors = {};
  try {
    for (const [key, expression] of Object.entries(TOKEN_EXPRESSIONS)) {
      probe.style.color = expression;
      const resolved = getComputedStyle(probe).color;
      if (!ctx) {
        // No 2D context (test environments): fall back to the computed string.
        colors[key] = resolved;
        continue;
      }
      ctx.clearRect(0, 0, 1, 1);
      // Canvas silently keeps the previous fillStyle on a parse failure; seed
      // a sentinel so an unparseable value falls back to the computed string
      // instead of leaking the previous token's color.
      ctx.fillStyle = '#010203';
      ctx.fillStyle = resolved;
      if (ctx.fillStyle === '#010203' && resolved !== '#010203') {
        colors[key] = resolved;
        continue;
      }
      ctx.fillRect(0, 0, 1, 1);
      const [r, g, b] = ctx.getImageData(0, 0, 1, 1).data;
      colors[key] = `rgb(${r}, ${g}, ${b})`;
    }
  } finally {
    probe.remove();
  }
  return colors;
}
