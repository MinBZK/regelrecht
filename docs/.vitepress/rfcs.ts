// Single source of truth for the RFC list.
//
// Both the VitePress sidebar (config.ts) and the index table (via a data
// loader) are generated from docs/rfcs/rfc-*.md, so they cannot drift apart.
// This module is pure Node — no Vue/VitePress imports — so it runs in the
// VitePress config and in the data loader at build time.

import { readdirSync, readFileSync } from 'node:fs'
import { fileURLToPath } from 'node:url'

export interface RfcEntry {
  /** Numeric RFC number, e.g. 8 */
  num: number
  /** Zero-padded id, e.g. "RFC-008" */
  id: string
  /** Full H1 title — used in the index table */
  title: string
  /** Optional **Short title:** value, falls back to title — used in the sidebar */
  shortTitle: string
  /** Status from the **Status:** preamble line */
  status: string
  /** Site-relative link derived from the real filename, e.g. "/rfcs/rfc-008" */
  link: string
}

const RFCS_DIR = fileURLToPath(new URL('../rfcs', import.meta.url))

const RFC_FILE = /^rfc-(\d+)\.md$/
const H1 = /^#\s*RFC-(\d+):\s*(.+?)\s*$/m
const STATUS = /^\*\*Status:\*\*\s*(.+?)\s*$/m
const SHORT_TITLE = /^\*\*Short title:\*\*\s*(.+?)\s*$/m

/**
 * Scan docs/rfcs for rfc-NNN.md files and parse number, title, status and
 * optional short title from each. Throws if an RFC file lacks a parseable H1,
 * so a broken RFC fails the build loudly instead of silently disappearing.
 */
export function getRfcs(): RfcEntry[] {
  const entries: RfcEntry[] = []
  const seen = new Map<number, string>()

  for (const filename of readdirSync(RFCS_DIR)) {
    // This filter excludes index.md and template.md by construction.
    if (!RFC_FILE.test(filename)) continue

    const content = readFileSync(`${RFCS_DIR}/${filename}`, 'utf-8')

    const h1 = content.match(H1)
    if (!h1) {
      throw new Error(
        `docs/rfcs/${filename}: no parseable "# RFC-NNN: Title" heading found`,
      )
    }

    const num = parseInt(h1[1], 10)
    const existing = seen.get(num)
    if (existing) {
      throw new Error(
        `RFC-${num} declared in both ${existing} and ${filename}; ` +
          'RFC numbers must be unique',
      )
    }
    seen.set(num, filename)

    const title = h1[2]
    const status = content.match(STATUS)?.[1] ?? 'Unknown'
    const shortTitle = content.match(SHORT_TITLE)?.[1] ?? title

    // The link is derived from a filename we just read, so it provably
    // resolves to a real page. This is why a bare <a href> in the index
    // component is safe even though it sidesteps VitePress dead-link
    // checking: a non-existent target cannot be produced here.
    entries.push({
      num,
      id: `RFC-${String(num).padStart(3, '0')}`,
      title,
      shortTitle,
      status,
      link: `/rfcs/${filename.replace(/\.md$/, '')}`,
    })
  }

  return entries.sort((a, b) => a.num - b.num)
}

/** Sidebar items for the `/rfcs/` section, with the Overview entry first. */
export function rfcSidebarItems(): { text: string; link: string }[] {
  return [
    { text: 'Overview', link: '/rfcs/' },
    ...getRfcs().map((r) => ({ text: `${r.id}: ${r.shortTitle}`, link: r.link })),
  ]
}
