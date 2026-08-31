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
 * The frontmatter's lines, padded so indexes stay file line numbers.
 *
 * Both parsers below look for a key at column 0, and outside the frontmatter
 * that is just prose: a body paragraph starting "onderzoeksvragen:" would be
 * read as the field itself. Werkpakket files carry no body today, so this is
 * a guard rather than a fix, but the alternative is a wrong edit link that
 * nothing would catch.
 */
function frontmatterLines(text: string): string[] {
  const lines = text.split(/\r?\n/);
  if (lines[0]?.trim() !== '---') return [];
  const end = lines.findIndex((l, i) => i > 0 && l.trim() === '---');
  if (end === -1) return [];
  // Keep the array as long as the region so an index is still a file line.
  return lines.slice(0, end);
}

/**
 * The file line range (1-based, inclusive) a field's value occupies.
 *
 * Only a LITERAL block scalar (`|`) keeps one source line per rendered line,
 * so only there can a paragraph be mapped back to its own lines. A folded
 * scalar (`>`) joins several source lines into one logical line and a plain
 * or quoted scalar has no line structure at all; for those, per-paragraph
 * arithmetic silently drifts — in one werkpakket the third paragraph came out
 * four lines above where it actually sits. So they report the whole field
 * instead: the edit link then opens the right region every time, which is the
 * honest answer.
 */
function fieldLines(
  text: string,
  field: string,
): { first: number; last: number; perLine: boolean } | null {
  const lines = frontmatterLines(text);
  const header = new RegExp(`^${field}:(.*)$`);
  for (let i = 0; i < lines.length; i++) {
    const m = header.exec(lines[i]);
    if (!m) continue;

    const rest = m[1].trim();
    const literal = /^\|[-+]?$/.test(rest);
    const folded = /^>[-+]?$/.test(rest);

    // A value on the header line itself (plain or quoted) occupies that line.
    if (!literal && !folded) return { first: i + 1, last: i + 1, perLine: false };

    // Block scalar: its content runs until the next line at column 0.
    let last = i + 1;
    for (let j = i + 1; j < lines.length; j++) {
      if (lines[j].trim() !== '' && !/^\s/.test(lines[j])) break;
      last = j + 1;
    }
    return { first: i + 2, last, perLine: literal };
  }
  return null;
}

/**
 * Line ranges (1-based, inclusive) of the items of a YAML sequence field.
 *
 * The onderzoeksvragen are a list in the frontmatter, not markdown, so they
 * never pass through remark and have no positions of their own. Reading the
 * file gives each item its range, which is what the select-to-edit affordance
 * needs to build a `#L<a>-L<b>` link. Returns [] when the field is absent or
 * is not a sequence (`onderzoeksvragen: []`).
 */
export function yamlSequenceItemLines(
  filePath: string,
  field: string,
): [number, number][] {
  let lines: string[];
  try {
    lines = frontmatterLines(readFileSync(filePath, 'utf8'));
  } catch {
    return [];
  }

  const header = lines.findIndex((l) => new RegExp(`^${field}:\\s*$`).test(l));
  if (header === -1) return [];

  const ranges: [number, number][] = [];
  let start = -1;
  // The indent of the first "- " fixes what counts as an item. Without it a
  // deeper "- " inside an item's own text — a markdown bullet in a block
  // scalar, say — would start a phantom item and shift every range after it.
  let itemIndent: number | null = null;

  const isItem = (line: string): boolean => {
    const m = /^(\s+)-(\s|$)/.exec(line);
    if (!m) return false;
    const indent = m[1].length;
    if (itemIndent === null) {
      itemIndent = indent;
      return true;
    }
    return indent === itemIndent;
  };

  let end = lines.length;
  for (let i = header + 1; i < lines.length; i++) {
    const line = lines[i];
    // A non-blank line at column 0 ends the sequence: it is the next key.
    if (line.trim() !== '' && !/^\s/.test(line)) {
      end = i;
      break;
    }
    if (isItem(line)) {
      if (start !== -1) ranges.push([start, i]);
      start = i + 1; // 1-based
    }
  }
  if (start !== -1) ranges.push([start, end]);
  return ranges;
}

/**
 * How a node's own position maps to file lines. `perLine` shifts each node by
 * `offset`; otherwise every node reports the whole field, because the source
 * lines cannot be told apart (see fieldLines).
 */
interface LineMap {
  perLine: boolean;
  offset: number;
  first: number;
  last: number;
}

function stamp(node: Element, map: LineMap): void {
  const pos = node.position;
  if (!pos?.start?.line || !pos?.end?.line) return;
  node.properties = node.properties ?? {};
  if (map.perLine) {
    node.properties['data-line'] = pos.start.line + map.offset;
    node.properties['data-line-end'] = pos.end.line + map.offset;
  } else {
    node.properties['data-line'] = map.first;
    node.properties['data-line-end'] = map.last;
  }
}

function walkListItems(children: ElementContent[], map: LineMap): void {
  for (const child of children) {
    if (child.type !== 'element') continue;
    if (child.tagName === 'li') stamp(child, map);
    if (child.children) walkListItems(child.children, map);
  }
}

/** Stamp data-line/-end so the numbers point at lines in the source file. */
function rehypeFieldSourceLines(map: LineMap) {
  return (tree: Root) => {
    for (const node of tree.children) {
      if (node.type !== 'element') continue;
      stamp(node, map);
      if ((node.tagName === 'ul' || node.tagName === 'ol') && node.children) {
        walkListItems(node.children, map);
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

  let map: LineMap | null = null;
  try {
    const text = readFileSync(source.filePath, 'utf8');
    const range = fieldLines(text, source.field);
    if (range) {
      // remark counts the field's content from 1; the same content sits on
      // `first` in the file, so everything shifts by their difference.
      map = { ...range, offset: range.first - 1 };
    }
  } catch {
    // An unreadable file costs the edit affordance, not the render.
  }

  // No range means no provenance: stamping anyway would point the edit link
  // at whatever happens to sit on those lines, which is worse than offering
  // no link at all. The markdown still renders.
  if (!map) return String(plain.processSync(value));

  const processor = base()
    .use(rehypeFieldSourceLines, map)
    .use(rehypeStringify);
  return String(processor.processSync(value));
}
