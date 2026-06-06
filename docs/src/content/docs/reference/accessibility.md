---
title: Accessibility statement
description: Accessibility statement for RegelRecht (WCAG 2.1 AA, draft).
lang: en
---

This is an English translation, provided for readers of the English
documentation. The [Dutch version](/reference/toegankelijkheid) is the binding
one: it is the legally required form for a Dutch government site, and where the
two differ, the Dutch text prevails.

The statement describes how far RegelRecht meets the accessibility requirements
for government websites. The legal standard right now is WCAG 2.1 level AA, via
EN 301 549 and mandatory in the Netherlands under the Besluit digitale
toegankelijkheid overheid. WCAG 2.2 has been published by the W3C but is not yet
part of EN 301 549; once the European standard moves to 2.2, the Netherlands
follows. RegelRecht has therefore already tested against the nine new 2.2
criteria, ahead of that transition. The statement covers the site served at
`regelrecht.rijks.app` and at `docs.regelrecht.rijks.app`, so the landing page,
the sign-up form, and the documentation.

**This is a draft.** The status below rests on an automated test in the build
pipeline and a manual review by the team. No independent party has audited the
site, and the statement is not yet listed in the DigiToegankelijk register.
Until both of those are done, this is not a legally valid statement and
RegelRecht carries no accessibility label.

## Compliance status

Partial. The automated test finds no errors on the criteria it measures, and the
manual review covers the criteria no tool tests reliably. A few known limitations
remain, listed below. Without an external audit, RegelRecht claims no full
compliance.

## Language of the site

The documentation is mostly English; the landing page and the sign-up form are
Dutch. Every page carries a `lang` attribute that matches the language of its
content (`en` for the docs, `nl` for the landing and the Dutch statement), so a
screen reader picks the right pronunciation. Where a single term in the other
language appears within a page, that fragment gets its own `lang`. This covers
WCAG 3.1.1 (language of page) and 3.1.2 (language of parts).

## How this was tested

**Automated, on every change.** An accessibility test runs in CI on every change
that touches the documentation. It builds the site and runs
[pa11y-ci](https://github.com/pa11y/pa11y-ci) with two independent engines,
HTML_CodeSniffer and axe-core 4.11, against every generated page. The URL list
comes from the build, so a new page is tested automatically and does not slip
past the check. The test runs locally with `just docs-a11y`.

Those two engines cover a large part of what can be measured by machine:
contrast, form labels, heading structure, landmarks, and alt text. What a tool
does not see reliably was checked by hand.

**Manual, including a head start on 2.2.** The team went through the nine success
criteria that WCAG 2.2 adds, although they are not yet a legal requirement: focus
not obscured (minimum and enhanced), focus appearance, dragging movements, target
size, consistent help, redundant entry, and accessible authentication (minimum
and enhanced). Beyond that, hand-tested:

- keyboard-only operation, including the skip link ("Direct naar de inhoud")
  and the focus order;
- the focus indicator (a visible blue outline on every interactive element);
- the three themes (System, Light, Dark) via the theme menu, for contrast and
  legibility;
- rendering at 200% zoom and at 400% reflow (no horizontal scroll);
- spot-checked contrast ratios measured in the browser on the landing page
  (dark and light modes): headings 11.3–12.4:1, links 5.8–9.4:1, all well
  above the AA requirement. The documentation pages use the same NLDD
  palette but were not measured element by element.

## Known limitations

The automated contrast test flags a few elements whose contrast actually passes
comfortably. The engines cannot read the ratio of these elements reliably. The
team measured the real ratio in the browser and excluded these elements from the
automated check, with a note in the configuration:

- **Diagrams (Mermaid).** Diagrams render as inline SVG, with text on transparent
  layers where axe cannot read the color behind it. The measured ratio is 14.4:1
  for text in flow and state diagrams (dark blue on light blue) and well above the
  requirement for the C4 diagrams (white on dark blue). The diagram colors come
  from the NLDD palette.
- **Code samples.** Code blocks carry a light and a dark theme on the same
  element; the test mixes the two color sets and measures a blend that never
  appears on screen. The text that does appear clears the 4.5:1 that AA asks for
  in both modes.

The accessible name of each diagram (`role="img"` with an `aria-label`) is checked
separately against the build output in the same gate, apart from the excluded
contrast measurement.

## Not yet covered

- There has been **no independent audit**. Everything above rests on the team's
  own test and review.
- The interface uses web components from the NLDD design system that render their
  content in a shadow DOM. The `<main>` landmark is rendered by `nldd-page`
  inside that shadow DOM. Screen reader support for landmarks in shadow DOM
  still varies (NVDA and JAWS handle it well, VoiceOver on iOS is inconsistent);
  accessibility there depends on the design system and was not verified per
  screen reader by this team.

## Reporting a problem

Run into an accessibility problem, or something does not work? Email
[regelrecht@minbzk.nl](mailto:regelrecht@minbzk.nl). Describe what went wrong and
on which page, and we will pick it up.

## Drawn up

This draft statement was drawn up on 21 May 2026, updated on 27 May 2026 after a
re-check, and updated again on 29 May 2026 after the footer copyright text was
brought up to AA contrast in the NLDD design system. RegelRecht is an
exploration and still under development; the statement is updated when the site
changes or after a formal audit.
