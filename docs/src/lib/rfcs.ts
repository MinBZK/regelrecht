// Single source of truth for the RFC list.
//
// Both the docs sidebar (via sidebar.ts) and the RFC index table
// (RfcIndexTable.astro) are generated from src/content/rfcs/rfc-*.md, so they
// cannot drift apart. This module is pure Node, so it runs at build time.

import { existsSync, readdirSync, readFileSync } from 'node:fs'
import { resolve } from 'node:path'
import { fileURLToPath } from 'node:url'
import GithubSlugger from 'github-slugger'

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
   * `implementation` frontmatter value: "Implemented", "Partially implemented",
   * or "Not implemented". Every RFC sets it (enforced by the content schema);
   * optional here only because this pure-Node parser also reads files the
   * schema does not validate.
   */
  implementation?: string
  /**
   * Raw `depends_on` frontmatter entries, each like "RFC-004 (Uniform Operation
   * Syntax)". Resolved into links by `rfcRelations()`.
   */
  dependsOn: string[]
  /** Site-relative link derived from the real filename, e.g. "/rfcs/rfc-008" */
  link: string
}

/** A resolved cross-reference to another RFC, ready to render as a link. */
export interface RfcLink {
  /** Numeric RFC number, e.g. 4 */
  num: number
  /** Zero-padded id, e.g. "RFC-004" */
  id: string
  /** Descriptive title, e.g. "Uniform Operation Syntax" */
  title: string
  /** Site-relative link, e.g. "/rfcs/rfc-004" */
  link: string
}

/**
 * Map an RFC status to an NDD tag colour. Shared by the index page and the RFC
 * page so the two cannot drift. Substring match keeps "Accepted (…)"-style
 * values working even though the schema now constrains status to single words.
 */
export function rfcStatusColor(status: string): string {
  const k = (status ?? '').toLowerCase()
  if (k.includes('accept')) return 'success'
  if (k.includes('propos')) return 'accent'
  if (k.includes('reject')) return 'critical'
  if (k.includes('supersed')) return 'warning'
  return 'neutral'
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
 * Read a `key:` block sequence (one `- item` per line) from a frontmatter
 * block. Returns [] when the key is absent. Stops at the next top-level key or
 * the end of the block, so it never bleeds into a following field.
 */
function frontmatterList(block: string, key: string): string[] {
  const lines = block.split('\n')
  const start = lines.findIndex((l) => l.trimEnd() === `${key}:`)
  if (start === -1) return []
  const items: string[] = []
  for (let i = start + 1; i < lines.length; i++) {
    const line = lines[i]
    const item = line.match(/^\s*-\s+(.*)$/)
    if (item) {
      items.push(item[1].trim())
    } else if (line.trim() !== '') {
      break // a non-list, non-blank line ends the sequence
    }
  }
  return items
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
    const dependsOn = frontmatterList(block, 'depends_on')

    // The link is derived from a filename we just read, so it provably
    // resolves to a real page: a non-existent target cannot be produced here.
    entries.push({
      num,
      id: `RFC-${String(num).padStart(3, '0')}`,
      title,
      shortTitle,
      status,
      ...(implementation ? { implementation } : {}),
      dependsOn,
      link: `/rfcs/${filename.replace(/\.md$/, '')}`,
    })
  }

  return entries.sort((a, b) => a.num - b.num)
}

/** The RFC number a `depends_on` entry points at, e.g. "RFC-004 (…)" → 4. */
const DEPENDS_ON_NUM = /^RFC-0*(\d+)\b/

/**
 * Forward and reverse dependency links per RFC, resolved against the real RFC
 * set so a target that does not exist is dropped rather than linked to a 404.
 * `dependsOn` comes from the `depends_on` frontmatter; `requiredBy` is its
 * inverse (the RFCs that declare a dependency on this one), computed here so
 * the back-references cannot drift from the forward ones.
 */
export function rfcRelations(): Map<
  number,
  { dependsOn: RfcLink[]; requiredBy: RfcLink[] }
> {
  const rfcs = getRfcs()
  const byNum = new Map(rfcs.map((r) => [r.num, r]))
  const toLink = (r: RfcEntry): RfcLink => ({
    num: r.num,
    id: r.id,
    title: r.title,
    link: r.link,
  })

  const relations = new Map<
    number,
    { dependsOn: RfcLink[]; requiredBy: RfcLink[] }
  >(rfcs.map((r) => [r.num, { dependsOn: [], requiredBy: [] }]))

  for (const rfc of rfcs) {
    for (const raw of rfc.dependsOn) {
      const targetNum = raw.match(DEPENDS_ON_NUM)?.[1]
      if (!targetNum) continue
      const target = byNum.get(parseInt(targetNum, 10))
      if (!target) continue // points at an RFC that is not in the corpus
      relations.get(rfc.num)!.dependsOn.push(toLink(target))
      relations.get(target.num)!.requiredBy.push(toLink(rfc))
    }
  }

  // Stable order: dependencies and dependents read low-to-high.
  for (const rel of relations.values()) {
    rel.dependsOn.sort((a, b) => a.num - b.num)
    rel.requiredBy.sort((a, b) => a.num - b.num)
  }
  return relations
}

/** Sidebar items for the `/rfcs/` section, with the Overview entry first. */
export function rfcSidebarItems(): { text: string; link: string }[] {
  return [
    { text: 'Overview', link: '/rfcs/' },
    ...getRfcs().map((r) => ({ text: `${r.id}: ${r.shortTitle}`, link: r.link })),
  ]
}

/** A cross-link target: the RFC page plus its section anchors. */
export interface RfcTarget {
  /** Zero-padded id, e.g. "RFC-008" */
  id: string
  /** Descriptive title, e.g. "Awb Administrative Procedures" */
  title: string
  /** Site-relative link to the page, e.g. "/rfcs/rfc-008" */
  link: string
  /**
   * Section number (as a reader writes it after `§`, normalised) → the heading
   * slug Astro generates. Normalisation removes the dots and lowercases, so
   * "§9", "§1.2" and "§A.8" key on "9", "12", "a8" — matching how
   * github-slugger collapses "9. …", "1.2. …" and "A.8 …" into ids.
   */
  sections: Map<string, string>
}

/**
 * Normalise a section number the way github-slugger collapses a heading's
 * leading numeric token: lowercase, drop dots and spaces. "1.2" → "12",
 * "A.8" → "a8", "9" → "9". Used on both sides (heading parse and `§`
 * reference) so the two cannot disagree.
 */
function normalizeSectionNumber(raw: string): string {
  return raw.toLowerCase().replace(/[.\s]/g, '')
}

// Leading section token of a heading, e.g. "9. Foo" → "9", "A.8 Bar" → "A.8",
// "1.2. Baz" → "1.2". Optional leading letter (appendix sections, optionally
// with its own dot as in "A.8"), then dot-separated digit groups, then a
// delimiter (the heading's own "." or the space before its title). Headings
// without a number (Context, Why) produce no section entry and are reachable
// only as the whole page.
const HEADING_SECTION = /^((?:[A-Za-z]\.?)?\d+(?:\.\d+)*)[.\s]/

/** Strip markdown inline syntax that would otherwise leak into a slug. */
function headingText(line: string): string {
  return line
    .replace(/^#{1,6}\s+/, '')
    .replace(/`([^`]*)`/g, '$1') // inline code: keep the contents
    .trim()
}

/**
 * Parse a single RFC file's body headings into a section-number → slug map,
 * using the same github-slugger instance semantics Astro applies (a fresh
 * slugger per document, so duplicate headings get -1/-2 suffixes in source
 * order). Only headings that begin with a section number get an entry; the
 * RFC page itself is always linkable without one.
 */
function parseSections(content: string): Map<string, string> {
  const slugger = new GithubSlugger()
  const sections = new Map<string, string>()
  // Strip frontmatter so its lines never feed the slugger and shift suffixes.
  const body = content.replace(FRONTMATTER, '')
  let inFence = false
  for (const line of body.split('\n')) {
    // Track fenced code blocks so a "# comment" inside one is not a heading.
    if (/^\s*(```|~~~)/.test(line)) {
      inFence = !inFence
      continue
    }
    if (inFence) continue
    if (!/^#{1,6}\s/.test(line)) continue
    const text = headingText(line)
    // Slug every heading in order so duplicates dedupe exactly as Astro does.
    const slug = slugger.slug(text)
    const num = text.match(HEADING_SECTION)?.[1]
    if (!num) continue
    const key = normalizeSectionNumber(num)
    // First heading wins for a given number; later collisions keep the slug
    // they already produced (and the slugger has handled the id dedupe).
    if (!sections.has(key)) sections.set(key, slug)
  }
  return sections
}

/**
 * Every RFC keyed by its number, with the data the cross-link rehype plugin
 * needs: the page link, the descriptive title (for link titles/aria), and the
 * section-anchor map so `§N` references resolve to the right heading. Pure
 * Node, runs at build time.
 */
export function rfcTargets(): Map<number, RfcTarget> {
  const targets = new Map<number, RfcTarget>()
  for (const rfc of getRfcs()) {
    const content = readFileSync(
      `${RFCS_DIR}/${rfc.link.replace('/rfcs/', '')}.md`,
      'utf-8',
    )
    targets.set(rfc.num, {
      id: rfc.id,
      title: rfc.title,
      link: rfc.link,
      sections: parseSections(content),
    })
  }
  return targets
}
