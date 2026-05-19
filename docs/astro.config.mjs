import { defineConfig } from 'astro/config';
import mdx from '@astrojs/mdx';
import pagefind from 'astro-pagefind';
import rehypeMermaid from 'rehype-mermaid';

export default defineConfig({
  site: 'https://docs.regelrecht.rijks.app',
  // `format: 'directory'` emits every page as `<route>/index.html`, so the
  // index routes the internal links use with a trailing slash (`/en/`,
  // `/rfcs/`) resolve, and slash-less doc links (`/guide/what-is-regelrecht`)
  // are served directly by nginx trying `$uri/index.html` before the bare
  // directory — no 301 hop (see docs/nginx.conf try_files). `trailingSlash:
  // 'ignore'` lets both `/x` and `/x/` work, matching that nginx rule.
  trailingSlash: 'ignore',
  build: {
    format: 'directory',
  },
  integrations: [mdx(), pagefind()],
  markdown: {
    // Exclude `mermaid` from Shiki so the fenced block reaches
    // rehype-mermaid as a real <pre><code class="language-mermaid">
    // instead of being pre-highlighted into styled spans.
    syntaxHighlight: {
      type: 'shiki',
      excludeLangs: ['mermaid'],
    },
    shikiConfig: {
      themes: {
        light: 'github-light-high-contrast',
        dark: 'github-dark-high-contrast',
      },
    },
    rehypePlugins: [[rehypeMermaid, { strategy: 'img-svg', dark: false }]],
  },
  vite: {
    ssr: {
      noExternal: ['@nldd/design-system'],
    },
  },
});
