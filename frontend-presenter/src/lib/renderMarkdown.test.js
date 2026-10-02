// DOMPurify needs a real DOM parser; happy-dom's drops text around tags.
// @vitest-environment jsdom
import { describe, expect, it } from 'vitest';
import { bodyBlocks } from './slideDoc.js';
import { renderBlock, renderInline } from './renderMarkdown.js';

const pieces = (md) => bodyBlocks(md).map(renderBlock);

describe('renderBlock', () => {
  it('turns wet and mermaid code blocks into component pieces', () => {
    const [wet, mermaid, code] = pieces("```wet\nlaw: x\narticle: '2'\n```\n\n```mermaid\nflowchart LR\n A-->B\n```\n\n```yaml\na: 1\n```\n");
    expect(wet).toEqual({ kind: 'wet', spec: { law: 'x', article: '2' }, error: null });
    expect(mermaid).toEqual({ kind: 'mermaid', source: 'flowchart LR\n A-->B' });
    expect(code.kind).toBe('html');
    expect(code.html).toContain('<pre>');
  });

  it('reports a wet block without a law, or with broken YAML', () => {
    expect(pieces('```wet\narticle: 2\n```\n')[0].error).toMatch(/law/);
    expect(pieces('```wet\nlaw: [\n```\n')[0].error).toMatch(/Ongeldige YAML/);
  });

  it('strips unsafe HTML', () => {
    const [p] = pieces('<img src=x onerror="alert(1)"> <script>alert(1)</script>tekst\n');
    expect(p.html).not.toMatch(/onerror|<script/);
  });

  it('resolves reference links defined elsewhere in the slide', () => {
    const [p] = pieces('Zie [de wet][w].\n\n[w]: https://wetten.overheid.nl/\n');
    expect(p.html).toContain('href="https://wetten.overheid.nl/"');
  });
});

describe('renderInline', () => {
  it('renders emphasis without a paragraph', () => {
    expect(renderInline('De wet is de **specificatie**.')).toBe('De wet is de <strong>specificatie</strong>.');
  });
});
