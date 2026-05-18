/**
 * Shared article-text markdown pipeline.
 *
 * ArticleText.vue (Tekst pane, notes off) and AnnotatedText.vue (Tekst pane,
 * notes on) must render the law text identically — same list nesting, same
 * paragraph breaks — so toggling Notities does not visually reflow the text.
 * Keeping the marked + DOMPurify config in one place is the only way to
 * guarantee that; a drift here is exactly the "duidelijke stap terug"
 * regression #646 is about.
 *
 * marked v18 no longer sanitizes HTML in Markdown by default; harvested laws
 * could in principle carry arbitrary HTML, so the output is always run through
 * DOMPurify before it reaches the DOM (v-html or manual parsing).
 */
import { marked } from 'marked';
import DOMPurify from 'dompurify';

/** Raw article text -> sanitized HTML string. */
export function renderArticleHtml(text) {
  if (!text) return '';
  return DOMPurify.sanitize(marked.parse(text));
}
