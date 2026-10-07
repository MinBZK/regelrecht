// The design system's colours as plain values the graph canvas can use.
//
// Its tokens are CSS custom properties written as `light-dark(oklch(…), …)`.
// Cytoscape draws on a canvas and parses neither, so each token is applied to
// a probe element (the browser resolves `var()` and `light-dark()` for the
// current colour scheme) and the result is painted on a one-pixel canvas and
// read back as `rgb()`.

import { CATEGORIES } from './callgraph.js';
import { FALLBACK_THEME } from './cy.js';

export { CATEGORIES };

export function resolveTheme(root = document.body) {
  const probe = document.createElement('span');
  probe.style.display = 'none';
  root.appendChild(probe);
  const canvas = document.createElement('canvas');
  canvas.width = 1;
  canvas.height = 1;
  const ctx = canvas.getContext('2d', { willReadFrequently: true });
  const color = (token, fallback) => {
    probe.style.color = '';
    probe.style.color = `var(${token})`;
    const css = getComputedStyle(probe).color;
    if (!ctx || !css) return fallback;
    ctx.clearRect(0, 0, 1, 1);
    ctx.fillStyle = '#000';
    ctx.fillStyle = css;
    ctx.fillRect(0, 0, 1, 1);
    const [r, g, b, a] = ctx.getImageData(0, 0, 1, 1).data;
    return a === 0 ? fallback : `rgb(${r}, ${g}, ${b})`;
  };
  const f = FALLBACK_THEME;
  try {
    return {
      font: getComputedStyle(root).fontFamily || f.font,
      content: color('--semantics-content-color', f.content),
      secondary: color('--semantics-content-secondary-color', f.secondary),
      surface: color('--semantics-surfaces-base-background-color', f.surface),
      divider: color('--semantics-dividers-color', f.divider),
      edge: color('--semantics-content-secondary-color', f.edge),
      accent: color('--semantics-content-accent-color', f.accent),
      incoming: color('--semantics-categories-oranje-tinted-content-color', f.incoming),
      outgoing: color('--semantics-categories-groen-tinted-content-color', f.outgoing),
      categories: CATEGORIES.map((c) => ({
        background: color(`--semantics-categories-${c}-tinted-background-color`, f.categories[0].background),
        border: color(`--semantics-categories-${c}-tinted-highlight-border-color`, f.categories[0].border),
        text: color(`--semantics-categories-${c}-tinted-content-color`, f.categories[0].text),
      })),
    };
  } finally {
    probe.remove();
  }
}
