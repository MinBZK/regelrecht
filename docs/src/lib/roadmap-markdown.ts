/*
 * Markdown for the roadmap's prose fields (toelichting, vision, mission).
 *
 * These are short strings inside YAML/JSON, not page content, so they do not
 * go through Astro's markdown pipeline: that one autolinks "RFC-008", turns
 * fenced blocks into <nldd-code-viewer> and boots a headless Chromium for
 * mermaid — all wrong for a paragraph of prose in a frontmatter field. This is
 * the same unified stack, minus everything page-specific.
 *
 * The app rendered these client-side with marked + DOMPurify. Build-time
 * rendering makes both unnecessary: the input is repo-committed and reviewed
 * like every other file, and raw HTML in the source stays inert because
 * rehype-raw is deliberately absent.
 *
 * What it does keep is the source-line provenance the rest of the site has, so
 * selecting text in a toelichting offers the same "edit these lines on GitHub"
 * button as a docs page. The offset differs from rehypeSourceLines': that one
 * anchors on the first body line after the frontmatter, and a toelichting
 * lives *inside* the frontmatter, so the anchor is the field's own first
 * content line.
 */
import { readFileSync } from 'node:fs';
import { unified } from 'unified';
import remarkParse from 'remark-parse';
import remarkGfm from 'remark-gfm';
import remarkRehype from 'remark-rehype';
import rehypeStringify from 'rehype-stringify';
import type { Root, Element, ElementContent } from 'hast';

/**
 * File line (1-based) where a block scalar's content starts, i.e. the line
 * after `<field>: |-` / `>-`. Returns null when the field is absent or is a
 * plain inline scalar, in which case there is nothing useful to stamp.
 */
function blockScalarFirstLine(text: string, field: string): number | null {
  const lines = text.split('\n');
  const header = new RegExp(`^${field}:\\s*[|>][-+]?\\s*$`);
  for (let i = 0; i < lines.length; i++) {
    if (header.test(lines[i])) return i + 2; // 1-based, line after the header
  }
  return null;
}

function stamp(node: Element, offset: number): void {
  const pos = node.position;
  if (!pos?.start?.line || !pos?.end?.line) return;
  node.properties = node.properties ?? {};
  node.properties['data-line'] = pos.start.line + offset;
  node.properties['data-line-end'] = pos.end.line + offset;
}

function walkListItems(children: ElementContent[], offset: number): void {
  for (const child of children) {
    if (child.type !== 'element') continue;
    if (child.tagName === 'li') stamp(child, offset);
    if (child.children) walkListItems(child.children, offset);
  }
}

/** Stamp data-line/-end, shifted so the numbers point at lines in the file. */
function rehypeFieldSourceLines(options: { offset: number }) {
  return (tree: Root) => {
    for (const node of tree.children) {
      if (node.type !== 'element') continue;
      stamp(node, options.offset);
      if ((node.tagName === 'ul' || node.tagName === 'ol') && node.children) {
        walkListItems(node.children, options.offset);
      }
    }
  };
}

const base = () =>
  unified().use(remarkParse).use(remarkGfm).use(remarkRehype);

const plain = base().use(rehypeStringify);

/**
 * Render a markdown field to HTML. Empty input yields an empty string.
 *
 * Pass `source` (the file the field lives in) and `field` (its YAML key) to
 * get source-line attributes, which turn on the select-to-edit affordance.
 * Without them the markdown still renders, just without provenance.
 */
export function renderMarkdown(
  value: string | undefined,
  source?: { filePath: string; field: string },
): string {
  if (!value || !value.trim()) return '';
  if (!source) return String(plain.processSync(value));

  let offset = 0;
  try {
    const text = readFileSync(source.filePath, 'utf8');
    const firstLine = blockScalarFirstLine(text, source.field);
    // remark counts the field's content from 1; the same content sits on
    // `firstLine` in the file, so everything shifts by their difference.
    if (firstLine != null) offset = firstLine - 1;
  } catch {
    // An unreadable file only costs the edit affordance, not the render.
  }

  const processor = base()
    .use(rehypeFieldSourceLines, { offset })
    .use(rehypeStringify);
  return String(processor.processSync(value));
}
