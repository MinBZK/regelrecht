# RFC-008: Execution Lifecycle Hooks

**Status:** Proposed
**Date:** 2026-03-16
**Authors:** Eelco Hotting

## Context

The engine currently supports **active execution**: someone requests a legal determination, the engine evaluates with specific parameters, and produces a result. This covers laws like the Zorgtoeslagwet, Participatiewet, and BW5.

But a large class of law operates differently. The Algemene wet bestuursrecht (AWB) applies whenever any government body issues a beschikking. Article 6:7 sets a bezwaartermijn of six weeks. Article 3:46 requires a deugdelijke motivering. Neither article is called explicitly. They fire because the output qualifies as a beschikking, regardless of which law produced it.

The target law (Zorgtoeslag, Participatiewet) does not know about AWB. AWB does not know about the target law. The relationship is unilateral from AWB's side, triggered by a property of the output.

This is **reactive execution**: law that augments other law's execution without either side declaring a bilateral relationship.

### Execution modes

This is one of four identified execution modes:

1. **Active execution**: request-response (current engine, RFC-007 IoC)
2. **Reactive execution**: lifecycle hooks (this RFC)
3. **Generative execution**: law that creates other law (git workflow, out of scope)
4. **Verificative execution**: continuous invariant checking (out of scope)

The specification (YAML) is the same across all modes. The modes differ in triggering mechanism and required infrastructure.

### Current state

The engine has no way for a law to inject itself into another law's execution. The PoC (`poc-machine-law`) has reactive behaviour via event sourcing with a `Case` aggregate, but this couples the law specification to infrastructure (aggregates, event types, update methods).

Schema v0.4.0 already has a `produces` block on `execution` with `legal_character` and `decision_type` annotations. These annotations classify the output but the engine does not act on them at runtime. This RFC builds on `produces` as the filter target for hooks.

## Decision

The engine has a defined execution lifecycle with two observable points. Laws can register hooks at these points. When a hook's filter matches the executing article's `produces` annotation, the hook fires and its outputs enrich the result.

### Execution lifecycle

The engine evaluates an article in five stages:

1. **Create context** with parameters and calculation_date
2. **Resolve inputs** from cross-law references and data sources
3. **Resolve open terms** via IoC (RFC-007, `implements` index)
4. **Execute actions** that evaluate conditions and set outputs
5. **Return result** with outputs

In the Rust codebase: `RuleContext::new()`, `resolve_inputs_with_service()`, `resolve_open_terms()`, `ArticleEngine::evaluate_with_trace()`, return `ArticleResult`.

### Hook points

Two hook points interleave with this lifecycle:

| Hook point | Fires between stages | Context available to hook |
|---|---|---|
| `pre_actions` | 3 and 4 | Parameters, inputs, resolved open terms |
| `post_actions` | 4 and 5 | Parameters, inputs, open terms, outputs |

#### Why only two hook points

Earlier drafts included `pre_input` (between stages 1-2) and `post_input` (between stages 2-3). These were removed because the legal requirements that operate on the input phase tend to be either application-layer concerns (authorization per AWB 2:1), authoring-time concerns (data minimization per AVG art. 5), or process-management concerns (completeness checks per AWB 4:5 with hersteltermijn). None of these map cleanly to a runtime engine hook. The strongest candidate — AWB 4:5's input completeness check — belongs in the pipeline/orchestration layer where process state (notification deadlines, hersteltermijn) is tracked, not in the stateless execution engine.

The two remaining hook points cover the compelling use cases: `pre_actions` for decision requirements (motiveringsplicht, AWB 3:46) and `post_actions` for procedural consequences (bezwaartermijn, AWB 6:7). Input-phase hooks can be added as a backward-compatible extension if a concrete engine-level need emerges.

### YAML construct

Introduce `hooks` on article-level `machine_readable`:

```yaml
# AWB artikel 6:7
- number: '6:7'
  text: |-
    De termijn voor het indienen van een bezwaar- of
    beroepschrift bedraagt zes weken.
  machine_readable:
    hooks:
      - hook_point: post_actions
        applies_to:
          legal_character: BESCHIKKING
    execution:
      output:
        - name: bezwaartermijn_weken
          type: number
      actions:
        - output: bezwaartermijn_weken
          value: 6
```

The `hooks` block is a list. Each entry has:

- `hook_point`: one of `pre_actions`, `post_actions`
- `applies_to`: filter predicate matched against the executing article's `produces` block

Available filter fields:

| Filter field | Matches against |
|---|---|
| `legal_character` | `execution.produces.legal_character` |
| `decision_type` | `execution.produces.decision_type` |

When multiple filter fields are present, they are AND-combined. An article with `produces: { legal_character: BESCHIKKING, decision_type: TOEKENNING }` matches a hook with `applies_to: { legal_character: BESCHIKKING }` but also a more specific hook with `applies_to: { legal_character: BESCHIKKING, decision_type: TOEKENNING }`.

### Resolution model

#### At load time

The engine builds a `hooks_index` when loading laws. For each article with a `hooks` declaration, it indexes the hook by `(hook_point, legal_character)`, mapping to a list of `(law_id, article_number, HookFilter)` entries. This parallels `implements_index` (RFC-007) in `RuleResolver`.

At query time, the engine looks up by `(hook_point, legal_character)` and then post-filters candidates by `decision_type` if the hook's `applies_to` specifies one. This avoids requiring exact-match on `Option` fields while keeping the common case (filter by `legal_character` only) fast.

#### At execution time

When the engine executes an article that has a `produces` annotation:

1. Resolve inputs (cross-law references, data sources).
2. Resolve open terms (IoC, RFC-007).
3. Query `hooks_index` for `pre_actions` hooks matching the article's `produces`. Fire matching hooks. Their outputs enter the execution context.
4. Execute the article's own actions.
5. Query `hooks_index` for `post_actions` hooks matching the article's `produces`. Fire matching hooks. Their outputs are merged into the `ArticleResult`.

Hook articles are executed as ordinary article evaluations. They produce outputs.

#### Parameter passing

Hook articles do not receive the target article's execution context as input parameters. Each hook article declares its own `parameters` and `input` sections (or none, for constant-producing hooks like AWB 3:46 and AWB 6:7). The engine passes only the parameters declared in the hook article's `execution.parameters` section, consistent with RFC-007's principle of least privilege (`filter_parameters_for_article`).

This means:
- **Constant hooks** (no parameters declared): execute standalone, producing fixed values. Most AWB hooks fall in this category.
- **Context-aware hooks** (parameters declared): receive only the parameters they explicitly request. For example, a hook that needs `bsn` to look up audit data declares `bsn` as a parameter.

#### Priority

When multiple hooks produce the same output name, the engine resolves by lex superior (higher regulatory layer wins) then lex posterior (newer `valid_from` wins). This is the same priority model as IoC resolution (RFC-007).

#### Execution order

When multiple hooks match at the same hook point, they execute independently — there are no inter-hook dependencies. The engine does not guarantee a specific execution order among hooks at the same point. If hook A's output is needed by hook B, they must be at different hook points (e.g., A at `pre_actions`, B at `post_actions`).

#### Interaction with overrides (RFC-009, conditional)

If RFC-009 (Lex Specialis Overrides) is accepted: when a hook article fires, it is subject to the same override resolution as any other article. If the contextual law has an `overrides` declaration targeting the hook article's output, the override applies. The contextual law does not change when a hook fires; it remains the root of the call stack.

Example: AWB 6:7 fires as a `post_actions` hook. The contextual law is the Vreemdelingenwet. Vreemdelingenwet article 69 overrides AWB 6:7's `bezwaartermijn_weken` from 6 to 4. The citizen sees 4 weken.

Without RFC-009, hooks fire and produce their default values unconditionally. The override interaction is an enhancement, not a prerequisite for hooks to function.

### Full YAML example

AWB articles that hook into any beschikking:

```yaml
---
$id: algemene_wet_bestuursrecht
regulatory_layer: WET
publication_date: '2024-01-01'
valid_from: '1994-01-01'
articles:
  - number: '3:46'
    text: |-
      Een besluit dient te berusten op een deugdelijke motivering.
    machine_readable:
      hooks:
        - hook_point: pre_actions
          applies_to:
            legal_character: BESCHIKKING
      execution:
        output:
          - name: motivering_vereist
            type: boolean
        actions:
          - output: motivering_vereist
            value: true

  - number: '6:7'
    text: |-
      De termijn voor het indienen van een bezwaar- of
      beroepschrift bedraagt zes weken.
    machine_readable:
      hooks:
        - hook_point: post_actions
          applies_to:
            legal_character: BESCHIKKING
      execution:
        output:
          - name: bezwaartermijn_weken
            type: number
        actions:
          - output: bezwaartermijn_weken
            value: 6
```

Zorgtoeslag article that produces a beschikking:

```yaml
---
$id: wet_op_de_zorgtoeslag
regulatory_layer: WET
publication_date: '2024-01-01'
articles:
  - number: '2'
    text: |-
      Aanspraak op een zorgtoeslag heeft degene...
    machine_readable:
      execution:
        produces:
          legal_character: BESCHIKKING
          decision_type: TOEKENNING
        parameters:
          - name: toetsingsinkomen
            type: number
            required: true
          - name: drempelinkomen
            type: number
            required: true
        output:
          - name: heeft_recht_op_zorgtoeslag
            type: boolean
        actions:
          - output: heeft_recht_op_zorgtoeslag
            value:
              operation: LESS_THAN_OR_EQUAL
              subject: $toetsingsinkomen
              value: $drempelinkomen
```

### Walk-through

When the engine executes Zorgtoeslag article 2:

```
1. Create context: { toetsingsinkomen: 28000, drempelinkomen: 38520 }
2. Inspect produces: { legal_character: BESCHIKKING }
3. Resolve inputs → none declared
4. Resolve open terms → none declared
5. Query hooks_index for pre_actions + BESCHIKKING:
   → AWB 3:46 matches
   → Execute AWB 3:46 → { motivering_vereist: true }
   → Add to context
6. Execute Zorgtoeslag art 2 actions:
   → { heeft_recht_op_zorgtoeslag: true }
7. Query hooks_index for post_actions + BESCHIKKING:
   → AWB 6:7 matches
   → Execute AWB 6:7 → { bezwaartermijn_weken: 6 }
8. Merge into result

Final ArticleResult outputs:
  heeft_recht_op_zorgtoeslag: true
  motivering_vereist: true
  bezwaartermijn_weken: 6
```

The Zorgtoeslag YAML declares nothing about AWB. AWB declares nothing about Zorgtoeslag. The relationship exists purely through `produces` on the target side and `applies_to` on the hook side.

### External processes

Some hook results represent obligations that trigger separate processes: sending a notification, starting a bezwaar procedure, logging for audit. The engine does not implement external process triggering. The annotated result (with hook outputs included) is available to the orchestration layer, which decides what external actions to take. This is out of scope for this RFC.

## Why

### Benefits

AWB 6:7 fires on every beschikking. The schema captures this directly through `hooks` and `applies_to`, without external configuration or a maintained event vocabulary.

The engine can answer "which articles hook into BESCHIKKING executions?" by querying its `hooks_index`. This supports impact analysis when law changes.

The mechanism is general. Any article with a `produces` annotation can be hooked into. Adding a new hookable law requires only adding `produces` to its articles, not modifying AWB or any other hooking law.

The declaration is unilateral from the hook side: the target law is not modified. This matches the legal reality where AWB applies to all besluiten without each specific law needing to acknowledge AWB.

### Tradeoffs

Every execution of an article with `produces` requires querying the `hooks_index` at two points. For articles without `produces`, there is no overhead.

When multiple hooks produce the same output name, priority resolution adds complexity. The lex superior / lex posterior model is proven (RFC-007 uses it for IoC), but the interaction between hook priority and override priority needs careful implementation.

A law that does not annotate its articles with `produces` cannot be hooked into. This is by design (explicit opt-in through annotation), but means that unannotated laws are invisible to hooks.

### Alternatives Considered

**Alternative 1: Event-bus model (original RFC-008)**
- Laws declare `reacts_to: event_type` and `produces: event_type`. The engine annotates results with event metadata. An orchestration layer routes events.
- Rejected: couples law specification to an event vocabulary (`besluit`, `bezwaarschrift`, `aanvraag`) that needs maintenance, and to an orchestration routing mechanism the engine should not own.

**Alternative 2: Bilateral declaration (IoC-style)**
- Both the target law and the hooking law declare the relationship.
- Rejected: AWB is a general law. Requiring every specific law to declare "AWB hooks into me" inverts the legal hierarchy. AWB's generality is the whole point.

**Alternative 3: Service-layer middleware**
- The service layer always runs AWB after any beschikking, hardcoded outside the law specification.
- Rejected: makes the reactive relationship invisible in the law YAML. The machine-readable version should capture what the legal text says.

### Implementation Notes

- New structs in `article.rs`: `HookDeclaration { hook_point: HookPoint, applies_to: HookFilter }`, enum `HookPoint { PreActions, PostActions }`, struct `HookFilter { legal_character: Option<String>, decision_type: Option<String> }`.
- `hooks_index` in `RuleResolver`, keyed by `(HookPoint, String)` where the String is `legal_character`, mapping to `Vec<(law_id, article_number, HookFilter)>`. Post-filtered by `decision_type` at query time. Built during `load_law()`.
- Hook firing in `LawExecutionService::evaluate_article_with_service()`, at the `pre_actions` and `post_actions` stages. The method already has clear separation between stages (set definitions, resolve inputs, resolve open terms, execute actions).
- Hook articles execute through the same `evaluate_article_with_service` path, with cycle detection via `ResolutionContext.visited`.
- New trace types: `PathNodeType::HookResolution`, `ResolveType::Hook`.

## References

- RFC-007: Inversion of Control for Delegated Legislation (PR #246)
- RFC-009: Lex Specialis Overrides (companion RFC, `overrides` mechanism, this PR)
- AWB article 3:46: https://wetten.overheid.nl/BWBR0005537/2024-01-01#Artikel3:46
- AWB article 6:7: https://wetten.overheid.nl/BWBR0005537/2024-01-01#Artikel6:7
