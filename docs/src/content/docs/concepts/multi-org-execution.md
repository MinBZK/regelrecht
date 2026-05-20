# Multi-Organization Execution

In practice, different government organizations handle different parts of the law chain. The Tax Authority determines your income. The Allowances Service determines your healthcare allowance. A municipality determines your social assistance.

When the engine follows a cross-law reference, it crosses organizational boundaries. The question becomes: should it compute the value itself, or accept another organization's authoritative determination?

## The legal distinction

Dutch administrative law distinguishes between two kinds of outputs:

- A **beschikking** (formal decision) is a legally binding act that only the competent authority can issue. The Tax Authority's income determination is a beschikking.
- A **berekening** (calculation) is a computation that anyone can perform. Multiplying two publicly known numbers does not require authority.

This distinction determines the execution boundary. A calculation can be run by anyone. A beschikking requires the right organization.

## How the engine decides

Each article that produces a formal decision declares `competent_authority`:

```yaml
execution:
  produces:
    legal_character: BESCHIKKING
    decision_type: TOEKENNING
    competent_authority: belastingdienst_toeslagen
```

When the engine hits a cross-law reference and needs to resolve it, the decision tree is:

1. Does the target article produce a BESCHIKKING? If no: **execute locally** (it is just a calculation)
2. Is `competent_authority` declared? If no: **execute locally** with a warning
3. Is the competent authority the same organization running this engine? If yes: **execute locally**
4. Otherwise: **accept** the other organization's determination

## Execution modes

The engine supports four modes, based on two independent choices:

| | Simulation | Authoritative |
|---|---|---|
| **Solo** (local only) | Execute everything locally. No identity, unsigned outputs. Good for exploring the full chain. | Execute with identity. Sign outputs. No cross-org calls. |
| **Federated** (cross-org) | Identity + real calls to other engines. Unsigned outputs. For testing inter-org flows. | Identity + real calls + signed outputs. Production mode. |

**Solo simulation** is the default. Anyone can explore the full calculation chain for any law without needing credentials or organizational identity. This is what runs in the browser editor.

**Federated authoritative** is production mode. Each engine identifies as a specific organization, calls other organizations' engines through FSC (Federated Service Connectivity), and signs its outputs cryptographically.

## What the trace shows

In multi-org mode, the execution trace marks organizational boundaries. Each value shows whether it was:
- **Computed locally** (the running engine executed the logic)
- **Accepted from another authority** (the value came from an external engine, with a signature)

A citizen requesting their trace can see: "your income was determined by the Tax Authority (accepted, signed), your healthcare allowance was calculated by the Allowances Service (computed locally)."

## Further reading

- [Hooks and Reactive Execution](./hooks-and-reactive-execution) - how AWB rules apply across organizations
- [Federated Corpus](./federated-corpus) - how different organizations maintain their own law files
- [RFC-009: Multi-Org Execution](/rfcs/rfc-009) - the full design specification
