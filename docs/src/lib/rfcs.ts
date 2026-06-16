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
  /** Optional `short_title` frontmatter value, falls back to title — sidebar */
  shortTitle: string
  /** Lifecycle status from the `status` frontmatter field */
  status: string
  /**
   * Optional `implementation` frontmatter value (e.g. "Partially implemented",
   * "Not implemented", "Implemented"). Only set when the field is present, so
   * a plain Accepted RFC carries no value and renders no second badge.
   */
  implementation?: string
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

/**
 * Read a scalar `key:` value from a frontmatter block. Handles the two shapes
 * our RFC frontmatter uses: a bare scalar (`status: Accepted`) and a quoted
 * scalar (`title: "RFC-001: …"`, `date: '2026-01-01'`). Block sequences and
 * nested maps are out of scope — this is the controlled frontmatter that the
 * content collection validates, not arbitrary YAML.
 */
function frontmatterField(block: string, key: string): string | undefined {
  const m = block.match(new RegExp(`^${key}:[ \\t]*(.+?)[ \\t]*$`, 'm'))
  if (!m) return undefined
  const raw = m[1].trim()
  // Strip a single layer of matching quotes.
  if (
    (raw.startsWith('"') && raw.endsWith('"')) ||
    (raw.startsWith("'") && raw.endsWith("'"))
  ) {
    return raw.slice(1, -1)
  }
  return raw
}

/**
 * Scan docs/rfcs for rfc-NNN.md files and parse number (from filename) and the
 * metadata fields (title, status, optional implementation and short title)
 * from frontmatter. Throws if a file lacks a parseable frontmatter title, so a
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
    const block = fm[1]
    const rawTitle = frontmatterField(block, 'title')
    if (!rawTitle) {
      throw new Error(
        `docs/rfcs/${filename}: no parseable 'title:' in frontmatter`,
      )
    }
    // Strip a leading "RFC-NNN: " prefix — the sidebar already prepends the
    // id, so the descriptive half is what we want stored as `title`.
    const title = rawTitle.replace(/^RFC-\d+:\s*/, '')
    const status = frontmatterField(block, 'status') ?? 'Unknown'
    const implementation = frontmatterField(block, 'implementation')
    const shortTitle = frontmatterField(block, 'short_title') ?? title

    // The link is derived from a filename we just read, so it provably
    // resolves to a real page: a non-existent target cannot be produced here.
    entries.push({
      num,
      id: `RFC-${String(num).padStart(3, '0')}`,
      title,
      shortTitle,
      status,
      ...(implementation ? { implementation } : {}),
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
