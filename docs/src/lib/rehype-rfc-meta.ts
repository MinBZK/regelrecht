import type { Root, Element, RootContent } from 'hast';

/**
 * Strip the RFC preamble paragraph from the body.
 *
 * Every RFC body opens with a paragraph of bold-labelled metadata:
 *
 *   <p><strong>Status:</strong> Accepted
 *      <strong>Date:</strong> 2026-05-29
 *      <strong>Authors:</strong> regelrecht team</p>
 *
 * The RFC page template (pages/rfcs/[slug].astro) renders this metadata in the
 * section header — Status as a coloured <nldd-tag>, the rest as a line beneath
 * the title — so this plugin removes the paragraph from the body to avoid
 * showing it twice. It only fires on a first paragraph whose first child is
 * <strong>Status:…</strong>, so ordinary prose is untouched and RFCs that omit
 * the preamble render unchanged.
 */
function textOf(node: RootContent): string {
  if (node.type === 'text') return node.value;
  if ('children' in node) {
    return (node.children as RootContent[]).map(textOf).join('');
  }
  return '';
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

    tree.children.splice(tree.children.indexOf(firstP), 1);
  };
}

export default rehypeRfcMeta;
