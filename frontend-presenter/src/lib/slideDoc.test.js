import { describe, expect, it } from 'vitest';
import { appendBlock, bodyBlocks, normalize, replaceRange, setFrontmatterKey, setYamlKey, splitFrontmatter } from './slideDoc.js';

const SLIDE = `---
kind: content
# een opmerking die moet blijven staan
overline: Hoe het werkt
---

# Drie lagen

Eerste alinea.

- een
- twee

\`\`\`wet
law: wet_op_de_zorgtoeslag
article: '2'
\`\`\`
`;

describe('splitFrontmatter', () => {
  it('splits and rejoins byte for byte', () => {
    const { fm, fmRaw, body } = splitFrontmatter(SLIDE);
    expect(fm).toEqual({ kind: 'content', overline: 'Hoe het werkt' });
    expect(fmRaw + body).toBe(SLIDE);
  });

  it('treats a file without frontmatter as all body', () => {
    expect(splitFrontmatter('# Titel\n')).toEqual({ fm: {}, fmRaw: '', body: '# Titel\n', bodyStart: 0 });
  });

  it('survives broken frontmatter', () => {
    expect(splitFrontmatter('---\nkind: [\n---\n# T\n').body).toBe('# T\n');
  });

  it('normalises CRLF, after which a CRLF slide splits and edits like any other', () => {
    expect(normalize('a\r\nb\rc')).toBe('a\nb\nc');
    const crlf = normalize(SLIDE.replaceAll('\n', '\r\n'));
    expect(splitFrontmatter(crlf).fm.kind).toBe('content');
    expect(bodyBlocks(crlf).every((b) => b.start >= 0)).toBe(true);
  });
});

describe('bodyBlocks', () => {
  it('gives every block its exact range in the file', () => {
    const blocks = bodyBlocks(SLIDE);
    expect(blocks.map((b) => b.token.type)).toEqual(['heading', 'paragraph', 'list', 'code']);
    for (const b of blocks) expect(SLIDE.slice(b.start, b.end)).toBe(b.raw);
  });
});

describe('replaceRange', () => {
  it('changes only the edited block', () => {
    const para = bodyBlocks(SLIDE)[1];
    const out = replaceRange(SLIDE, para.start, para.end, 'Andere alinea.');
    expect(out).toBe(SLIDE.replace('Eerste alinea.', 'Andere alinea.'));
  });

  it('keeps the blank line after a block when the new text has none', () => {
    const heading = bodyBlocks(SLIDE)[0];
    const out = replaceRange(SLIDE, heading.start, heading.end, '# Vier lagen');
    expect(out).toBe(SLIDE.replace('# Drie lagen', '# Vier lagen'));
  });

  it('deletes a block when the text is emptied', () => {
    const para = bodyBlocks(SLIDE)[1];
    const out = replaceRange(SLIDE, para.start, para.end, '  ');
    expect(out).not.toContain('Eerste alinea');
    expect(bodyBlocks(out).map((b) => b.token.type)).toEqual(['heading', 'list', 'code']);
  });
});

describe('frontmatter edits', () => {
  it('rewrites one key and leaves comments, order and body alone', () => {
    const out = setFrontmatterKey(SLIDE, 'overline', 'Anders');
    expect(out).toBe(SLIDE.replace('overline: Hoe het werkt', 'overline: Anders'));
  });

  it('appends a missing key', () => {
    const out = setFrontmatterKey(SLIDE, 'note', 'Spreektekst');
    expect(splitFrontmatter(out).fm.note).toBe('Spreektekst');
    expect(splitFrontmatter(out).body).toBe(splitFrontmatter(SLIDE).body);
  });

  it('adds frontmatter to a file without it', () => {
    const out = setFrontmatterKey('# T\n', 'kind', 'title');
    expect(splitFrontmatter(out)).toMatchObject({ fm: { kind: 'title' } });
    expect(out.endsWith('# T\n')).toBe(true);
  });

  it('quotes values that YAML would otherwise misread', () => {
    const out = setYamlKey('presenter: Oud\n', 'presenter', 'ja: nee');
    expect(out).toBe("presenter: 'ja: nee'\n");
  });

  it('rewrites a value that contains | or > in place instead of adding a duplicate key', () => {
    expect(setYamlKey('presenter: A > B\ntitle: T\n', 'presenter', 'C | D')).toBe("presenter: C | D\ntitle: T\n");
  });

  it('leaves a block scalar alone', () => {
    expect(setYamlKey('note: |\n  lang\n', 'title', 'T')).toBe('note: |\n  lang\ntitle: T\n');
  });

  it('creates a deck.yaml from nothing', () => {
    expect(setYamlKey('', 'presenter', 'Jan Jansen')).toBe('presenter: Jan Jansen\n');
  });
});

describe('appendBlock', () => {
  it('adds a block after one blank line', () => {
    expect(appendBlock('# T\n\n\n', 'Nieuw')).toBe('# T\n\nNieuw\n');
  });
});
