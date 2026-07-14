/**
 * Display titles for werkdocumenten. The document's identity stays its path
 * (restricted to [a-z0-9._-]); these helpers only decide what the user SEES:
 * a `title:` from a leading YAML frontmatter block when present, otherwise a
 * de-slugged rendering of the filename. The raw path-based name used by the
 * rename sheet lives in useDocumentsManager (displayTitle/pathFromTitle) and
 * must stay path-faithful — do not swap it for these.
 */
import * as yaml from 'js-yaml';

/**
 * Title from a leading YAML frontmatter block (`---` … `---`), or null.
 * Cosmetic only: any malformed shape (no opening fence, unterminated fence,
 * YAML parse error, non-scalar or empty title) yields null, never throws.
 */
export function frontmatterTitle(body) {
  if (typeof body !== 'string') return null;
  const text = body.replace(/^\uFEFF/, '');
  // This runs on every keystroke (the editor header follows the live body),
  // so bail before splitting: the common no-frontmatter case must be O(1),
  // and a real frontmatter block is tiny \u2014 scan at most the first 8 KiB
  // rather than the whole (possibly PDF-sized) document.
  if (!text.startsWith('---')) return null;
  const lines = text.slice(0, 8192).split('\n');
  if (lines[0].replace(/\r$/, '') !== '---') return null;
  const end = lines.findIndex((line, i) => i > 0 && line.replace(/\r$/, '') === '---');
  if (end === -1) return null;
  let parsed;
  try {
    parsed = yaml.load(lines.slice(1, end).join('\n'));
  } catch {
    return null;
  }
  const title = parsed && typeof parsed === 'object' ? parsed.title : undefined;
  if (typeof title !== 'string' && typeof title !== 'number') return null;
  const trimmed = String(title).trim();
  return trimmed || null;
}

/**
 * De-slugged fallback title for a document path: last segment with the
 * extension stripped, `-`/`_` turned into spaces (runs collapsed), first
 * letter capitalized. Any directory prefix stays as-is so two same-named
 * documents in different folders remain distinguishable:
 * `nota/bijlage-v2.md` → "nota/Bijlage v2".
 */
export function deslugifyDocPath(path) {
  if (!path) return '';
  const slash = path.lastIndexOf('/');
  const prefix = slash === -1 ? '' : path.slice(0, slash + 1);
  const name = path
    .slice(slash + 1)
    .replace(/\.[^.]+$/, '')
    .replace(/[-_]+/g, ' ')
    .trim();
  return prefix + name.charAt(0).toUpperCase() + name.slice(1);
}

/** Display title for a list entry: its frontmatter title, else the de-slugged path. */
export function docDisplayTitle(entry) {
  if (!entry) return '';
  return entry.title || deslugifyDocPath(entry.path);
}
