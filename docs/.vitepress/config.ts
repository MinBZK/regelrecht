import { defineConfig } from 'vitepress'
import { withMermaid } from 'vitepress-plugin-mermaid'
import { rfcSidebarItems } from './rfcs'
import { docsNav } from './navLinks'

export default withMermaid(
  defineConfig({
    title: 'RegelRecht',
    titleTemplate: ':title · RegelRecht',
    description: 'Machine-readable Dutch law execution',
    lang: 'en',

    head: [
      ['link', { rel: 'icon', type: 'image/svg+xml', href: '/regelrecht-icon.svg' }],
    ],

    // VitePress' default light code theme (github-light) renders some tokens
    // (e.g. the string/keyword green #22863A) at ~4.42:1 on the code-block
    // background — below the WCAG 2.2 AA 4.5:1 threshold. The high-contrast
    // GitHub themes keep every token >= 4.5:1. This fixes contrast in every
    // code block site-wide, not just the landing.
    markdown: {
      theme: {
        light: 'github-light-high-contrast',
        dark: 'github-dark-high-contrast',
      },
    },

    ignoreDeadLinks: [
      /^https?:\/\/localhost/,
    ],

    // The site default lang is `en` (the docs). The Dutch landing pages
    // ("/" and "/aanmelden") must declare lang="nl" on <html> for WCAG 3.1.1.
    // VitePress emits the site lang into the SSR HTML; rewrite it for the
    // Dutch routes only. EN landing and all docs stay `en`.
    transformHtml(code, id) {
      // `id` is the absolute path of the emitted file in dist. Only the
      // bare "index.html" (= "/") and "aanmelden.html" (= "/aanmelden")
      // are the Dutch landing pages; everything else (incl. en/index.html,
      // rfcs/index.html, all docs) stays English.
      const rel = id.split('/dist/')[1] ?? ''
      if (rel === 'index.html' || rel === 'aanmelden.html') {
        return code.replace(/<html([^>]*?)\slang="en"/, '<html$1 lang="nl"')
      }
    },

    // The public-facing landing is bilingual and lives entirely in the custom
    // theme Layout: Dutch at "/", English at "/en/". The English documentation
    // stays at the root (/guide/, /concepts/, ...) — it is the only docs
    // language, so VitePress `locales` are intentionally NOT used here.
    // Landing language is derived from the route path in the Layout component.

    themeConfig: {
      logo: '/logo.svg',
      // Shared with the RrNav component (see ./navLinks). The visible header
      // is RrNav; this stays for VitePress metadata/prev-next consistency.
      nav: docsNav.map(({ text, link }) => ({ text, link })),
      sidebar: {
        '/guide/': [
          {
            text: 'Introduction',
            items: [
              { text: 'What is RegelRecht?', link: '/guide/what-is-regelrecht' },
              { text: 'Architecture Overview', link: '/guide/architecture' },
              { text: 'Getting Started', link: '/guide/getting-started' },
            ],
          },
          {
            text: 'Translating Law',
            items: [
              { text: 'Translation Examples', link: '/guide/translation-examples' },
            ],
          },
          {
            text: 'Development',
            items: [
              { text: 'Dev Environment', link: '/guide/dev-environment' },
              { text: 'Testing', link: '/guide/testing' },
            ],
          },
        ],
        '/concepts/': [
          {
            text: 'How It Works',
            items: [
              { text: 'Overview', link: '/concepts/how-it-works' },
              { text: 'Law Format', link: '/concepts/law-format' },
              { text: 'Cross-Law References', link: '/concepts/cross-law-references' },
              { text: 'Inversion of Control', link: '/concepts/inversion-of-control' },
              { text: 'Hooks and Reactive Execution', link: '/concepts/hooks-and-reactive-execution' },
              { text: 'Multi-Org Execution', link: '/concepts/multi-org-execution' },
              { text: 'Federated Corpus', link: '/concepts/federated-corpus' },
              { text: 'Untranslatables', link: '/concepts/untranslatables' },
              { text: 'Execution Provenance', link: '/concepts/execution-provenance' },
            ],
          },
          {
            text: 'Research',
            items: [
              { text: 'Branches of Law', link: '/concepts/branches-of-law' },
            ],
          },
          {
            text: 'Methodology',
            items: [
              { text: 'Validation Methodology', link: '/concepts/methodology' },
              { text: 'Validation Methodology (full)', link: '/concepts/validation-methodology' },
            ],
          },
        ],
        '/components/': [
          {
            text: 'Core',
            items: [
              { text: 'Execution Engine', link: '/components/engine' },
              { text: 'Corpus Library', link: '/components/corpus' },
            ],
          },
          {
            text: 'Processing',
            items: [
              { text: 'Pipeline', link: '/components/pipeline' },
              { text: 'Harvester', link: '/components/harvester' },
            ],
          },
          {
            text: 'User Interfaces',
            items: [
              { text: 'Editor', link: '/components/frontend' },
              { text: 'Editor API', link: '/components/editor-api' },
              { text: 'Admin Dashboard', link: '/components/admin' },
              { text: 'Lawmaking Frontend', link: '/components/lawmaking' },
              { text: 'Landing Page', link: '/components/landing' },
              { text: 'TUI', link: '/components/tui' },
            ],
          },
          {
            text: 'Observability',
            items: [
              { text: 'Grafana', link: '/components/grafana' },
            ],
          },
        ],
        '/operations/': [
          {
            text: 'Deployment',
            items: [
              { text: 'CI/CD Pipeline', link: '/operations/ci-cd' },
              { text: 'Deployment', link: '/operations/deployment' },
            ],
          },
          {
            text: 'Corpus Management',
            items: [
              { text: 'Adding a Law', link: '/operations/adding-a-law' },
            ],
          },
          {
            text: 'Contributing',
            items: [
              { text: 'Contributing Guide', link: '/operations/contributing' },
            ],
          },
        ],
        '/rfcs/': [
          {
            text: 'RFCs',
            items: rfcSidebarItems(),
          },
        ],
        '/reference/': [
          {
            text: 'Reference',
            items: [
              { text: 'Glossary', link: '/reference/glossary' },
              { text: 'Schema', link: '/reference/schema' },
            ],
          },
          {
            text: 'Known Issues',
            items: [
              { text: 'Article ID Collision', link: '/reference/issues/issue-article-id-collision' },
              { text: 'Phased Implementation', link: '/reference/issues/issue-phased-implementation' },
            ],
          },
        ],
      },
      socialLinks: [
        { icon: 'github', link: 'https://github.com/MinBZK/regelrecht' },
      ],
      search: {
        provider: 'local',
      },
      editLink: {
        pattern: 'https://github.com/MinBZK/regelrecht/edit/main/docs/:path',
      },
    },

    vue: {
      template: {
        compilerOptions: {
          isCustomElement: (tag: string) =>
            tag.startsWith('rr-') || tag.startsWith('nldd-'),
        },
      },
    },

    vite: {
      // @nldd/design-system (Lit web components) is bundled for the client so
      // <nldd-*> elements actually upgrade in the browser. It must NOT run
      // during SSR (Lit needs the DOM): the theme imports it client-only,
      // guarded by `typeof window`, and ssr.noExternal keeps Vite from trying
      // to externalize/evaluate it on the server.
      ssr: {
        noExternal: ['@nldd/design-system'],
      },
    },

    mermaid: {
      theme: 'neutral',
    },
  })
)
