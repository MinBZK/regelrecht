/*
 * Single source of truth for the documentation navigation, consumed by
 * Base.astro to build the docs header (and its no-JS fallback nav).
 *
 * `match` is a path prefix used to mark the active item (aria-current).
 */
export interface DocsNavItem {
  text: string
  link: string
  match?: string
  /** Optional NLDD icon name; when set the item shows the icon (text stays
   *  as the accessible label). */
  icon?: string
  /** One-line summary, shown as supporting text on the /docs overview cards. */
  summary?: string
  /** Short intro paragraph, shown in the left column of the category page. */
  intro?: string
}

export const docsNav: DocsNavItem[] = [
  { text: 'Home', link: '/en/', match: '/en/' },
  {
    text: 'Guide',
    link: '/guide/what-is-regelrecht',
    match: '/guide/',
    summary:
      "Get oriented: what RegelRecht is, how it's built, and how to run it locally.",
    intro:
      'Start here. The guide introduces RegelRecht, sketches the architecture, and walks you through getting a development environment running.',
  },
  {
    text: 'Concepts',
    link: '/concepts/how-it-works',
    match: '/concepts/',
    summary:
      'The core ideas: how law becomes executable code, references, delegation, and provenance.',
    intro:
      'The ideas behind RegelRecht: how Dutch legislation is turned into executable code, how laws reference and delegate to one another, and how every result traces back to its legal source.',
  },
  {
    text: 'Components',
    link: '/components/engine',
    match: '/components/',
    summary:
      'The building blocks: engine, corpus, pipeline, harvester, and the user-facing tools.',
    intro:
      'A tour of the parts that make up RegelRecht: the execution engine and corpus at the core, the processing pipeline and harvester, and the editors and dashboards on top.',
  },
  {
    text: 'Operations',
    link: '/operations/deployment',
    match: '/operations/',
    summary:
      'Running RegelRecht: deployment, CI/CD, and adding laws to the corpus.',
    intro:
      'Running RegelRecht in practice: how it is deployed and built through CI/CD, how laws are added to the corpus, and how to contribute changes.',
  },
  {
    text: 'Auth & Roles',
    link: '/auth-and-roles',
    match: '/auth-and-roles',
    summary: 'Who can do what: the authorization and role model.',
  },
  {
    text: 'RFCs',
    link: '/rfcs/',
    match: '/rfcs/',
    summary:
      'Design decisions, documented: the problem, the alternatives, and the chosen approach.',
  },
  {
    text: 'Reference',
    link: '/reference/glossary',
    match: '/reference/',
    summary: 'Look-ups: glossary, schema, accessibility, and known issues.',
    intro:
      'Reference material to look things up: a glossary of terms, the law schema, the accessibility statement, and a record of known issues.',
  },
]
