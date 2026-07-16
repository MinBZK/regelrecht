import { describe, it, expect } from 'vitest';
import { frontmatterTitle, deslugifyDocPath, docDisplayTitle } from './docTitle.js';

describe('frontmatterTitle', () => {
  it('extracts a plain title', () => {
    expect(frontmatterTitle('---\ntitle: Mijn Brief\n---\n\nBody.')).toBe('Mijn Brief');
  });

  it('extracts a quoted title containing a colon and quotes', () => {
    expect(frontmatterTitle('---\ntitle: "Rapport: Q3 \\"final\\""\n---\nBody.')).toBe(
      'Rapport: Q3 "final"',
    );
  });

  it('stringifies a numeric title', () => {
    expect(frontmatterTitle('---\ntitle: 2024\n---\n')).toBe('2024');
  });

  it('tolerates CRLF line endings and a BOM', () => {
    expect(frontmatterTitle('\uFEFF---\r\ntitle: Nota\r\n---\r\nBody.')).toBe('Nota');
  });

  it('returns null without a leading fence', () => {
    expect(frontmatterTitle('# Kop\n\nBody.')).toBeNull();
    expect(frontmatterTitle('')).toBeNull();
    expect(frontmatterTitle(null)).toBeNull();
    // A thematic break further down is not frontmatter.
    expect(frontmatterTitle('Body.\n---\ntitle: X\n---\n')).toBeNull();
  });

  it('returns null on an unterminated fence', () => {
    expect(frontmatterTitle('---\ntitle: X\n')).toBeNull();
  });

  it('returns null on invalid YAML instead of throwing', () => {
    expect(frontmatterTitle('---\ntitle: [\n---\n')).toBeNull();
  });

  it('returns null for a missing, empty, blank or non-scalar title', () => {
    expect(frontmatterTitle('---\nauthor: X\n---\n')).toBeNull();
    expect(frontmatterTitle('---\ntitle: ""\n---\n')).toBeNull();
    expect(frontmatterTitle('---\ntitle: "   "\n---\n')).toBeNull();
    expect(frontmatterTitle('---\ntitle: [a, b]\n---\n')).toBeNull();
    expect(frontmatterTitle('---\ntitle: true\n---\n')).toBeNull();
  });
});

describe('deslugifyDocPath', () => {
  it('strips the extension, expands separators and capitalizes', () => {
    expect(deslugifyDocPath('untitled.md')).toBe('Untitled');
    expect(deslugifyDocPath('mijn-brief-2.md')).toBe('Mijn brief 2');
    expect(deslugifyDocPath('een_lang__verhaal.md')).toBe('Een lang verhaal');
  });

  it('keeps the folder prefix as-is and works for .txt', () => {
    expect(deslugifyDocPath('nota/bijlage.txt')).toBe('nota/Bijlage');
    expect(deslugifyDocPath('mvt/concept-v2.md')).toBe('mvt/Concept v2');
  });

  it('returns an empty string for empty input', () => {
    expect(deslugifyDocPath(null)).toBe('');
    expect(deslugifyDocPath('')).toBe('');
  });
});

describe('docDisplayTitle', () => {
  it('prefers the entry title and falls back to the de-slugged path', () => {
    expect(docDisplayTitle({ path: 'mijn-brief.md', title: 'Mijn Brief' })).toBe('Mijn Brief');
    expect(docDisplayTitle({ path: 'mijn-brief.md' })).toBe('Mijn brief');
    expect(docDisplayTitle(null)).toBe('');
  });
});
