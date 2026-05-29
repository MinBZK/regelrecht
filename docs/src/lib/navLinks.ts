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
}

export const docsNav: DocsNavItem[] = [
  { text: 'Home', link: '/en/', match: '/en/' },
  { text: 'Guide', link: '/guide/what-is-regelrecht', match: '/guide/' },
  { text: 'Concepts', link: '/concepts/how-it-works', match: '/concepts/' },
  { text: 'Components', link: '/components/engine', match: '/components/' },
  { text: 'Operations', link: '/operations/deployment', match: '/operations/' },
  { text: 'Auth & Roles', link: '/auth-and-roles', match: '/auth-and-roles' },
  { text: 'RFCs', link: '/rfcs/', match: '/rfcs/' },
  { text: 'Reference', link: '/reference/glossary', match: '/reference/' },
]
