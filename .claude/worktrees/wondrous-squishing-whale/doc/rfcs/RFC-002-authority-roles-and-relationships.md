# RFC-002: Bevoegdheid (Authority) in Machine-Readable Law

**Status:** Proposed
**Date:** 2025-11-21
**Authors:** Tim de Jager

## Context

When representing laws and regulations in machine-readable format, we need to capture who has the authority (*bevoegdheid*) to make binding decisions.

### Observed Patterns

**In Wet op de Zorgtoeslag (BWBR0018451):**
- Article 1a defines "Onze Minister" (responsible minister)
- Article 5 defines "Dienst Toeslagen" as executing authority

**In Regeling Standaardpremie (BWBR0050536):**
- Preamble defines the Minister as both issuer and executor

**In Wet langdurige zorg (BWBR0035917):**
- Article 2.1.3: Sociale verzekeringsbank — insurance determination (*vaststelling verzekering*)
- Article 3.2.3: CIZ — indication decision (*indicatiebesluit*)
- Article 3.3.3: Zorgkantoor — personal care budget (*persoonsgebonden budget*)
- Article 4.2.1: Wlz-uitvoerder — duty of care (*zorgplicht*)
- Article 5.1.1: Zorginstituut Nederland — supervision (*toezicht*)
- Article 6.1.1: CAK
- Article 7.1.1: CIZ

### The Problem

One law can have **multiple** competent authorities for different actions. For example, a law might grant authority to:
- A minister for policy decisions
- An agency for individual decisions (*beschikkingen*)
- A different body for appeals

Additionally, some authorities are **categorical** rather than specific. For example, the College of Mayor and Aldermen (*college van burgemeester en wethouders*) applies to all 340+ municipalities, not a single entity.

## Decision

### 1. Article Level

Define `competent_authority` at the **article level** (in `machine_readable`), not at document top-level.

### 2. Object Structure

`competent_authority` is an object with:
- `name`: name of the authority (required)
- `type`: enum `INSTANCE` or `CATEGORY` (optional, defaults to `INSTANCE`)

```yaml
# Specific authority (instance) - type can be omitted
competent_authority:
  name: "Dienst Toeslagen"

# Categorical authority - type must be explicit
competent_authority:
  name: "college van burgemeester en wethouders"
  type: CATEGORY
```

### 3. Type Definitions

- **`INSTANCE`**: A specific organization (Dienst Toeslagen, CIZ, CAK, Sociale verzekeringsbank)
- **`CATEGORY`**: A category that must be resolved per context (College of Mayor and Aldermen (*college van B&W*), municipal council (*gemeenteraad*), provincial executive (*gedeputeerde staten*))

## Why

**Article-level authority makes sense because:**
- One law can have 0..n competent authorities
- Different articles may have different authorities
- The law text itself defines where authority is granted

**Object structure with type because:**
- Distinguishes between specific instances and categories
- Explicit about what needs runtime resolution

**Skip identifiers (OIN, TOOI) for now because:**
- Multiple identifier systems exist
- Name is sufficient for MVP
- Can add identifiers later when requirements are clearer

## Open Questions

- How to handle delegation of authority (*mandaat/delegatie*)?
- How to handle `#` references for `competent_authority`? When `competent_authority: '#bevoegd_gezag'` references an output, the `type` (INSTANCE/CATEGORY) must be specified separately. This is awkward because the type isn't part of the law being modeled - it's metadata we're adding.

## References

- Issue #7: Good enough Language for 1st fase Editor and Engine
- PR #30 Discussion
- Art. 1:1 Awb (definition of administrative body (*bestuursorgaan*))
- [Glossary of Dutch Legal Terms](../glossary.md)
