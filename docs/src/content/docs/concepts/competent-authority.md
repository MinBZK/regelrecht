---
title: "Competent Authority"
description: "How an article records the bevoegd gezag: the authority whose execution produces a binding decision, as a named organization or a category resolved per context."
---

A *beschikking* is only binding when the right body issues it. The Tax Authority assesses income; the CIZ decides on long-term care indications; a municipal college grants social assistance. An article records which body that is in `competent_authority`, the *bevoegd gezag*. This is what lets the engine tell a binding decision apart from a calculation anyone may run (see [Multi-Org Execution](./multi-org-execution)), and it is the anchor for deriving who may authoritatively annotate a law (see [Notes and Annotations](./notes-and-annotations)).

## The two forms

`competent_authority` sits in an article's `machine_readable` section and takes one of two shapes.

A named authority, when the body is fixed:

```yaml
machine_readable:
  competent_authority:
    name: Belastingdienst
    type: INSTANCE
```

`type` is either `INSTANCE` or `CATEGORY`, defaulting to `INSTANCE`:

- **`INSTANCE`** is one specific organization, named outright. Belastingdienst, CIZ, Minister van Volksgezondheid, Welzijn en Sport.
- **`CATEGORY`** is a categorical role that resolves to a different concrete body depending on context. "College van burgemeester en wethouders" names a kind of authority; which of the 342 colleges is meant depends on the municipality in scope. The distinction tells the engine whether the authority is settled by the article alone or still needs a contextual fact to pin down.

A reference to a computed output, when the authority itself depends on the case:

```yaml
machine_readable:
  competent_authority: '#bevoegd_gezag'
```

The `#` form points at an output computed elsewhere in the same law. The Wet langdurige zorg uses this: which body is competent (`2_1_3_bevoegd_gezag`) is itself a rule, so the article references the output rather than hard-coding a name.

## One law, several authorities

A single law can name a different authority per article, because different articles describe different acts. In the Wet langdurige zorg the indication decision is the CIZ's, while later articles assign other steps to the Zorgkantoor and the Wlz-uitvoerder. Each carries its own `competent_authority`. The authority is a property of the act in the article, not of the law as a whole.

## Further reading

- [Multi-Org Execution](./multi-org-execution) - how the competent authority decides execute-vs-accept
- [Notes and Annotations](./notes-and-annotations) - how note authority is derived from the competent authority
- [RFC-002: Bevoegdheid](/rfcs/rfc-002) - full specification
