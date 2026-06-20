import { defineConfig } from 'astro/config';
import mdx from '@astrojs/mdx';
import pagefind from 'astro-pagefind';
import remarkGfm from 'remark-gfm';
import rehypeMermaid from 'rehype-mermaid';
import { rehypeMermaidAlt } from './src/lib/rehype-mermaid-alt.ts';
import { rehypeNlddCodeViewer } from './src/lib/rehype-nldd-code-viewer.ts';
import { rehypeSourceLines } from './src/lib/rehype-source-lines.ts';
import { rehypeRfcLinks } from './src/lib/rehype-rfc-links.ts';

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
    // No Shiki: rehype-nldd-code-viewer turns every fenced block into <nldd-code-viewer>,
    // which owns styling + (Prism) highlighting. Disabling Shiki also leaves
    // ```mermaid blocks as real <pre><code class="language-mermaid"> for
    // rehype-mermaid (it skips them via the language check).
    syntaxHighlight: false,
    // GFM tables (and the rest of GitHub-flavored markdown) parse into real
    // <table> nodes that nldd-rich-text then styles. Astro enables gfm for
    // plain .md by default, but @astrojs/mdx does not pick it up, so without
    // this an MDX page renders a pipe table as raw text. Listing remark-gfm
    // explicitly here covers both .md and .mdx (MDX extends markdown config).
    remarkPlugins: [remarkGfm],
    // inline-svg renders the diagram as an inline <svg> in the DOM (not an
    // <img>), so its colours can be themed with CSS and follow the .dark
    // toggle. Mermaid's `base` theme with neutral themeVariables hands the
    // colouring to docs.css (.mermaid svg rules), keyed off currentColor and
    // NLDD tokens, light and dark.
    rehypePlugins: [
      // Stamp source-line data attributes first, before the plugins below
      // restructure nodes and lose the original markdown positions.
      rehypeSourceLines,
      // Auto-link bare RFC cross-references ("RFC-008", "RFC-001 §9") in RFC
      // bodies. Runs after source-lines (it inserts <a> nodes, which would
      // otherwise perturb the line stamping) and before the code-viewer
      // reshape; it skips <a>/<code>/<pre> so existing links and code examples
      // are left untouched.
      rehypeRfcLinks,
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
      rehypeNlddCodeViewer,
    ],
  },
  vite: {
    ssr: {
      noExternal: ['@nldd/design-system'],
    },
  },
});
