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
        { text: 'Architecture', link: '/architecture/overview' },
        { text: 'Components', link: '/components/engine' },
        { text: 'RFCs', link: '/rfcs/' },
        { text: 'Reference', link: '/reference/glossary' },
      ],
      sidebar: {
        '/guide/': [
          {
            text: 'Introduction',
            items: [
              { text: 'What is RegelRecht?', link: '/guide/what-is-regelrecht' },
              { text: 'Getting Started', link: '/guide/getting-started' },
              { text: 'Law Format', link: '/guide/law-format' },
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
        '/architecture/': [
          {
            text: 'Architecture',
            items: [
              { text: 'System Overview', link: '/architecture/overview' },
              { text: 'Methodology', link: '/architecture/methodology' },
            ],
          },
        ],
        '/components/': [
          {
            text: 'Components',
            items: [
              { text: 'Execution Engine', link: '/components/engine' },
              { text: 'Pipeline', link: '/components/pipeline' },
              { text: 'Harvester', link: '/components/harvester' },
              { text: 'Frontend', link: '/components/frontend' },
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
              { text: 'RFC-010: Federated Corpus', link: '/rfcs/rfc-010' },
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
