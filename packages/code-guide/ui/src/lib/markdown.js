// Doc comments are Markdown written for rustdoc. This turns one into HTML that
// is safe to put in the page, and deals with the one rustdoc habit a plain
// Markdown renderer gets wrong: intra-doc links such as
// [`evaluate_law`](Self::evaluate_law), whose target is a Rust path, not a URL.

import { Marked } from 'marked';
import DOMPurify from 'dompurify';

const EXTERNAL = /^(https?:|mailto:)/i;

/**
 * Renders `markdown` to sanitised HTML.
 *
 * `resolve(path)` gets every link target that is not an external URL (a Rust
 * path like `Self::new` or `crate::error::EngineError`) and returns an address
 * in the app, or `null`. A link that resolves points there; one that does not
 * keeps its text and loses the link, so the reader never meets a dead link.
 * External links stay, and open in a new tab.
 */
export function renderDocs(markdown, { resolve = () => null } = {}) {
  if (!markdown) return '';
  const marked = new Marked({ gfm: true, breaks: false });
  marked.use({
    renderer: {
      link({ href, tokens }) {
        const text = this.parser.parseInline(tokens);
        if (EXTERNAL.test(href)) {
          return `<a href="${escapeAttr(href)}" target="_blank" rel="noopener noreferrer">${text}</a>`;
        }
        const target = resolve(href);
        return target ? `<a href="${escapeAttr(target)}">${text}</a>` : text;
      },
      // A doc comment's headings start at `#`; inside a method entry they must
      // not outrank the method's own name, so `# Arguments` becomes an h5.
      heading({ tokens, depth }) {
        const level = Math.min(6, depth + 4);
        return `<h${level}>${this.parser.parseInline(tokens)}</h${level}>\n`;
      },
    },
  });
  return DOMPurify.sanitize(marked.parse(markdown), { ADD_ATTR: ['target'] });
}

function escapeAttr(s) {
  return String(s).replace(/&/g, '&amp;').replace(/"/g, '&quot;').replace(/</g, '&lt;');
}

/**
 * The rustdoc paths the app can resolve from inside a type: `Self::x` and a
 * bare `x` that names one of the type's methods. Anything else is left
 * unresolved rather than guessed.
 */
export function selfResolver(methodNames, hrefForMethod) {
  const names = new Set(methodNames);
  return (path) => {
    const p = String(path).replace(/\(\)$/, '');
    const name = p.startsWith('Self::') ? p.slice('Self::'.length) : p;
    return names.has(name) ? hrefForMethod(name) : null;
  };
}
