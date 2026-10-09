/**
 * Any CSS colour as a plain `rgb(r, g, b)` string, or null.
 *
 * The browser reports a computed colour in the colour space it was written in.
 * The design system writes its primitives in oklch, so a computed token comes
 * back as `oklch(1 0 0)`, and mermaid's colour parser (khroma) only reads hex,
 * rgb and hsl: one such value in the theme makes every diagram fail with
 * "Unsupported color format". Painting the colour on a 1×1 canvas and reading
 * the pixel back converts any colour the browser understands to sRGB.
 *
 * Returns null when there is no canvas (a test DOM) or when the colour did not
 * paint (invalid, or fully transparent), so the caller can fall back.
 */
export function toRgb(css) {
  const ctx = document.createElement('canvas').getContext?.('2d', { willReadFrequently: true });
  if (!ctx) return null;
  ctx.clearRect(0, 0, 1, 1);
  // An invalid colour is ignored by the canvas and the previous fillStyle stays;
  // starting from transparent makes that case paint nothing instead.
  ctx.fillStyle = 'transparent';
  ctx.fillStyle = css;
  ctx.fillRect(0, 0, 1, 1);
  const [r, g, b, a] = ctx.getImageData(0, 0, 1, 1).data;
  return a ? `rgb(${r}, ${g}, ${b})` : null;
}
