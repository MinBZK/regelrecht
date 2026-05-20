import type { Root, Element, RootContent } from 'hast';

/**
 * rehype-mermaid renders each ```mermaid block to an <img src="data:image/svg…">
 * with an empty `alt`, which is a WCAG 1.1.1 failure for a meaningful diagram:
 * the SVG's own <title>/<desc> live inside the data URI and are not exposed to
 * assistive tech. This plugin runs AFTER rehype-mermaid and gives every such
 * <img> a descriptive `alt` derived from the nearest preceding heading (the
 * section that introduces the diagram), falling back to the mermaid diagram
 * type. It also sets role="img" so the element is announced as an image even
 * where the data-URI source confuses heuristics.
 *
 * Authors can override per diagram by setting an `accTitle:` line in the
 * mermaid source (mermaid's native accessible-title directive); when present
 * we prefer it over the heading.
 */

const HEADING_TAGS = new Set(['h1', 'h2', 'h3', 'h4', 'h5', 'h6']);

function textOf(node: RootContent): string {
  if (node.type === 'text') return node.value;
  if (node.type === 'element') {
    return (node.children ?? []).map(textOf).join('');
  }
  return '';
}

function isMermaidImg(node: Element): boolean {
  if (node.tagName !== 'img') return false;
  const id = String(node.properties?.id ?? '');
  const src = String(node.properties?.src ?? '');
  return id.startsWith('mermaid') || /class=['"]?flowchart|aria-roledescription/.test(src);
}

/** Pull `accTitle: ...` out of the SVG data URI if the author set one. */
function accTitleFromSrc(src: string): string | null {
  const decoded = (() => {
    try {
      return decodeURIComponent(src);
    } catch {
      return src;
    }
  })();
  const m = decoded.match(/accTitle\s*[:=]\s*([^\n;"']+)/i);
  return m ? m[1].trim() : null;
}

/** Best-effort diagram type for the fallback alt. */
function diagramTypeFromSrc(src: string): string {
  if (/sequenceDiagram/.test(src)) return 'sequence diagram';
  if (/stateDiagram/.test(src)) return 'state diagram';
  if (/classDiagram/.test(src)) return 'class diagram';
  if (/erDiagram/.test(src)) return 'entity-relationship diagram';
  if (/gantt/.test(src)) return 'gantt chart';
  return 'flowchart diagram';
}

export function rehypeMermaidAlt() {
  return (tree: Root) => {
    // Track the most recent heading text as we walk the document in order.
    let lastHeading = '';

    const walk = (nodes: RootContent[]) => {
      for (const node of nodes) {
        if (node.type !== 'element') continue;
        const el = node as Element;

        if (HEADING_TAGS.has(el.tagName)) {
          lastHeading = (el.children ?? []).map(textOf).join('').trim();
        }

        if (isMermaidImg(el)) {
          const src = String(el.properties?.src ?? '');
          const acc = accTitleFromSrc(src);
          const heading = lastHeading;
          const type = diagramTypeFromSrc(src);
          const alt = acc
            ? acc
            : heading
              ? `${type}: ${heading}`
              : type;
          el.properties = el.properties ?? {};
          el.properties.alt = alt;
          el.properties.role = 'img';
        }

        if (el.children?.length) {
          walk(el.children as RootContent[]);
        }
      }
    };

    walk(tree.children as RootContent[]);
  };
}

export default rehypeMermaidAlt;
