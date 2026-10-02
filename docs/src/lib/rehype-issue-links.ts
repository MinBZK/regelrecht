import type { Root, Element, ElementContent, Text } from 'hast'

/**
 * Auto-link issue and pull-request references in rendered prose.
 *
 * The docs mention issues and PRs constantly ("see issue #444", "PR #748"), and
 * almost none of those mentions are markdown links: at the time of writing
 * there were 38 bare references against 2 hand-linked ones, and PR #704
 * appeared in both forms in the same corpus. Hand-linking does not survive
 * contact with people writing prose, so the build does it.
 *
 *   - "issue #1036"        → https://github.com/MinBZK/regelrecht/issues/1036
 *   - "PR #748"            → …/issues/748
 *   - "pull request #30"   → …/issues/30
 *
 * One URL shape covers both kinds. GitHub redirects an /issues/N URL to the
 * pull request when N happens to be a PR, which is exactly the question we
 * cannot answer here — see "No network" below.
 *
 * ## Only with a word in front
 *
 * A bare "#299" is deliberately NOT linked. The corpus uses `name#number` as a
 * law-node address — `awir#8`, `wet_op_de_zorgtoeslag#2`,
 * `regeling_standaardpremie#1` (RFC-026) — and a rule on `#\d+` would turn
 * legal addressing into broken GitHub links, in the very documents that explain
 * that notation. The cost is that "(see #299.)" stays plain text. That is the
 * right way to lean: a reference that is not a link is a small loss, a link
 * that points somewhere wrong is believed.
 *
 * ## No network
 *
 * There is deliberately no check that the number exists, and no fetch of the
 * title. GitHub's rate limit is one bucket shared by everything authenticating
 * as this account, so a build that calls the API can exhaust a quota a deploy
 * needs an hour later — and it makes `docs-build` fail when GitHub hiccups. An
 * author who writes a number is trusted for it, the same way the RFC plugin
 * trusts a section anchor it cannot verify.
 *
 * ## What it deliberately does NOT touch
 *
 *   - text already inside an <a> (hand-authored links stay as they are),
 *   - <code>/<pre> (a "#444" in a YAML example is content, not a link).
 *
 * Unlike rehype-rfc-links this runs on every page: an issue reference is just
 * as useful in a guide or a roadmap toelichting as in an RFC.
 */

const REPO_URL = 'https://github.com/MinBZK/regelrecht'

/*
 * "issue #1036", "Issue #1036", "PR #748", "pull request #30".
 *
 * The word is part of the match so it ends up inside the link text: prose then
 * reads exactly as written, with "issue #1036" underlined as one phrase rather
 * than a bare "#1036" hanging off it.
 *
 * `issues?` also covers "issues #1 and #2"-style plurals for the first number.
 * The lookahead keeps a sentence-ending dot or a closing paren out of the
 * match, so "(see issue #444.)" links "issue #444" and leaves ".)" alone.
 */
const REF = /\b(issues?|PRs?|pull requests?)\s+#(\d+)(?=[).,;:\s]|$)/gi

// Skip these subtrees entirely: existing links and code keep their text.
const SKIP_TAGS = new Set(['a', 'code', 'pre'])

function text(value: string): Text {
  return { type: 'text', value }
}

function anchor(href: string, label: string, nummer: string): Element {
  return {
    type: 'element',
    tagName: 'a',
    properties: {
      href,
      // The hover title says where it goes; the visible text stays what the
      // author wrote. No issue title here — that would need the API.
      title: `${label} op GitHub (#${nummer})`,
      className: ['issue-ref'],
    },
    children: [text(label)],
  }
}

/**
 * Build the replacement nodes for one text value: the parts between matches
 * stay as text, each match becomes an <a>. Returns null when there is nothing
 * to link, so callers can leave the original node untouched.
 */
function linkify(value: string): ElementContent[] | null {
  const out: ElementContent[] = []
  let last = 0
  let linked = false

  for (const m of value.matchAll(REF)) {
    const [whole, , nummer] = m
    const start = m.index ?? 0

    if (start > last) out.push(text(value.slice(last, start)))
    out.push(anchor(`${REPO_URL}/issues/${nummer}`, whole, nummer))
    last = start + whole.length
    linked = true
  }

  if (!linked) return null
  if (last < value.length) out.push(text(value.slice(last)))
  return out
}

export function rehypeIssueLinks() {
  return (tree: Root) => {
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
        const replacement = linkify(child.value)
        if (!replacement) continue
        children.splice(i, 1, ...replacement)
        i += replacement.length - 1 // skip over the nodes we just inserted
      }
    }

    walk(tree as unknown as { children?: ElementContent[] })
  }
}

export default rehypeIssueLinks
