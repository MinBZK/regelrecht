---
title: "Multi-Organization Execution"
description: "How the engine handles organizational boundaries, distinguishing formal decisions from calculations."
---

In practice, different government organizations handle different parts of the law chain. The Tax Authority determines your income. The Allowances Service determines your healthcare allowance. A municipality determines your social assistance.

When the engine follows a cross-law reference, it crosses organizational boundaries. The question becomes: should it compute the value itself, or accept another organization's authoritative determination?

## The legal distinction

Dutch administrative law distinguishes between two kinds of outputs:

- A **beschikking** (formal decision) is a legally binding act that only the competent authority can issue. The Tax Authority's income determination is a beschikking.
- A **berekening** (calculation) is a computation that anyone can perform. Multiplying two publicly known numbers does not require authority.

This distinction determines the execution boundary. A calculation can be run by anyone. A beschikking requires the right organization.

## How the engine decides

A law that produces a formal decision declares `competent_authority`. The engine reads it from the top level of the law (or from a `machine_readable` section); it is **not** a field of `execution.produces`. The JSON schema defines `competent_authority` on the `machine_readable` section, but the top-level form is accepted too (the top-level object does not reject extra keys), and that is what the corpus uses, often as a `#`-reference into the law's definitions (see `corpus/regulation/nl/wet/wet_op_de_zorgtoeslag/2025-01-01.yaml`):

```yaml
$id: wet_op_de_zorgtoeslag
competent_authority: '#bevoegd_gezag'   # top-level, resolved from definitions
# ...

# elsewhere, an article's execution declares what it produces:
articles:
  - number: "2"
    machine_readable:
      execution:
        produces:
          legal_character: BESCHIKKING
          decision_type: TOEKENNING
```

When the engine hits a cross-law reference and needs to resolve it, the intended decision tree is:

1. Does the target article produce a BESCHIKKING? If no: **execute locally** (it is just a calculation)
2. Is `competent_authority` declared? If no: **execute locally** with a warning
3. Is the competent authority the same organization running this engine? If yes: **execute locally**
4. Otherwise: **accept** the other organization's determination

Step 4 (accepting an external authority's determination) belongs to the federated modes below and is not yet implemented; in the current Solo mode the engine always executes locally.

## Execution modes

The design ([RFC-009](/rfcs/rfc-009)) describes four modes, based on two independent choices, **connectivity** (Solo or Federated) and **legal status** (Simulation or Authoritative):

| | Simulation | Authoritative |
|---|---|---|
| **Solo** (local only) | Execute everything locally. No identity, unsigned outputs. Good for exploring the full chain. | Execute with identity. Sign outputs. No cross-org calls. |
| **Federated** (cross-org) | Identity + real calls to other engines. Unsigned outputs. For testing inter-org flows. | Identity + real calls + signed outputs. Production mode. |

Today the engine implements only **Solo / Simulation**: it executes everything locally with no organizational identity and unsigned outputs. This is the default and is what runs in the browser editor.

The other three modes are the proposed end state. **Federated authoritative** is the eventual production mode, where each engine identifies as a specific organization, calls other organizations' engines through FSC (Federated Service Connectivity), and signs its outputs cryptographically. Federated connectivity, the Authoritative legal status, FSC calls, and output signing are not implemented yet.

## What the trace shows

In multi-org mode, the execution trace marks organizational boundaries. Each value shows whether it was:
- **Computed locally** (the running engine executed the logic)
- **Accepted from another authority** (the value came from an external engine, with a signature)

A citizen requesting their trace can see: "your income was determined by the Tax Authority (accepted, signed), your healthcare allowance was calculated by the Allowances Service (computed locally)."

## Further reading

- [Hooks and Reactive Execution](./hooks-and-reactive-execution) - how Awb rules apply across organizations
- [Federated Corpus](./federated-corpus) - how different organizations maintain their own law files
- [RFC-009: Multi-Org Execution](/rfcs/rfc-009) - the full design specification
