/*
 * Single source of truth for the documentation navigation.
 * Imported by both config.ts (VitePress themeConfig.nav) and the theme
 * Layout, so the docs header and VitePress metadata never drift.
 *
 * `match` is a path prefix used to mark the active item (aria-current).
 */
export interface DocsNavItem {
  text: string
  link: string
  match?: string
}

export const docsNav: DocsNavItem[] = [
  { text: 'Home', link: '/en/', match: '/en/' },
  { text: 'Guide', link: '/guide/what-is-regelrecht', match: '/guide/' },
  { text: 'Concepts', link: '/concepts/how-it-works', match: '/concepts/' },
  { text: 'Components', link: '/components/engine', match: '/components/' },
  { text: 'Operations', link: '/operations/deployment', match: '/operations/' },
  { text: 'RFCs', link: '/rfcs/', match: '/rfcs/' },
  { text: 'Reference', link: '/reference/glossary', match: '/reference/' },
]
