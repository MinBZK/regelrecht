import type { Root, Element, RootContent } from 'hast';

/**
 * Wide markdown tables overflow horizontally. The overflow makes the element a
 * scrollable region, and a scrollable region must be reachable by keyboard
 * (WCAG 2.1.1 / axe scrollable-region-focusable) — otherwise a keyboard-only
 * user can't pan a table wider than the viewport.
 *
 * This plugin wraps every <table> in a scroll container that is focusable:
 *   <div class="rr-table-scroll" tabindex="0"><table>…</table></div>
 * The horizontal overflow lives on the wrapper (docs.css .rr-table-scroll), so
 * the focusable element is what scrolls. The axe rule only fires when there is
 * actual overflow and only asks for keyboard reachability, so `tabindex="0"`
 * is enough. We deliberately do NOT add role="region" + aria-label: that would
 * announce every narrow, non-scrolling table as a named region — noise for
 * screen-reader users — and the rule does not require it. A non-overflowing
 * wrapped table is then just a harmless extra tab stop.
 *
 * This is the same wrapper the RfcIndexTable component applies by hand, so the
 * one hand-built wide table on the site is covered by the same mechanism.
 */

export function rehypeTableScroll() {
  return (tree: Root) => {
    const walk = (parent: { children: RootContent[] }) => {
      const children = parent.children;
      for (let i = 0; i < children.length; i++) {
        const node = children[i];
        if (node.type !== 'element') continue;
        const el = node as Element;

        if (el.tagName === 'table') {
          const wrapper: Element = {
            type: 'element',
            tagName: 'div',
            properties: {
              className: ['rr-table-scroll'],
              tabIndex: 0,
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
