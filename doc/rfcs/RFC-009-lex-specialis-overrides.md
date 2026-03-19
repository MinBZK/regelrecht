# RFC-009: Lex Specialis Overrides

**Status:** Proposed
**Date:** 2026-03-16
**Authors:** Eelco Hotting

> **Note:** This RFC is under active research and may change significantly. The interaction between overrides, hooks (RFC-008), and temporal computation (RFC-011) is being explored. See RFC-011 (Temporal Computation) for the current design exploration.

## Context

Dutch law follows the principle of *lex specialis derogat legi generali*: a more specific law prevails over a more general one. This is an unwritten legal principle; the general law does not need to grant permission.

For example, AWB article 6:7 says "De termijn voor het indienen van een bezwaar- of beroepschrift bedraagt zes weken." No exception clause, no delegation. Yet the Vreemdelingenwet article 69 says "In afwijking van artikel 6:7 van de Algemene wet bestuursrecht bedraagt de termijn vier weken." It unilaterally replaces the value.

This differs from delegation (RFC-007 IoC):

| | IoC (RFC-007) | Lex specialis (this RFC) |
|---|---|---|
| General law | Declares `open_terms`, knows it is delegating | Declares nothing, does not know about overrides |
| Specific law | Declares `implements`, fills in a value left open | Declares `overrides`, replaces a value already set |
| Relationship | Declared by both (general declares `open_terms`, specific declares `implements`) | Declared by overrider only (general law is unaware) |
| Legal text | *"Gelet op artikel..."* | *"In afwijking van artikel..."* |

The engine currently has no way to declare lex specialis relationships. The override exists in the legal text but is invisible to the machine.

### Scope

Lex specialis overrides apply in both active and hook-augmented execution:

- **Active execution**: AWB article 6:7 sets the bezwaartermijn at 6 weeks. When the engine evaluates AWB 6:7 in the context of the Vreemdelingenwet, article 69 overrides it to 4 weeks.
- **Hook-augmented execution**: AWB article 6:7 hooks into any execution that produces a BESCHIKKING (see RFC-008). When Vreemdelingenwet is the contextual law, its article 69 overrides the bezwaartermijn that AWB 6:7 would produce from 6 to 4 weeks.

The `overrides` mechanism is independent of `hooks` (RFC-008). A law can use `overrides` without hooks, and vice versa. Their interaction: overrides affect a hook article's result, not its triggering condition.

## Decision

Introduce `overrides` on article-level `machine_readable`:

```yaml
# Vreemdelingenwet artikel 69
- number: '69'
  text: |-
    1. In afwijking van artikel 6:7 van de Algemene wet
       bestuursrecht bedraagt de termijn voor het indienen van
       een bezwaar- of beroepschrift vier weken.
  machine_readable:
    overrides:
      - law: algemene_wet_bestuursrecht
        article: '6:7'
        output: bezwaartermijn_weken
    execution:
      output:
        - name: bezwaartermijn_weken
          type: number
      actions:
        - output: bezwaartermijn_weken
          value: 4
```

The `overrides` declaration sits on the article where the legal text is. Article 69 says "in afwijking van", so article 69 carries the declaration.

### Structure

```yaml
overrides:
  - law: <target_law_id>        # the law being overridden
    article: <target_article>    # the article being overridden
    output: <output_name>        # the specific output being replaced
```

This parallels `implements`:

```yaml
implements:
  - law: <target_law_id>
    article: <target_article>
    open_term: <term_id>
```

Both point from the specific law to the general law. Both declare which article and which output/term. The difference is the nature of the relationship: filling in vs replacing.

### Contextual law

The **contextual law** is the law that initiated the current execution chain: the root of the call stack. When a citizen applies for a verblijfsvergunning, the Vreemdelingenwet is the contextual law. If execution crosses into AWB (e.g., to determine a beslistermijn), the contextual law remains the Vreemdelingenwet.

Only overrides declared in the contextual law apply. A Vreemdelingenwet override to AWB 6:7 does not affect cases initiated under the Participatiewet.

In a chain where law A calls law B which calls law C, the contextual law is always A. Intermediate laws on the stack do not contribute overrides unless they are themselves the contextual law in a separate execution.

When no contextual law is set (e.g., a standalone evaluation of AWB 6:7 via an API call without an originating law), no overrides apply. The target article executes with its own values.

Unlike IoC resolution (RFC-007), lex specialis does not require lex superior/lex posterior tiebreaking between the overrider and the target. The `overrides` declaration is itself the assertion of specificity — the overriding law explicitly states "in afwijking van." The engine does not verify whether the override is legally valid; that is a law authoring responsibility.

### Resolution model

#### Active execution

When the engine executes a law that has articles with `overrides` declarations:

1. **At load time**: the engine builds an `overrides_index`, keyed by `(target_law, target_article, output)`, mapping to a list of `(overriding_law, overriding_article)` entries
2. **At execution time**: when executing a target article (e.g., AWB 6:7), the engine queries the overrides index and filters by contextual law
3. **If an override exists**: execute the overriding article and use its value for the targeted output. Other outputs from the target article are preserved — the override replaces only the named output, not the entire article
4. **If no override exists**: execute the target article normally
5. **If multiple overrides exist** for the same `(target_law, target_article, output)` within the contextual law: this is a law authoring error (a single law should not have two articles both saying "in afwijking van" the same provision). The engine raises an error rather than silently picking one, consistent with RFC-007's ambiguity rule for IoC

#### Hook-augmented execution (RFC-008)

When a hook article fires (e.g., AWB 6:7 at `post_actions`), the override resolution follows the same logic as active execution. The contextual law still governs:

1. The engine fires AWB 6:7 as a `post_actions` hook on a BESCHIKKING result
2. Before executing AWB 6:7, the engine queries the `overrides_index` with the contextual law
3. If the contextual law has an override for AWB 6:7 `bezwaartermijn_weken`, the engine executes the overriding article instead
4. The overriding value enters the result instead of AWB 6:7's default

The contextual law does not change when a hook fires. The root of the call stack is always the contextual law.

### Example: active execution

When the engine evaluates AWB 6:7 directly (not via a hook) in the context of the Vreemdelingenwet:

```
1. Contextual law = vreemdelingenwet
2. Engine executes AWB 6:7
3. Queries overrides_index for (algemene_wet_bestuursrecht, 6:7, bezwaartermijn_weken)
4. Filters by contextual law → found: vreemdelingenwet art 69
5. Executes art 69 instead → bezwaartermijn_weken = 4
```

Without a contextual law (standalone evaluation), the override does not apply and AWB 6:7 returns 6.

### Example: hook-augmented execution (RFC-008)

```
Vreemdelingenwet:
  art 69: overrides AWB 6:7 bezwaartermijn_weken
  art 25: produces { legal_character: BESCHIKKING }

Engine executes Vreemdelingenwet art 25:
  1. Produces BESCHIKKING
  2. Queries hooks_index for post_actions + BESCHIKKING
  3. Finds AWB 6:7

Before executing AWB 6:7, engine queries overrides_index:
  4. Contextual law = vreemdelingenwet
  5. Target = (algemene_wet_bestuursrecht, 6:7, bezwaartermijn_weken)
  6. Found: vreemdelingenwet art 69

Engine executes Vreemdelingenwet art 69 instead of AWB 6:7 default:
  7. bezwaartermijn_weken = 4

Result to citizen:
  "U kunt binnen vier weken bezwaar maken
   (artikel 69 Vreemdelingenwet, in afwijking van
   artikel 6:7 Awb)"
```

## Why

### Benefits

The relationship "in afwijking van" becomes machine-readable. The engine can answer "if AWB 6:7 changes, which laws override it?" by querying the overrides index.

The override declaration lives on the article where the legal text is (article 69 says "in afwijking van", so article 69 carries the declaration), not on a distant besluit-producing article. This is consistent with how `implements` works: same direction (specific → general), different relationship type.

Works for both active and hook-augmented execution.

### Tradeoffs

The engine needs to know which law initiated the execution (the contextual law) to determine which overrides apply. A Vreemdelingenwet override should not affect a Participatiewet case.

Some provisions may be modelled as either IoC with defaults or as plain values subject to override. For example, AWB 4:13 says "bij wettelijk voorschrift bepaalde termijn" (anticipating that specific laws set their own), which is closer to delegation than lex specialis — better modelled as `open_terms` with a default. AWB 6:7 ("bedraagt zes weken", flat value, no anticipation of override) is the canonical lex specialis case.

### Alternatives Considered

**Alternative 1: Model as IoC (`open_terms` + `implements`)**
- AWB 6:7 declares `open_terms: bezwaartermijn_weken` with default 6.
- Vreemdelingenwet declares `implements` for that open term.
- Rejected: AWB 6:7 says "bedraagt zes weken". It sets a value, it does not delegate. Adding `open_terms` to AWB would misrepresent the legal text.

**Alternative 2: Override on the besluit-producing article**
- Vreemdelingenwet art 25 carries the `overrides` declaration.
- Rejected: the legal text for the override is in article 69, not article 25.

**Alternative 3: Top-down pull (PoC approach)**
- AWB checks `$WET.bezwaartermijn_weken` from the source law, falling back to a default.
- Rejected: tight coupling. AWB must know which fields might be overridden. The general law does not know about its overrides.

### Implementation Notes

- `overrides_index` in `RuleResolver`, keyed by `(target_law, target_article, output)`, mapping to a list of `(overriding_law, overriding_article)` entries. Filtered by contextual law at query time.
- The `overrides` mechanism is independent of `implements`. A law can use both.
- Scope: `overrides` from law X only apply in the context of law X.
- An article can both `implements` and `overrides`, e.g. a ministerial regulation that fills in a delegated rate and overrides an AWB termijn.
- **Contextual law**: requires threading a `contextual_law_id` through `ResolutionContext`. Set once at the root of the execution chain, immutable for the duration.
- **Cycle detection**: override resolution uses `ResolutionContext.visited` (same as IoC) to prevent A overriding B overriding A.
- **Temporal filtering**: override candidates are subject to the same temporal filtering as `implements` resolution. The engine selects the version of the overriding law valid for the calculation date. An override enacted in 2025 does not affect calculations for 2024.
- **Parameter forwarding**: the overriding article receives the same parameters that the target article would have received, filtered by `filter_parameters_for_article`.
- **Validation at load time**: the engine validates that the target `(law, article)` in an override declaration actually exists. Array size validation applies (consistent with RFC-007). Layer validation is deliberately omitted — lex specialis operates between laws at the same layer (e.g., WET overriding WET), unlike delegation which crosses layers.
- **Trace output**: new trace types `PathNodeType::OverrideResolution`, `ResolveType::Override` to show which override was applied and why.
- **`ResolutionContext` change**: add `contextual_law_id: Option<String>` field. This field does not exist in the current engine; it is a new addition required by this RFC.

## References

- RFC-007: Inversion of Control for Delegated Legislation (PR #246)
- RFC-008: Execution Lifecycle Hooks (companion RFC, `hooks` mechanism, this PR)
- AWB article 6:7: https://wetten.overheid.nl/BWBR0005537/2024-01-01#Artikel6:7
- Vreemdelingenwet article 69: https://wetten.overheid.nl/BWBR0011823/2024-01-01#Artikel69
