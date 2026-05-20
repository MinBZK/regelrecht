# Hooks and Reactive Execution

Some laws apply to every government decision without being called explicitly. The General Administrative Law Act (Algemene wet bestuursrecht, AWB) is the main example: whenever any government body issues a formal decision (*beschikking*), AWB rules about reasoning requirements, objection periods, and notification deadlines kick in automatically.

The law that triggers these rules does not know about the AWB. The AWB does not know about that specific law. They are decoupled by design.

## How hooks work

An article can declare a `hooks` block. This tells the engine: "fire this article whenever the conditions in `applies_to` match."

```yaml
# AWB 6:7 - sets the objection period to 6 weeks
machine_readable:
  hooks:
    - hook_point: post_actions
      applies_to:
        legal_character: BESCHIKKING
        stage: BESLUIT
  execution:
    output:
      - name: bezwaartermijn_weken
        type: number
    actions:
      - output: bezwaartermijn_weken
        value: 6
```

When the Zorgtoeslagwet calculates a healthcare allowance and produces a *beschikking*, this AWB hook fires and adds `bezwaartermijn_weken: 6` to the result. The Zorgtoeslagwet does not mention the AWB or objection periods.

## Hook points

Hooks can fire at two moments during article execution:

- **`pre_actions`** - after open term resolution, before the article's own logic runs. Used for prerequisites like the AWB reasoning requirement (3:46): the decision must include a motivation.
- **`post_actions`** - after the article's logic completes. Used for consequences like the objection period (6:7) and notification deadlines (6:8).

## Triggering hooks: the `produces` annotation

For hooks to fire, the target article must declare what kind of legal product it produces:

```yaml
# Zorgtoeslagwet, article 2
execution:
  produces:
    legal_character: BESCHIKKING
    decision_type: TOEKENNING
```

The engine builds a hook index at load time. When it encounters an article with a `produces` annotation, it checks the index for matching hooks and fires them.

## A real chain: objection deadline calculation

AWB hooks compose into a chain:

1. **AWB 3:46** (pre_actions hook, stage: BESLUIT) - sets `motivering_vereist: true`
2. The target law's article runs and produces the decision
3. **AWB 6:7** (post_actions hook, stage: BESLUIT) - sets `bezwaartermijn_weken: 6`
4. Later, when the decision is communicated (stage: BEKENDMAKING):
5. **AWB 6:8** (post_actions hook, stage: BEKENDMAKING) - calculates `bezwaartermijn_startdatum` and `bezwaartermijn_einddatum` based on the notification date and the 6-week period from AWB 6:7

```yaml
# AWB 6:8 - calculates start and end dates for the objection period
machine_readable:
  hooks:
    - hook_point: post_actions
      applies_to:
        legal_character: BESCHIKKING
        stage: BEKENDMAKING
  execution:
    parameters:
      - name: bekendmaking_datum
        type: date
        required: true
    input:
      - name: bezwaartermijn_weken
        type: number
        source:
          regulation: algemene_wet_bestuursrecht
          output: bezwaartermijn_weken
    output:
      - name: bezwaartermijn_startdatum
        type: date
      - name: bezwaartermijn_einddatum
        type: date
    actions:
      - output: bezwaartermijn_startdatum
        value:
          operation: DATE_ADD
          date: $bekendmaking_datum
          days: 1
      - output: bezwaartermijn_einddatum
        value:
          operation: DATE_ADD
          date: $bekendmaking_datum
          weeks: $bezwaartermijn_weken
```

## Overrides (lex specialis)

Sometimes a specific law overrides a general AWB rule. The Aliens Act (*Vreemdelingenwet*) article 69 says: *"in afwijking van artikel 6:7 Awb bedraagt de termijn vier weken"* ("departing from AWB article 6:7, the period is four weeks").

This is modeled with `overrides`:

```yaml
# Vreemdelingenwet, article 69
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

The AWB does not know it is being overridden. The Vreemdelingenwet unilaterally replaces the value. This only applies when the Vreemdelingenwet is part of the execution chain (a Participatiewet case is not affected by this override).

### How overrides differ from IoC

| | IoC (`open_terms` + `implements`) | Overrides |
|---|---|---|
| General law | Knows it delegates, declares `open_terms` | Does not know, declares nothing |
| Specific law | Declares `implements` | Declares `overrides` |
| Legal text | *"Gelet op artikel..."* | *"In afwijking van artikel..."* |
| Relationship | Bilateral (both sides participate) | Unilateral (only the overrider declares) |

## Administrative procedure stages

A *beschikking* is not an instant event. It moves through stages over time: application, review, decision, notification, objection. Hooks bind to specific stages via `applies_to.stage`.

The AWB defines a procedure lifecycle:

| Stage | Description | Example hook |
|-------|-------------|-------------|
| AANVRAAG | Application filed | |
| BEHANDELING | Under review | |
| BESLUIT | Decision taken | AWB 3:46 (reasoning), 6:7 (objection period) |
| BEKENDMAKING | Decision communicated | AWB 6:8 (deadline calculation) |
| BEZWAAR | Objection period | |

The engine yields between stages, returning accumulated outputs and indicating what inputs are needed for the next stage. The engine itself stays stateless; the procedure state is managed externally.

## Further reading

- [Cross-Law References](./cross-law-references) - how laws reference each other explicitly
- [Inversion of Control](./inversion-of-control) - how higher laws delegate to lower regulations
- [RFC-007: Cross-Law Execution](/rfcs/rfc-007) - hooks and overrides specification
- [RFC-008: Bestuursrecht/AWB](/rfcs/rfc-008) - the administrative procedure model
