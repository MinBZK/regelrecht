/**
 * Shared RFC status helpers, used by both the RFC index (rfcs/index.astro) and
 * the individual RFC page (rfcs/[slug].astro) so the tag colour and label stay
 * in sync.
 */
const STATUS_COLOR: Record<string, string> = {
  accept: 'success',
  propos: 'accent',
  reject: 'critical',
  supersed: 'warning',
};

/** Map an RFC status string to an nldd-tag colour. */
export function statusColor(status: string): string {
  const k = (status ?? '').toLowerCase();
  for (const key of Object.keys(STATUS_COLOR)) {
    if (k.includes(key)) return STATUS_COLOR[key];
  }
  return 'neutral';
}

/**
 * Display label for the status tag: "Accepted (implemented)" reads better as
 * just "Implemented" (the colour still derives from the underlying status).
 */
export function statusLabel(status: string): string {
  return /\(implemented\)/i.test(status ?? '') ? 'Implemented' : status;
}
