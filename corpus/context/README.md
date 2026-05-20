# Corpus Context

This directory holds domain knowledge that is not law but is needed to
execute law correctly. It sits alongside `corpus/regulation/` which
has the legally authoritative articles.

## The three-way split

```
corpus/
  regulation/    What the law says (normative, legally authoritative)
  context/       What the world looks like (factual domain knowledge)
```

Engine policy (how the engine processes law, layer hierarchy, type
coercions) is a separate concern. See RFC-015.

## Why this exists

Law articles reference concepts without defining them. The Algemene
termijnenwet says "de beide Kerstdagen" but never says "25 en 26
december." That's universal knowledge, not a legal provision.

When translating law to machine-readable format, we don't inject this
knowledge into the law translation. The translation must be faithful
to what the article says. The execution engine (and the BDD test
harness) need this knowledge to produce correct results, so it lives
here.

If it's not in the article text, it doesn't go in `machine_readable`.
It goes here.

## Contents

### `nl/calendar/`

One YAML file per year (2020-2035) with the 9 ATW-recognized public
holidays. Holiday names match the parameter names used by the
Algemene termijnenwet machine_readable sections.

Rules:
- Koningsdag shifts to April 26 when April 27 falls on Sunday
- Bevrijdingsdag is ATW-recognized every year for deadline extensions,
  even though it's only a public day off every 5 years
- Moveable holidays (Easter-dependent) are pre-computed from the
  computus

### `nl/hooks/`

Describes which legal concepts trigger which cross-cutting laws via
the hooks mechanism. Used by:
- Translators: to know what NOT to hardcode in law translations (if
  a law fires via hooks, target laws should not reference it)
- Engine (future): to validate hook configuration
- BDD tests: to understand expected hook chaining

When a new hook is defined in a law's machine_readable section, a
corresponding entry must be added to the hook register here.

## Authority and provenance

Context data has different epistemic status than enacted law. An
auditor can trace regulation files back to enacted law (Staatsblad,
etc.). Context files trace back to factual convention, doctrine, or
administrative practice.

Context files are versioned in git alongside regulations, giving them
the same audit trail.
