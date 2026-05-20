/*
 * Documentation sidebar structure, ported verbatim from the former
 * VitePress themeConfig.sidebar object literal. Single source of truth for
 * the per-section navigation; the /rfcs/ group items come from rfcs.ts so
 * the RFC list cannot drift from the actual rfc-*.md files.
 */
import { rfcSidebarItems } from './rfcs';

export interface SidebarItem {
  text: string;
  link?: string;
  items?: SidebarItem[];
}
export interface SidebarGroup {
  text?: string;
  items: SidebarItem[];
}

export const sidebar: Record<string, SidebarGroup[]> = {
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
        { text: 'Toegankelijkheid', link: '/reference/toegankelijkheid' },
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
};

/** The sidebar group(s) for a pathname, by matching the section prefix. */
export function sidebarForPath(pathname: string): SidebarGroup[] | null {
  for (const prefix of Object.keys(sidebar)) {
    if (pathname.startsWith(prefix)) return sidebar[prefix];
  }
  return null;
}

/** Flatten the sidebar section that owns this pathname into ordered links. */
export function flatSidebar(pathname: string): { text: string; link: string }[] {
  const groups = sidebarForPath(pathname);
  if (!groups) return [];
  const out: { text: string; link: string }[] = [];
  const walk = (items: SidebarItem[]) => {
    for (const it of items) {
      if (it.link) out.push({ text: it.text, link: it.link });
      if (it.items) walk(it.items);
    }
  };
  for (const g of groups) walk(g.items);
  return out;
}
