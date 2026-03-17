# RFC-009: Lex Specialis Overrides

**Status:** Proposed
**Date:** 2026-03-16
**Authors:** Eelco Hotting

## Context

Dutch law follows the principle of *lex specialis derogat legi generali*: a more specific law prevails over a more general one. This is an unwritten legal principle; the general law does not need to grant permission.

For example, AWB article 6:7 says "De termijn voor het indienen van een bezwaar- of beroepschrift bedraagt zes weken." No exception clause, no delegation. Yet the Vreemdelingenwet article 69 says "In afwijking van artikel 6:7 van de Algemene wet bestuursrecht bedraagt de termijn vier weken." It unilaterally replaces the value.

This differs from delegation (RFC-007 IoC):

| | IoC (RFC-007) | Lex specialis (this RFC) |
|---|---|---|
| General law | Declares `open_terms`, knows it is delegating | Declares nothing, does not know about overrides |
| Specific law | Declares `implements`, fills in a value left open | Declares `overrides`, replaces a value already set |
| Relationship | Bilateral (both sides know) | Unilateral (only the overrider knows) |
| Legal text | *"Gelet op artikel..."* | *"In afwijking van artikel..."* |

The engine currently has no way to declare lex specialis relationships. The override exists in the legal text but is invisible to the machine.

### Scope

Lex specialis overrides apply in both active and hook-augmented execution:

- **Active execution**: AWB article 4:13 sets the beslistermijn for an aanvraag at 8 weeks. The Vreemdelingenwet overrides this to a shorter period. Someone applies, the engine determines the applicable termijn.
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

### Resolution model

#### Active execution

When the engine executes a law that has articles with `overrides` declarations:

1. **At load time**: the engine builds an `overrides_index`, keyed by `(target_law, target_article, output)`, mapping to a list of `(overriding_law, overriding_article)` entries
2. **At execution time**: when executing a target article (e.g., AWB 4:13), the engine queries the overrides index and filters by contextual law
3. **If an override exists**: execute the overriding article instead of using the target's value for that output
4. **If no override exists**: execute the target article normally

#### Hook-augmented execution (RFC-008)

When a hook article fires (e.g., AWB 6:7 at `post_actions`), the override resolution follows the same logic as active execution. The contextual law still governs:

1. The engine fires AWB 6:7 as a `post_actions` hook on a BESCHIKKING result
2. Before executing AWB 6:7, the engine queries the `overrides_index` with the contextual law
3. If the contextual law has an override for AWB 6:7 `bezwaartermijn_weken`, the engine executes the overriding article instead
4. The overriding value enters the result instead of AWB 6:7's default

The contextual law does not change when a hook fires. The root of the call stack is always the contextual law.

### Example: active execution

AWB article 4:13 sets the beslistermijn for government decisions on applications:

```yaml
# AWB artikel 4:13
- number: '4:13'
  text: |-
    1. Een beschikking dient te worden gegeven binnen de bij
       wettelijk voorschrift bepaalde termijn of, bij het
       ontbreken van zulk een termijn, binnen een redelijke
       termijn na ontvangst van de aanvraag.
    2. De in het eerste lid bedoelde redelijke termijn is in
       ieder geval verstreken wanneer het bestuursorgaan binnen
       acht weken na ontvangst van de aanvraag geen beschikking
       heeft gegeven, noch een kennisgeving als bedoeld in
       artikel 4:14, derde lid, heeft gedaan.
  machine_readable:
    execution:
      output:
        - name: redelijke_beslistermijn_weken
          type: number
      actions:
        - output: redelijke_beslistermijn_weken
          value: 8
```

Note: article 4:13 *does* say "bij wettelijk voorschrift bepaalde termijn", acknowledging that specific laws may set their own termijn. It does not use `open_terms` because it also sets a default ("redelijke termijn... acht weken"). This could be modelled as IoC with a default, or as a plain value subject to lex specialis override. Both are valid interpretations.

### Example: hook-augmented execution

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

Some provisions (like AWB 4:13) can be modelled as either IoC with defaults or as plain values subject to override. The RFC does not prescribe a single approach. Law modellers choose based on the legal text.

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

## References

- RFC-007: Inversion of Control for Delegated Legislation (PR #246)
- RFC-008: Execution Lifecycle Hooks (companion RFC, `hooks` mechanism, this PR)
- AWB article 6:7: https://wetten.overheid.nl/BWBR0005537/2024-01-01#Artikel6:7
- Vreemdelingenwet article 69: https://wetten.overheid.nl/BWBR0011823/2024-01-01#Artikel69
