import { defineConfig } from 'astro/config';
import mdx from '@astrojs/mdx';
import pagefind from 'astro-pagefind';
import rehypeMermaid from 'rehype-mermaid';
import { rehypeMermaidAlt } from './src/lib/rehype-mermaid-alt.ts';

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
  integrations: [
    mdx(),
    // force_language: en builds ONE index for the whole site instead of
    // splitting per <html lang>. The docs are English (57 pages); the
    // bilingual landing is the only Dutch page. Without this, Pagefind makes
    // a separate near-empty `nl` index and a search FROM the Dutch landing
    // never finds the docs (it only queries the index matching the page's
    // lang). One forced-English index makes all content searchable from
    // every page. (Verified at runtime: there is no client-side override —
    // init('en')/mergeIndex/options.language do not switch the index.)
    pagefind({ indexConfig: { forceLanguage: 'en' } }),
  ],
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
    // inline-svg renders the diagram as an inline <svg> in the DOM (not an
    // <img>), so its colours can be themed with CSS and follow the .dark
    // toggle. Mermaid's `base` theme with neutral themeVariables hands the
    // colouring to docs.css (.mermaid svg rules), keyed off currentColor and
    // NLDD tokens, light and dark.
    rehypePlugins: [
      [
        rehypeMermaid,
        {
          strategy: 'inline-svg',
          mermaidConfig: {
            theme: 'base',
            themeVariables: {
              fontFamily: 'RijksSans, system-ui, sans-serif',
              // Transparent plate so the page background (light/dark) shows
              // through; nodes/edges/text are set in docs.css.
              background: 'transparent',
            },
          },
        },
      ],
      rehypeMermaidAlt,
    ],
  },
  vite: {
    ssr: {
      noExternal: ['@nldd/design-system'],
    },
  },
});
