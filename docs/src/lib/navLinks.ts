/*
 * The documentation sections. Consumed by the /docs overview page (one card
 * per section, linking to `link`) and by sidebar.ts, which takes each
 * section's title, summary and intro for its category page and back button.
 *
 * `match` is the section's path prefix; sidebar.ts pairs a section with its
 * sidebar through it.
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
    link: '/guide/',
    match: '/guide/',
    summary:
      "Get oriented: what RegelRecht is, how it's built, and how to run and test it locally.",
    intro:
      'Start here. The guide introduces RegelRecht, sketches the architecture, shows worked translations of real law, and walks you through a development environment and the test suites.',
  },
  {
    text: 'Concepts',
    link: '/concepts/',
    match: '/concepts/',
    summary:
      'The core ideas: the law format, how laws reference and delegate to each other, provenance, and execution across organizations.',
    intro:
      'The ideas behind RegelRecht: how Dutch legislation is written down as executable YAML, how laws reference and delegate to one another, how every result traces back to its legal source, and how several organizations share one corpus. The methodology pages explain how an interpretation is validated.',
  },
  {
    text: 'Components',
    link: '/components/',
    match: '/components/',
    summary:
      'The building blocks: engine, corpus, pipeline, harvester, and the user-facing tools.',
    intro:
      'A tour of the parts that make up RegelRecht: the execution engine and corpus at the core, the processing pipeline and harvester, and the editor and other tools on top.',
  },
  {
    text: 'Operations',
    link: '/operations/',
    match: '/operations/',
    summary:
      'Running RegelRecht: deployment and CI/CD, access and roles, and adding laws to the corpus.',
    intro:
      'Running RegelRecht in practice: how it is deployed and built through CI/CD, how access is governed by roles in Keycloak, how laws are added to the corpus, and how to contribute changes.',
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
    link: '/reference/',
    match: '/reference/',
    summary:
      'Look-ups: glossary, schema reference, conformance, and known harvester issues.',
    intro:
      'Reference material to look things up: a glossary of terms, the law schema, what conformance testing covers, which features are documented, the accessibility statement, and known issues in the harvester.',
  },
]
