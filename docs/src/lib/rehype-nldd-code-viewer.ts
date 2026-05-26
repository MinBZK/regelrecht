import { visit } from 'unist-util-visit';
import type { Root, Element, RootContent } from 'hast';

/* Collect raw text from a hast node. With Astro's syntaxHighlight disabled,
 * a fenced code block's <code> children are plain text nodes. */
function textContent(node: RootContent): string {
  if (node.type === 'text') return node.value;
  if ('children' in node) {
    return (node.children as RootContent[]).map(textContent).join('');
  }
  return '';
}

/**
 * Replace fenced code blocks (<pre><code class="language-X">…) with
 * <nldd-code-viewer language="X">…</nldd-code-viewer> so the design-system code component
 * owns the styling and (Prism) highlighting.
 *
 * Mermaid blocks are skipped — rehype-mermaid has already turned them into an
 * <svg> by the time this runs. Languages nldd-code-viewer doesn't (yet) support
 * render as plain code; the real language is still passed so highlighting
 * starts working automatically once the grammar is added upstream.
 */
export function rehypeNlddCodeViewer() {
  return (tree: Root) => {
    visit(tree, 'element', (node: Element, index: number | undefined, parent: Root | Element | null) => {
      if (node.tagName !== 'pre' || !parent || index === undefined) return;
      const code = node.children.find(
        (c): c is Element => c.type === 'element' && c.tagName === 'code',
      );
      if (!code) return;

      const classes = (code.properties && code.properties.className) || [];
      const langClass = (Array.isArray(classes) ? classes : []).find(
        (c: unknown) => typeof c === 'string' && c.startsWith('language-'),
      ) as string | undefined;
      const language = langClass ? langClass.slice('language-'.length) : '';
      if (language === 'mermaid') return;

      // Markdown adds a trailing newline to fenced blocks; drop it so the
      // code block has no empty last line.
      const raw = textContent(code).replace(/\n$/, '');

      parent.children[index] = {
        type: 'element',
        tagName: 'nldd-code-viewer',
        properties: language ? { language } : {},
        children: [{ type: 'text', value: raw }],
      };
    });
  };
}

export default rehypeNlddCodeViewer;
