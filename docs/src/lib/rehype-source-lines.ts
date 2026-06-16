import type { Root, Element, ElementContent } from 'hast';
import type { VFile } from 'vfile';
import { readFileSync } from 'node:fs';

/**
 * Source-line provenance for the select-lines-to-edit affordance.
 *
 * Markdown positions survive into the hast tree (Astro's remark pipeline
 * keeps `node.position`). This plugin stamps the source line range of each
 * block onto its rendered element as `data-line` / `data-line-end`, so a
 * client script can map a text selection back to source lines and build a
 * GitHub edit URL with a `#L<a>-L<b>` fragment.
 *
 * Frontmatter offset: Astro strips the YAML frontmatter ("remove" mode)
 * before remark parses the body, so `node.position` counts from the body, not
 * from line 1 of the file. How many lines the body is shifted depends on the
 * frontmatter length AND on how the tokenizer treats the blank line(s) after
 * the closing delimiter, so we don't compute it analytically. Instead we
 * anchor: read the raw file, find the file line of the first real body line,
 * and subtract the first stamped node's own (body-relative) start line. That
 * difference is the offset, self-correcting regardless of blank-line handling.
 *
 * Must run BEFORE the plugins that reshape nodes (rehype-nldd-code-viewer):
 * those replace elements and drop the original positions.
 *
 * Scope: top-level blocks (headings, paragraphs, lists, blockquotes, tables,
 * code) plus list items, the granularity a reader selects at.
 */

// Same shape Astro uses to detect/strip frontmatter (frontmatter.js).
const FRONTMATTER_RE = /(?:^﻿?|^\s*\n)(?:---|\+\+\+)([\s\S]*?\n)(?:---|\+\+\+)/;

/**
 * File line (1-based) of the first non-frontmatter, non-blank line. Lines
 * after the closing delimiter that are blank are skipped; the first content
 * line is what remark's first node maps to.
 */
function firstBodyFileLine(text: string): number | null {
  const match = FRONTMATTER_RE.exec(text);
  const lines = text.split('\n');
  // 0-based index of the line just past the frontmatter block (0 if none).
  let start = 0;
  if (match) {
    const blockEnd = (match.index ?? 0) + match[0].length;
    let nl = 0;
    for (let i = 0; i < blockEnd; i++) if (text[i] === '\n') nl++;
    // `nl` is the 0-based index of the closing delimiter line itself; the
    // body begins on the next line, so start scanning past it.
    start = nl + 1;
  }
  for (let i = start; i < lines.length; i++) {
    const line = lines[i].trim();
    if (line === '') continue;
    // .mdx: top-of-file import/export statements are consumed as ESM, not
    // rendered, so the first stamped block is the first prose line after them.
    // Skip them so the anchor pairs with the right line.
    if (/^(import|export)\b/.test(line)) continue;
    return i + 1; // 1-based file line
  }
  return null;
}

function computeOffset(filePath: string | undefined, firstNodeLine: number): number {
  if (!filePath) return 0;
  try {
    // file.path may be a file:// URL or a plain path depending on the loader.
    const target = filePath.startsWith('file:') ? new URL(filePath) : filePath;
    const text = readFileSync(target).toString('utf8');
    const bodyLine = firstBodyFileLine(text);
    if (bodyLine == null) return 0;
    // The first stamped node sits on `firstNodeLine` in the body; the same
    // content is `bodyLine` in the file. Their difference shifts everything.
    return bodyLine - firstNodeLine;
  } catch {
    return 0;
  }
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

export function rehypeSourceLines() {
  return (tree: Root, file: VFile) => {
    const blocks = tree.children.filter(
      (n): n is Element => n.type === 'element' && !!n.position?.start?.line,
    );
    if (blocks.length === 0) return;
    const offset = computeOffset(file?.path, blocks[0].position!.start.line);
    for (const node of blocks) {
      stamp(node, offset);
      // List items are the natural selection unit inside ul/ol; stamp them too.
      if ((node.tagName === 'ul' || node.tagName === 'ol') && node.children) {
        walkListItems(node.children, offset);
      }
    }
  };
}

export default rehypeSourceLines;
