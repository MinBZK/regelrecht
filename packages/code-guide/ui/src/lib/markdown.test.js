// @vitest-environment jsdom
// happy-dom 20, the default test environment, has a NodeIterator bug that makes
// DOMPurify drop elements (the first <p>, a <pre>). Real browsers and jsdom do
// not, so this file runs under jsdom; frontend/src/components/ArticleText.test.js
// does the same for the same reason.
import { describe, expect, it } from 'vitest';
import { renderDocs, selfResolver } from './markdown.js';

describe('renderDocs', () => {
  it('renders Markdown: paragraphs, inline code, lists', () => {
    const html = renderDocs('First line.\n\nUses `Value`:\n\n- one\n- two');
    expect(html).toContain('<p>First line.</p>');
    expect(html).toContain('<code>Value</code>');
    expect(html).toMatch(/<li>one<\/li>\s*<li>two<\/li>/);
  });

  it('renders a fenced example as a code block', () => {
    expect(renderDocs('```rust\nlet x = 1;\n```')).toContain('<pre><code class="language-rust">let x = 1;');
  });

  it('removes anything that could run', () => {
    const html = renderDocs('<script>alert(1)</script>\n\n<img src=x onerror=alert(1)>\n\n[x](javascript:alert(1))');
    expect(html).not.toMatch(/<script|onerror|javascript:/i);
  });

  it('turns a rustdoc link it can resolve into an app link', () => {
    const html = renderDocs('See [`evaluate_law`](Self::evaluate_law).', {
      resolve: (p) => (p === 'Self::evaluate_law' ? '#/engine/service/S/evaluate_law' : null),
    });
    expect(html).toContain('<a href="#/engine/service/S/evaluate_law"><code>evaluate_law</code></a>');
  });

  it('keeps the text of a rustdoc link it cannot resolve, without a dead link', () => {
    const html = renderDocs('See [`EngineError`](crate::error::EngineError).');
    expect(html).toContain('<code>EngineError</code>');
    expect(html).not.toContain('<a');
  });

  it('keeps an external link and opens it in a new tab', () => {
    const html = renderDocs('[RFC](https://example.org/rfc)');
    expect(html).toContain('href="https://example.org/rfc"');
    expect(html).toContain('target="_blank"');
    expect(html).toContain('rel="noopener noreferrer"');
  });

  it('puts doc headings below the page headings', () => {
    expect(renderDocs('# Example')).toContain('<h5>Example</h5>');
    expect(renderDocs('### Deep')).toContain('<h6>Deep</h6>');
    expect(renderDocs('###### Deepest')).toContain('<h6>Deepest</h6>');
  });

  it('gives nothing for no docs', () => {
    expect(renderDocs(null)).toBe('');
    expect(renderDocs('')).toBe('');
  });
});

describe('selfResolver', () => {
  const resolve = selfResolver(['new', 'evaluate_law'], (m) => `#/m/${m}`);

  it('resolves Self:: paths and bare names of the type\'s methods', () => {
    expect(resolve('Self::evaluate_law')).toBe('#/m/evaluate_law');
    expect(resolve('new')).toBe('#/m/new');
    expect(resolve('Self::new()')).toBe('#/m/new');
  });

  it('leaves other paths unresolved instead of guessing', () => {
    expect(resolve('crate::error::EngineError')).toBeNull();
    expect(resolve('Self::missing')).toBeNull();
    expect(resolve('Vec::new')).toBeNull();
  });
});
