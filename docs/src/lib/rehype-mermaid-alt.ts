import type { Root, Element, RootContent } from 'hast';

/**
 * rehype-mermaid (strategy 'inline-svg') renders each ```mermaid block to an
 * inline <svg>. Mermaid gives it role="graphics-document" + an
 * aria-roledescription, but no descriptive NAME, so a screen reader announces
 * only "flowchart-v2". This plugin runs AFTER rehype-mermaid and gives every
 * mermaid <svg> an accessible name (aria-label) derived from the nearest
 * preceding heading — the section that introduces the diagram — prefixed with
 * the diagram type. WCAG 1.1.1 / name-role-value.
 */

const HEADING_TAGS = new Set(['h1', 'h2', 'h3', 'h4', 'h5', 'h6']);

function textOf(node: RootContent): string {
  if (node.type === 'text') return node.value;
  if (node.type === 'element') {
    return (node.children ?? []).map(textOf).join('');
  }
  return '';
}

function isMermaidSvg(node: Element): boolean {
  if (node.tagName !== 'svg') return false;
  const id = String(node.properties?.id ?? '');
  const cls = node.properties?.className;
  const classStr = Array.isArray(cls) ? cls.join(' ') : String(cls ?? '');
  const role = String(node.properties?.role ?? '');
  return (
    id.startsWith('mermaid') ||
    /flowchart/.test(classStr) ||
    role.includes('graphics-document')
  );
}

/** Best-effort diagram type from the svg's aria-roledescription/class. */
function diagramType(el: Element): string {
  const desc = String(el.properties?.ariaRoledescription ?? '');
  const cls = el.properties?.className;
  const classStr = Array.isArray(cls) ? cls.join(' ') : String(cls ?? '');
  const hay = `${desc} ${classStr}`;
  if (/sequence/.test(hay)) return 'sequence diagram';
  if (/state/.test(hay)) return 'state diagram';
  if (/class/.test(hay)) return 'class diagram';
  if (/er(Diagram)?/.test(hay)) return 'entity-relationship diagram';
  if (/gantt/.test(hay)) return 'gantt chart';
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

        if (isMermaidSvg(el)) {
          const type = diagramType(el);
          const label = lastHeading ? `${type}: ${lastHeading}` : type;
          el.properties = el.properties ?? {};
          el.properties.ariaLabel = label;
          el.properties.role = 'img';
          // Don't descend into the SVG internals.
          continue;
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
