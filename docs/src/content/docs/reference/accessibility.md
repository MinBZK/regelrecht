---
title: Accessibility
description: Accessibility statement for RegelRecht (WCAG 2.2 AA, draft).
lang: en
---

# Accessibility statement

This is an English translation, provided for readers of the English
documentation. The [Dutch version](/reference/toegankelijkheid) is the binding
one: it is the legally required form for a Dutch government site, and where the
two differ, the Dutch text prevails.

The statement describes how far RegelRecht meets the accessibility requirements
for government websites. The standard is WCAG 2.2 level AA, which has applied
through EN 301 549 since October 2024 and is mandatory in the Netherlands under
the Tijdelijk besluit digitale toegankelijkheid overheid. It covers the site
served at `regelrecht.rijks.app` and at `docs.regelrecht.rijks.app`, so the
landing page, the sign-up form, and the documentation.

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
HTML_CodeSniffer and axe-core 4.11, against every generated page (61 at the
moment). The URL list comes from the build, so a new page is tested
automatically and does not slip past the check. The test runs locally with
`just docs-a11y`.

Those two engines cover a large part of WCAG 2.2 that can be measured by machine:
contrast, form labels, heading structure, landmarks, alt text, and some of the
criteria added in 2.2. What a tool does not see reliably was checked by hand.

**Manual, for the rest of 2.2.** The team went through the nine success criteria
that WCAG 2.2 adds: focus not obscured (minimum and enhanced), focus appearance,
dragging movements, target size, consistent help, redundant entry, and accessible
authentication (minimum and enhanced). Beyond that, hand-tested:

- keyboard-only operation, including the skip link and the focus order;
- dark mode, for contrast and legibility;
- rendering at 200% zoom and at 400% reflow.

## Known limitations

The automated contrast test flags a few elements whose contrast actually passes
comfortably. The engines cannot read the ratio of these elements reliably. The
team measured the real ratio in the browser and excluded these elements from the
automated check, with a note in the configuration:

- **Diagrams (Mermaid).** Diagrams render as inline SVG, with text on transparent
  layers where axe cannot read the colour behind it. The measured ratio is 14.4:1
  for text in flow and state diagrams (dark blue on light blue) and well above the
  requirement for the C4 diagrams (white on dark blue). The diagram colours come
  from the NLDD palette.
- **Code samples.** Code blocks carry a light and a dark theme on the same
  element; the test mixes the two colour sets and measures a blend that never
  appears on screen. The text that does appear clears the 4.5:1 that AA asks for
  in both modes.
- **The hero on the landing page.** The title text is white on a dark-blue
  gradient. axe cannot judge a gradient. The weakest point of the gradient, its
  lightest stop, gives white-on-dark-blue a ratio of 11.4:1; towards the dark end
  it rises to 15.5:1.

The accessible name of each diagram (`role="img"` with an `aria-label`) is checked
separately against the build output in the same gate, apart from the excluded
contrast measurement.

## Not yet covered

- There has been **no independent audit**. Everything above rests on the team's
  own test and review.
- The interface uses web components from the NLDD design system that render their
  content in a shadow DOM. Their own focus indication and internal accessibility
  fall outside the site's control and depend on the design system.

## Reporting a problem

Run into an accessibility problem, or something does not work? Email
[regelrecht@minbzk.nl](mailto:regelrecht@minbzk.nl). Describe what went wrong and
on which page, and we will pick it up.

## Drawn up

This draft statement was drawn up on 21 May 2026, based on the test and review of
that moment. RegelRecht is an exploration and still under development; the
statement is updated when the site changes or after a formal audit.
