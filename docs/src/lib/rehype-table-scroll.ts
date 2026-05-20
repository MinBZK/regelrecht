import type { Root, Element, RootContent } from 'hast';

/**
 * Wide markdown tables overflow horizontally. The overflow makes the element a
 * scrollable region, and a scrollable region must be reachable by keyboard
 * (WCAG 2.1.1 / axe scrollable-region-focusable) — otherwise a keyboard-only
 * user can't pan a table wider than the viewport.
 *
 * This plugin wraps every <table> in a focusable, labelled scroll container:
 *   <div class="rr-table-scroll" tabindex="0" role="region"
 *        aria-label="<caption | nearest heading | 'Table'> (scrollable)">
 *     <table>…</table>
 *   </div>
 * The horizontal scrolling itself is set in docs.css (.rr-table-scroll); here we
 * only add the wrapper, the tabindex, and an accessible name so a screen reader
 * announces the region rather than an anonymous scrollable box.
 */

const HEADING_TAGS = new Set(['h1', 'h2', 'h3', 'h4', 'h5', 'h6']);

function textOf(node: RootContent): string {
  if (node.type === 'text') return node.value;
  if (node.type === 'element') return (node.children ?? []).map(textOf).join('');
  return '';
}

/** A table's own <caption>, if it has one — the best label. */
function captionOf(table: Element): string {
  const caption = (table.children ?? []).find(
    (c): c is Element => c.type === 'element' && c.tagName === 'caption',
  );
  return caption ? textOf(caption).trim() : '';
}

export function rehypeTableScroll() {
  return (tree: Root) => {
    let lastHeading = '';

    const walk = (parent: { children: RootContent[] }) => {
      const children = parent.children;
      for (let i = 0; i < children.length; i++) {
        const node = children[i];
        if (node.type !== 'element') continue;
        const el = node as Element;

        if (HEADING_TAGS.has(el.tagName)) {
          lastHeading = (el.children ?? []).map(textOf).join('').trim();
        }

        if (el.tagName === 'table') {
          const label = captionOf(el) || lastHeading || 'Table';
          const wrapper: Element = {
            type: 'element',
            tagName: 'div',
            properties: {
              className: ['rr-table-scroll'],
              tabIndex: 0,
              role: 'region',
              ariaLabel: `${label} (scrollable)`,
            },
            children: [el],
          };
          children[i] = wrapper;
          // The table is now wrapped; don't descend into it again.
          continue;
        }

        if (el.children?.length) {
          walk(el as { children: RootContent[] });
        }
      }
    };

    walk(tree as unknown as { children: RootContent[] });
  };
}

export default rehypeTableScroll;
