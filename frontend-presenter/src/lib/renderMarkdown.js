/**
 * Markdown → the pieces a slide renders. Same pipeline as the editor's article
 * text (frontend/src/composables/useArticleMarkdown.js): marked, then DOMPurify,
 * because a slide can hold any HTML its author pasted in.
 *
 * Code blocks in the languages `wet`, `reken` and `mermaid` are not rendered as
 * code: they become a `{ kind: 'wet' | 'reken' | 'mermaid' }` piece that Slide.vue replaces with
 * a component. That is why rendering goes per block and not per document: a Vue
 * component cannot sit inside a v-html string.
 */
import { marked } from 'marked';
import DOMPurify from 'dompurify';
import * as yaml from 'js-yaml';

export const COMPONENT_LANGS = new Set(['wet', 'reken', 'mermaid']);

export function sanitize(html) {
  return DOMPurify.sanitize(html);
}

/** One block (from slideDoc.bodyBlocks) → a renderable piece. */
export function renderBlock(block) {
  const { token } = block;
  if (token.type === 'code' && COMPONENT_LANGS.has(token.lang)) {
    if (token.lang === 'mermaid') return { kind: 'mermaid', source: token.text };
    if (token.lang === 'reken') {
      try {
        const spec = yaml.load(token.text) ?? {};
        return { kind: 'reken', spec, error: spec.scenario ? null : 'Een reken-blok heeft `scenario:` nodig: wet/bestand.feature, of een .feature in de deck-map.' };
      } catch (e) {
        return { kind: 'reken', spec: null, error: `Ongeldige YAML in reken-blok: ${e.reason ?? e.message}` };
      }
    }
    let spec = null;
    let error = null;
    try {
      spec = yaml.load(token.text) ?? {};
      if (!spec.law && !spec.bestand) error = 'Een wet-blok heeft `law:` (uit de corpus) of `bestand:` (uit de deck-map) nodig.';
    } catch (e) {
      error = `Ongeldige YAML in wet-blok: ${e.reason ?? e.message}`;
    }
    return { kind: 'wet', spec, error };
  }
  const tokens = Object.assign([token], { links: block.links ?? {} });
  return { kind: 'html', html: sanitize(marked.parser(tokens)) };
}

/** Inline markdown (a title, a presenter line) → sanitised HTML without a <p>. */
export function renderInline(text) {
  return sanitize(marked.parseInline(String(text ?? '')));
}

/** Markdown → sanitised HTML, for article text inside a wet block. */
export function renderHtml(text) {
  return text ? sanitize(marked.parse(String(text))) : '';
}
