// Single source of truth for the RFC list.
//
// Both the docs sidebar (via sidebar.ts) and the RFC index table
// (RfcIndexTable.astro) are generated from src/content/rfcs/rfc-*.md, so they
// cannot drift apart. This module is pure Node, so it runs at build time.

import { existsSync, readdirSync, readFileSync } from 'node:fs'
import { resolve } from 'node:path'
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

// At `astro dev` this module runs from src/lib; at `astro build` it is
// bundled into dist/.prerender/chunks, so a path relative to import.meta
// no longer points at the RFC sources. Resolve from the project root
// (the docs/ working directory) and fall back to the module-relative
// path for the dev case.
function resolveRfcsDir(): string {
  const fromCwd = resolve(process.cwd(), 'src/content/rfcs')
  if (existsSync(fromCwd)) return fromCwd
  return fileURLToPath(new URL('../content/rfcs', import.meta.url))
}

const RFCS_DIR = resolveRfcsDir()

const RFC_FILE = /^rfc-(\d+)\.md$/
const FRONTMATTER = /^---\n([\s\S]*?)\n---\n/
const TITLE = /^title:\s*"([^"]+)"/m
const STATUS = /^\*\*Status:\*\*\s*(.+?)\s*$/m
const SHORT_TITLE = /^\*\*Short title:\*\*\s*(.+?)\s*$/m

/**
 * Scan docs/rfcs for rfc-NNN.md files and parse number (from filename),
 * title (from frontmatter), status and optional short title (both from the
 * body preamble). Throws if a file lacks a parseable frontmatter title, so a
 * broken RFC fails the build loudly instead of silently disappearing.
 */
export function getRfcs(): RfcEntry[] {
  const entries: RfcEntry[] = []
  const seen = new Map<number, string>()

  for (const filename of readdirSync(RFCS_DIR)) {
    // This filter excludes index.md and template.md by construction.
    const fileMatch = filename.match(RFC_FILE)
    if (!fileMatch) continue
    const num = parseInt(fileMatch[1], 10)

    const existing = seen.get(num)
    if (existing) {
      throw new Error(
        `RFC-${num} declared in both ${existing} and ${filename}; ` +
          'RFC numbers must be unique',
      )
    }
    seen.set(num, filename)

    const content = readFileSync(`${RFCS_DIR}/${filename}`, 'utf-8')

    const fm = content.match(FRONTMATTER)
    if (!fm) {
      throw new Error(
        `docs/rfcs/${filename}: no frontmatter — expected at least a "title" field`,
      )
    }
    const titleMatch = fm[1].match(TITLE)
    if (!titleMatch) {
      throw new Error(
        `docs/rfcs/${filename}: no parseable 'title: "…"' in frontmatter`,
      )
    }
    // Strip a leading "RFC-NNN: " prefix if the migration left one in
    // place — the sidebar already prepends the id, so the descriptive
    // half is what we want stored as `title`.
    const title = titleMatch[1].replace(/^RFC-\d+:\s*/, '')
    const status = content.match(STATUS)?.[1] ?? 'Unknown'
    const shortTitle = content.match(SHORT_TITLE)?.[1] ?? title

    // The link is derived from a filename we just read, so it provably
    // resolves to a real page: a non-existent target cannot be produced here.
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
