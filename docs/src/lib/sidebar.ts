/*
 * Documentation sidebar structure. Single source of truth for the
 * per-section navigation; the /rfcs/ group items come from rfcs.ts so
 * the RFC list cannot drift from the actual rfc-*.md files.
 *
 * Item labels match the page's frontmatter `title`, or a deliberate short
 * form of it (commented where it differs). The category page fails the build
 * when an item links to a page that does not exist.
 */
import { rfcSidebarItems } from './rfcs';
import { docsNav } from './navLinks';

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
        { text: 'Development Environment', link: '/guide/dev-environment' },
        { text: 'Testing', link: '/guide/testing' },
      ],
    },
  ],
  '/concepts/': [
    {
      text: 'Law Format',
      items: [
        { text: 'How RegelRecht Works', link: '/concepts/how-it-works' },
        { text: 'Law Format', link: '/concepts/law-format' },
        { text: 'Scenarios', link: '/concepts/scenarios' },
        { text: 'Collections', link: '/concepts/collections' },
        { text: 'Temporal Validity and Dates', link: '/concepts/temporal-and-dates' },
        { text: 'Markings', link: '/concepts/markings' },
      ],
    },
    {
      text: 'Relations Between Laws',
      items: [
        { text: 'Cross-Law References', link: '/concepts/cross-law-references' },
        { text: 'Inversion of Control', link: '/concepts/inversion-of-control' },
        { text: 'Hooks and Reactive Execution', link: '/concepts/hooks-and-reactive-execution' },
        { text: 'Competent Authority', link: '/concepts/competent-authority' },
      ],
    },
    {
      text: 'Execution and Accountability',
      items: [
        { text: 'Execution Provenance', link: '/concepts/execution-provenance' },
        { text: 'Traceability', link: '/concepts/traceability' },
      ],
    },
    {
      text: 'Organizations',
      items: [
        { text: 'Multi-Organization Execution', link: '/concepts/multi-org-execution' },
        { text: 'Federated Corpus', link: '/concepts/federated-corpus' },
        { text: 'Notes and Annotations', link: '/concepts/notes-and-annotations' },
      ],
    },
    {
      text: 'Methodology',
      items: [
        { text: 'Execution-First Validation', link: '/concepts/methodology' },
        // Short form of "RegelRecht Validation: From Analysis-First to
        // Execution-First", which is too long for a list row.
        { text: 'From Analysis-First to Execution-First', link: '/concepts/validation-methodology' },
        { text: 'Branches of Law', link: '/concepts/branches-of-law' },
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
        { text: 'Harvester Admin', link: '/components/admin' },
        { text: 'Lawmaking Frontend', link: '/components/lawmaking' },
        { text: 'Demo', link: '/components/demo' },
        { text: 'Terminal UI (TUI)', link: '/components/tui' },
      ],
    },
    {
      text: 'Observability',
      items: [
        { text: 'Grafana Monitoring', link: '/components/grafana' },
      ],
    },
  ],
  '/operations/': [
    {
      text: 'Deployment and Access',
      items: [
        { text: 'CI/CD Pipeline', link: '/operations/ci-cd' },
        { text: 'Deployment', link: '/operations/deployment' },
        // Served at /auth-and-roles, outside /operations/; see sectionForPage.
        { text: 'Authentication & Roles', link: '/auth-and-roles' },
      ],
    },
    {
      text: 'Corpus Management',
      items: [
        { text: 'Adding a Law', link: '/operations/adding-a-law' },
        { text: 'Private-repo trajects', link: '/operations/private-repo-trajects' },
      ],
    },
    {
      text: 'Contributing',
      items: [
        { text: 'Contributing', link: '/operations/contributing' },
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
        { text: 'Schema Reference', link: '/reference/schema' },
        { text: 'Conformance', link: '/reference/conformance' },
        { text: 'Documentation Coverage', link: '/reference/documentation-coverage' },
        { text: 'Accessibility statement', link: '/reference/accessibility' },
      ],
    },
    {
      // Write-ups of specific harvester problems. They stay under /reference/
      // so their URLs do not change. Each page opens by saying whether the
      // issue is open or resolved; the labels are short forms of the titles.
      text: 'Harvester Known Issues',
      items: [
        { text: 'Article ID Collision', link: '/reference/issues/issue-article-id-collision' },
        { text: 'Phased Implementation', link: '/reference/issues/issue-phased-implementation' },
      ],
    },
  ],
};

/*
 * Pages served outside every section prefix that still belong to a section.
 * Section membership is otherwise decided by URL prefix; this map lets such a
 * page keep its URL and still get its section's back button and breadcrumb.
 * Keys are pathnames without a trailing slash.
 */
const sectionForPage: Record<string, string> = {
  '/auth-and-roles': '/operations/',
};

/** The section prefix that owns a pathname, or null. */
function sectionForPath(pathname: string): string | null {
  const explicit = sectionForPage[pathname.replace(/\/$/, '')];
  if (explicit) return explicit;
  for (const prefix of Object.keys(sidebar)) {
    if (pathname.startsWith(prefix)) return prefix;
  }
  return null;
}

/** The sidebar group(s) for a pathname, by its section (see sectionForPath). */
export function sidebarForPath(pathname: string): SidebarGroup[] | null {
  const prefix = sectionForPath(pathname);
  return prefix ? sidebar[prefix] : null;
}

export interface DocsCategory {
  /** Section prefix, e.g. '/guide/'. Also the category page URL. */
  prefix: string;
  /** Display title, from the matching docsNav item. */
  title: string;
  /** One-line summary, from the matching docsNav item. */
  summary?: string;
  /** Intro paragraph for the category page, from the matching docsNav item. */
  intro?: string;
}

/**
 * All documentation categories (used for the back button + the docs overview).
 * The `[category]` route generates an overview page for each, except `/rfcs/`,
 * which has its own hand-built index page under src/pages/rfcs/.
 */
export const docsCategories: DocsCategory[] = Object.keys(sidebar).map(
  (prefix) => {
    const nav = docsNav.find((n) => n.match === prefix);
    return {
      prefix,
      title: nav?.text ?? prefix,
      summary: nav?.summary,
      intro: nav?.intro,
    };
  },
);

/** The category that owns a pathname (article or category page), or null. */
export function categoryForPath(pathname: string): DocsCategory | null {
  const prefix = sectionForPath(pathname);
  return docsCategories.find((c) => c.prefix === prefix) ?? null;
}
