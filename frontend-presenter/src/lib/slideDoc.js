/**
 * A slide file is frontmatter plus markdown. Edits made in the browser are
 * written back as text replacements on that file, never by re-serialising it:
 * the file stays the source, and a change shows up in git as exactly the lines
 * that were edited.
 *
 * - A frontmatter field is changed by rewriting only its own line.
 * - A body block is changed by replacing its character range. The ranges come
 *   from marked's lexer, whose tokens carry their source text in `raw`.
 */
import { marked } from 'marked';
import * as yaml from 'js-yaml';

const FM_RE = /^---\n([\s\S]*?)\n?---[ \t]*(?:\n|$)/;

/** CRLF → LF once, on read, so every offset below is into the same string. */
export function normalize(src) {
  return String(src ?? '').replace(/\r\n?/g, '\n');
}

/** `{ fm, fmRaw, body, bodyStart }`; `fmRaw` includes both `---` lines. */
export function splitFrontmatter(src) {
  const m = src.match(FM_RE);
  if (!m) return { fm: {}, fmRaw: '', body: src, bodyStart: 0 };
  let fm = {};
  try {
    fm = yaml.load(m[1]) ?? {};
  } catch {
    // A broken frontmatter should not take the whole slide down: show the body.
  }
  return { fm: typeof fm === 'object' ? fm : {}, fmRaw: m[0], body: src.slice(m[0].length), bodyStart: m[0].length };
}

/**
 * Set a top-level key in a YAML text by rewriting only that key's line, so
 * comments, order and the other values stay as the author wrote them. A key
 * that is not there yet is appended. A multi-line value for the key (a block
 * scalar or a nested map) is left alone and the key is appended instead, which
 * YAML resolves to the last one; that case does not occur for the scalar fields
 * the presenter edits.
 */
export function setYamlKey(text, key, value) {
  const line = yaml.dump({ [key]: value }, { lineWidth: -1 }).trimEnd();
  const re = new RegExp(`^${key.replace(/[.*+?^${}()|[\]\\]/g, '\\$&')}:[ \\t]*[^\\n|>]*$`, 'm');
  if (re.test(text)) return text.replace(re, () => line);
  const sep = text === '' || text.endsWith('\n') ? '' : '\n';
  return `${text}${sep}${line}\n`;
}

/** Set a frontmatter key in a slide file; the body is not touched. */
export function setFrontmatterKey(src, key, value) {
  const { fmRaw, body } = splitFrontmatter(src);
  if (!fmRaw) return `---\n${setYamlKey('', key, value)}---\n\n${body}`;
  const inner = fmRaw.replace(/^---\n/, '').replace(/---[ \t]*\n?$/, '');
  const trailing = fmRaw.endsWith('\n') ? '\n' : '';
  return `---\n${setYamlKey(inner, key, value)}---${trailing}${body}`;
}

/**
 * The top-level blocks of the body with their absolute ranges in `src`.
 * `space` tokens (blank lines) are skipped; they are not something to edit.
 * A token whose raw text cannot be found at the cursor gets `start: -1`: it is
 * still rendered, but not offered for editing, since saving it would replace
 * the wrong text.
 */
export function bodyBlocks(src) {
  const { body, bodyStart } = splitFrontmatter(src);
  const tokens = marked.lexer(body);
  const blocks = [];
  let cursor = 0;
  for (const token of tokens) {
    const at = body.indexOf(token.raw, cursor);
    const found = at === cursor || (at >= 0 && body.slice(cursor, at).trim() === '');
    if (found) cursor = at + token.raw.length;
    if (token.type === 'space') continue;
    blocks.push({
      token,
      links: tokens.links,
      raw: token.raw,
      start: found ? bodyStart + at : -1,
      end: found ? bodyStart + at + token.raw.length : -1,
    });
  }
  return blocks;
}

/**
 * Replace `src[start, end)` by `text`, keeping the block's trailing newlines.
 * An empty `text` deletes the block together with the blank line after it.
 */
export function replaceRange(src, start, end, text) {
  if (text.trim() === '') return src.slice(0, start) + src.slice(end).replace(/^\n+/, '');
  const trailing = src.slice(start, end).match(/\n*$/)[0];
  return src.slice(0, start) + text.replace(/\n+$/, '') + trailing + src.slice(end);
}

/** Append a block at the end of the body, separated by one blank line. */
export function appendBlock(src, text) {
  const trimmed = src.replace(/\n+$/, '');
  return `${trimmed}\n\n${text.replace(/\n+$/, '')}\n`;
}
