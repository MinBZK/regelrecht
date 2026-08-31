/*
 * Markdown for the roadmap's prose fields (toelichting, vision, mission).
 *
 * These are short strings inside YAML/JSON, not page content, so they do not
 * go through Astro's markdown pipeline: that one stamps source lines, autolinks
 * "RFC-008", turns fenced blocks into <nldd-code-viewer> and boots a headless
 * Chromium for mermaid — all wrong for a paragraph of prose in a frontmatter
 * field. This is the same unified stack, minus everything page-specific.
 *
 * The app rendered these client-side with marked + DOMPurify. Build-time
 * rendering makes both unnecessary: the input is repo-committed and reviewed
 * like every other file, and raw HTML in the source stays inert because
 * rehype-raw is deliberately absent.
 */
import { unified } from 'unified';
import remarkParse from 'remark-parse';
import remarkGfm from 'remark-gfm';
import remarkRehype from 'remark-rehype';
import rehypeStringify from 'rehype-stringify';

const processor = unified()
  .use(remarkParse)
  .use(remarkGfm)
  .use(remarkRehype)
  .use(rehypeStringify);

/** Render a markdown field to HTML. Empty input yields an empty string. */
export function renderMarkdown(value: string | undefined): string {
  if (!value || !value.trim()) return '';
  return String(processor.processSync(value));
}
