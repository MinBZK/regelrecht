import type { Root, Element, ElementContent, Text } from 'hast'
import type { VFile } from 'vfile'
import { rfcTargets, type RfcTarget } from './rfcs.ts'

/**
 * Auto-link RFC cross-references in rendered RFC bodies.
 *
 * RFCs mention each other constantly ("RFC-008", "RFC-001 §9", "RFC-008 §A.8"),
 * but most mentions are plain prose, not markdown links. Hand-linking each one
 * would be hundreds of edits across the corpus and would rot the moment a
 * heading is renumbered. This plugin instead scans the rendered text at build
 * time and wraps every bare reference in an <a>:
 *
 *   - "RFC-008"        → /rfcs/rfc-008
 *   - "RFC-001 §9"     → /rfcs/rfc-001#9-input-source-consolidation
 *   - "RFC-008 §A.8"   → /rfcs/rfc-008#a8-uniforme-…
 *
 * Section anchors come from rfcs.ts (rfcTargets), which parses each target
 * RFC's own headings with the same github-slugger Astro uses, so "§N" resolves
 * to the real heading id. A "§N" with no matching heading falls back to the
 * page link (the number still shows, just without a fragment).
 *
 * What it deliberately does NOT touch:
 *   - text already inside an <a> (hand-authored markdown links stay as-is),
 *   - <code>/<pre> (an "RFC-007" inside a YAML example is content, not a link),
 *   - a bare self-reference (an RFC linking to its own page top is noise) —
 *     but a self-reference WITH a §section becomes an in-page anchor jump.
 *
 * Runs on RFC pages only (keyed off the source path under content/rfcs); a
 * stray "RFC-008" elsewhere in the docs is left alone.
 */

// "RFC-008" or "RFC-8" (zero-padding optional), optionally followed by a
// section reference: "§9", "§1.2", "§A.8". The reference glyph is U+00A7. The
// section token is an optional leading letter (appendix sections, optionally
// with its own dot as in "A.8") then dot-separated digit groups — the same
// shape rfcs.ts keys on. Dots are only consumed *between* digits, so a
// sentence-ending dot ("§5.") is never swallowed and "§1.2" keeps its dot.
const REF =
  /RFC-0*(\d+)(\s*§\s*((?:[A-Za-z]\.?)?\d+(?:\.\d+)*))?(?=[).,;:\s]|$)/g

// Skip these subtrees entirely: existing links and code keep their text.
const SKIP_TAGS = new Set(['a', 'code', 'pre'])

/** Current RFC number from the source file path, or null for non-RFC pages. */
function currentRfcNum(file: VFile | undefined): number | null {
  const path = file?.path ?? ''
  const m = path.match(/rfc-(\d+)\.md$/)
  return m ? parseInt(m[1], 10) : null
}

/** github-slugger collapses "1.2"→"12", "A.8"→"a8"; mirror that for lookup. */
function sectionKey(raw: string): string {
  return raw.toLowerCase().replace(/[.\s]/g, '')
}

/**
 * Build the replacement nodes for one text value: the parts between matches
 * stay as text, each match becomes an <a> (or, for an unlinkable self-ref,
 * stays as text). Returns null when there is nothing to link, so callers can
 * leave the original node untouched.
 */
function linkify(
  value: string,
  targets: Map<number, RfcTarget>,
  selfNum: number | null,
): ElementContent[] | null {
  const out: ElementContent[] = []
  let last = 0
  let linked = false

  for (const m of value.matchAll(REF)) {
    const [whole, numStr, , sectionRaw] = m
    const start = m.index ?? 0
    const num = parseInt(numStr, 10)
    const target = targets.get(num)

    // Unknown RFC number: leave the text as-is (no link to a 404).
    if (!target) continue

    const section = sectionRaw
      ? target.sections.get(sectionKey(sectionRaw))
      : undefined

    // A self-reference to the page top is noise; a self-reference to a section
    // is a useful in-page jump. Drop the former, keep the latter.
    const isSelf = num === selfNum
    if (isSelf && !section) continue

    const href = section
      ? `${isSelf ? '' : target.link}#${section}`
      : target.link

    // Emit the gap before this match, then the link.
    if (start > last) out.push(text(value.slice(last, start)))
    out.push(anchor(href, whole, target, sectionRaw))
    last = start + whole.length
    linked = true
  }

  if (!linked) return null
  if (last < value.length) out.push(text(value.slice(last)))
  return out
}

function text(value: string): Text {
  return { type: 'text', value }
}

function anchor(
  href: string,
  label: string,
  target: RfcTarget,
  sectionRaw: string | undefined,
): Element {
  // Title gives the descriptive name on hover; the visible text stays exactly
  // what the author wrote ("RFC-008 §A.8"), so prose reads unchanged.
  const title = sectionRaw
    ? `${target.id} §${sectionRaw} — ${target.title}`
    : `${target.id} — ${target.title}`
  return {
    type: 'element',
    tagName: 'a',
    properties: { href, title, className: ['rfc-ref'] },
    children: [text(label)],
  }
}

export function rehypeRfcLinks() {
  return (tree: Root, file: VFile) => {
    const selfNum = currentRfcNum(file)
    // Only RFC source files carry RFC cross-references worth auto-linking.
    if (selfNum === null) return
    const targets = rfcTargets()

    const walk = (parent: { children?: ElementContent[] }) => {
      const children = parent.children
      if (!children) return
      for (let i = 0; i < children.length; i++) {
        const child = children[i]
        if (child.type === 'element') {
          if (SKIP_TAGS.has(child.tagName)) continue
          walk(child)
          continue
        }
        if (child.type !== 'text') continue
        const replacement = linkify(child.value, targets, selfNum)
        if (!replacement) continue
        children.splice(i, 1, ...replacement)
        i += replacement.length - 1 // skip over the nodes we just inserted
      }
    }

    walk(tree as unknown as { children?: ElementContent[] })
  }
}

export default rehypeRfcLinks
