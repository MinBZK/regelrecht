import { defineConfig } from 'vitepress'
import { withMermaid } from 'vitepress-plugin-mermaid'

export default withMermaid(
  defineConfig({
    title: 'RegelRecht',
    description: 'Machine-readable Dutch law execution',
    lang: 'en',

    ignoreDeadLinks: [
      /^https?:\/\/localhost/,
    ],

    // i18n: add Dutch locale here when translations are ready.
    // See https://vitepress.dev/guide/i18n for setup.
    locales: {
      root: { label: 'English', lang: 'en' },
      // nl: { label: 'Nederlands', lang: 'nl', link: '/nl/' },
    },

    themeConfig: {
      logo: '/logo.svg',
      nav: [
        { text: 'Guide', link: '/guide/what-is-regelrecht' },
        { text: 'Concepts', link: '/concepts/how-it-works' },
        { text: 'Components', link: '/components/engine' },
        { text: 'Operations', link: '/operations/deployment' },
        { text: 'RFCs', link: '/rfcs/' },
        { text: 'Reference', link: '/reference/glossary' },
      ],
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
            items: [
              { text: 'Overview', link: '/rfcs/' },
              { text: 'RFC-000: RFC Process', link: '/rfcs/rfc-000' },
              { text: 'RFC-001: YAML Schema', link: '/rfcs/rfc-001' },
              { text: 'RFC-002: Authority Roles', link: '/rfcs/rfc-002' },
              { text: 'RFC-003: Inversion of Control', link: '/rfcs/rfc-003' },
              { text: 'RFC-004: Uniform Operations', link: '/rfcs/rfc-004' },
              { text: 'RFC-005: Standoff Annotations', link: '/rfcs/rfc-005' },
              { text: 'RFC-006: Language Choice', link: '/rfcs/rfc-006' },
              { text: 'RFC-007: Cross-Law Execution', link: '/rfcs/rfc-007' },
              { text: 'RFC-008: Bestuursrecht/AWB', link: '/rfcs/rfc-008' },
              { text: 'RFC-009: Multi-Org Execution', link: '/rfcs/rfc-009' },
              { text: 'RFC-010: Federated Corpus', link: '/rfcs/rfc-010' },
              { text: 'RFC-012: Untranslatables', link: '/rfcs/rfc-012' },
              { text: 'RFC-013: Execution Provenance', link: '/rfcs/rfc-013' },
              { text: 'RFC-014: Engine Conformance', link: '/rfcs/rfc-014' },
            ],
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
          isCustomElement: (tag: string) => tag.startsWith('rr-'),
        },
      },
    },

    vite: {
      build: {
        rollupOptions: {
          // @minbzk/storybook is optional — externalize if not installed
          external: (id: string) => id.startsWith('@minbzk/storybook'),
        },
      },
    },

    mermaid: {
      theme: 'neutral',
    },
  })
)
