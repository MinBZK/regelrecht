import type { Root, Element, RootContent, ElementContent, Text } from 'hast';

/**
 * RFC preamble styling.
 *
 * Every RFC body opens with a paragraph of bold-labelled metadata:
 *
 *   <p><strong>Status:</strong> Accepted
 *      <strong>Date:</strong> 2026-05-29
 *      <strong>Authors:</strong> regelrecht team</p>
 *
 * Rendered as-is that is one grey run-on line. This plugin recognises that
 * first paragraph and rebuilds it as a small metadata header: the Status
 * value becomes an <nldd-tag> coloured by status (same mapping as the RFC
 * index), and the remaining fields render as a muted line beneath it.
 *
 * It only fires on a paragraph whose first child is <strong>Status:</strong>,
 * so ordinary prose is never touched. RFCs that omit the preamble render
 * unchanged.
 */

const STATUS_COLOR: Record<string, string> = {
  accept: 'success',
  propos: 'accent',
  reject: 'critical',
  supersed: 'warning',
};

function statusColor(status: string): string {
  const k = status.toLowerCase();
  for (const key of Object.keys(STATUS_COLOR)) {
    if (k.includes(key)) return STATUS_COLOR[key];
  }
  return 'neutral';
}

function textOf(node: RootContent): string {
  if (node.type === 'text') return node.value;
  if ('children' in node) {
    return (node.children as RootContent[]).map(textOf).join('');
  }
  return '';
}

/** Read a paragraph of `<strong>Label:</strong> value` pairs into a map. */
function parseFields(p: Element): Record<string, string> | null {
  const fields: Record<string, string> = {};
  let currentLabel: string | null = null;
  for (const child of p.children) {
    if (child.type === 'element' && child.tagName === 'strong') {
      currentLabel = textOf(child).replace(/:\s*$/, '').trim();
    } else if (child.type === 'text' && currentLabel) {
      const value = child.value.trim();
      if (value) {
        fields[currentLabel] = value;
        currentLabel = null;
      }
    }
  }
  return Object.keys(fields).length ? fields : null;
}

function tag(color: string, label: string): Element {
  return {
    type: 'element',
    tagName: 'nldd-tag',
    properties: { color, size: 'md' },
    children: [{ type: 'text', value: label } as Text],
  };
}

function mutedLine(text: string): Element {
  return {
    type: 'element',
    tagName: 'span',
    properties: { className: ['rr-rfc-meta-fields'] },
    children: [{ type: 'text', value: text } as Text],
  };
}

export function rehypeRfcMeta() {
  return (tree: Root) => {
    const firstP = tree.children.find(
      (n): n is Element => n.type === 'element' && n.tagName === 'p',
    );
    if (!firstP) return;
    const first = firstP.children[0];
    const startsWithStatus =
      first?.type === 'element' &&
      first.tagName === 'strong' &&
      /^Status:/.test(textOf(first));
    if (!startsWithStatus) return;

    const fields = parseFields(firstP);
    if (!fields || !fields.Status) return;

    // Status as a coloured tag; the rest (Date, Authors, …) as a muted line,
    // skipping Short title which is sidebar-only metadata.
    const rest = Object.entries(fields)
      .filter(([k]) => k !== 'Status' && k !== 'Short title')
      .map(([k, v]) => `${k}: ${v}`)
      .join('  ·  ');

    const children: ElementContent[] = [tag(statusColor(fields.Status), fields.Status)];
    if (rest) children.push(mutedLine(rest));

    const replacement: Element = {
      type: 'element',
      tagName: 'div',
      properties: { className: ['rr-rfc-meta'] },
      children,
    };

    const idx = tree.children.indexOf(firstP);
    tree.children[idx] = replacement;
  };
}

export default rehypeRfcMeta;
