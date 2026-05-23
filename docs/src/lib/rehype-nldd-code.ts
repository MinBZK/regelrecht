import { visit } from 'unist-util-visit';

/* Collect raw text from a hast node. With Astro's syntaxHighlight disabled,
 * a fenced code block's <code> children are plain text nodes. */
function textContent(node: any): string {
  if (node.type === 'text') return node.value as string;
  if (node.children) return node.children.map(textContent).join('');
  return '';
}

/**
 * Replace fenced code blocks (<pre><code class="language-X">…) with
 * <nldd-code language="X">…</nldd-code> so the design-system code component
 * owns the styling and (Prism) highlighting.
 *
 * Mermaid blocks are skipped — rehype-mermaid has already turned them into an
 * <svg> by the time this runs. Languages nldd-code doesn't (yet) support
 * render as plain code; the real language is still passed so highlighting
 * starts working automatically once the grammar is added upstream.
 */
export function rehypeNlddCode() {
  return (tree: any) => {
    visit(tree, 'element', (node: any, index: number | undefined, parent: any) => {
      if (node.tagName !== 'pre' || !parent || index === undefined) return;
      const code = node.children.find(
        (c: any) => c.type === 'element' && c.tagName === 'code',
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
        tagName: 'nldd-code',
        properties: language ? { language } : {},
        children: [{ type: 'text', value: raw }],
      };
    });
  };
}

export default rehypeNlddCode;
