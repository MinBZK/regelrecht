# Law Generate, the validate-and-test loop

Follow this file only when you have a shell. In the enrichment pipeline you do
not, and the worker validates the file after you write it.

## Capture the BDD baseline first

**Before modifying the law file**, capture the current BDD state so you can tell
pre-existing failures apart from ones you introduced:

```bash
just bdd 2>&1 | tail -100
```

Note the summary line and any failures. That is your reference for every later run.

## Validate

```bash
just validate <file_path>
```

- Passes: go on to the BDD run.
- Fails: repair, at most twice per iteration. Read the error output, fix the
  broken article or field, re-run. Still failing after two repair rounds: stop
  and report the validation errors. Do not run BDD against schema-invalid YAML;
  the failures look like logic bugs and cost you iterations on the wrong problem.

## Run the BDD suites

```bash
just bdd
```

This runs both buckets: the law-validation scenarios next to the live laws
(`corpus/regulation/**/scenarios/*.feature`) and the engine-conformance suite
(`bdd/conformance/*.feature`). Only investigate failures that are new compared
to the baseline. Pre-existing failures from other laws are not yours to fix.

The Gherkin vocabulary is generated from `bdd/grammar.yaml`; step bindings are
code-generated (`just bdd-codegen`). Never hand-edit a generated file, and
prefer an existing step over a new one. When a scenario genuinely needs a step
the grammar does not have, change `grammar.yaml` and regenerate.

Read the existing steps before you write anything:
`packages/engine/tests/bdd/steps/{given,when,then}.rs`. All steps are
synchronous `fn`, never `async fn`: the cucumber-rs runner here uses a
synchronous world, and an async step compiles but panics or hangs at runtime.

Useful world methods:

- `world.execute_law(law_id, output_name)` runs the engine and stores the result
- `world.get_output(name)` retrieves a named output
- `world.is_success()`, `world.error_message()`
- `world.parameters`, `world.external_data`

## Iterate, at most three times

- All scenarios pass: you are done.
- Failures: a logic bug goes in the YAML, a wrong assertion goes in the step
  code. **Never change the expected values in MvT-derived scenarios.** Those are
  the legislature's own worked examples and they are the ground truth.
- After three iterations, stop and report what is left. Each iteration includes
  its own validate cycle. For a law over 20 articles the budget applies per
  batch of roughly 15 articles.

## Ad-hoc evaluation without a feature file

Build the evaluate binary and feed it a payload:

```bash
cargo build --manifest-path packages/engine/Cargo.toml --bin evaluate --release
cat /tmp/eval_payload.json | ./target/release/evaluate
```

Write the payload with the `Write` tool, never with `echo`: Dutch legal text
contains quotes and newlines that break shell escaping.

```json
{
  "law_yaml": "<full YAML content of the law file>",
  "output_name": "heeft_recht",
  "params": {"bsn": "999993653", "peildatum": "2025-01-01"},
  "date": "2025-01-01",
  "extra_laws": [{"id": "wet_op_de_zorgtoeslag", "yaml": "<content>"}]
}
```

Every law the file binds to through `source.regulation` has to be in
`extra_laws`, or the binding resolves to nothing and the run proves nothing.
