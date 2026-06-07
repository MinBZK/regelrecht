# Investigation: harvester edge-case failures

**Status:** investigation / proposal (no behaviour change in this PR)
**Scope:** the harvest failures from the 2026-06-05 corpus-wide run that were **not** caused by the container resource-exhaustion incident (fixed in #763). After excluding the fork()/EAGAIN failures, ~15 distinct laws failed for two genuine, reproducible reasons in the harvester itself.

## Summary

| # | Cause | Failures (rows / laws) | Nature | Proposed handling |
|---|-------|------------------------|--------|-------------------|
| A | `Missing required XML element: _latestItem attribute in manifest` | 25 / 8 | Law has **no consolidated text** (withdrawn or not-yet-in-force) | Detect and skip gracefully with an explicit status — not a hard error |
| B | `IO error: Filename too long (os error 36)` | 17 / 5 | Official title → slug **> 255 bytes** used as a directory name | Cap slug length (filesystem-safe), keep uniqueness |

Two laws that failed once each (a redirect error and a `git pull: unstaged changes`) were transient side-effects of the resource-exhaustion chaos and have been re-queued separately; they are out of scope here.

---

## Issue A — missing `_latestItem` (withdrawn / future laws)

### Symptom
`manifest.rs::parse_manifest` requires a `_latestItem` attribute on the `<work>` element and returns a hard `HarvesterError::MissingElement` when it is absent. The job then fails, retries, and exhausts.

```rust
// packages/harvester/src/manifest.rs
let latest_item = work
    .attribute("_latestItem")
    .ok_or_else(|| HarvesterError::MissingElement {
        element: "_latestItem attribute".to_string(),
        context: format!("manifest for {bwb_id}"),
    })?
    .to_string();
```

### Root cause
`_latestItem` is only present when a law has at least one **consolidated expression**. Two legitimate classes of law have none, so BWB emits a `<work>` with only `<metadata>` and no `_latestItem`/`<expression>`:

**A1 — withdrawn / repealed laws (`datum_intrekking`)**

```
BWBR0004339  datum_intrekking 2004-01-03   _latestItem=0  expressions=0
BWBR0022977  datum_intrekking 2010-10-01   _latestItem=0  expressions=0
BWBR0023074  datum_intrekking 2021-04-21   _latestItem=0  expressions=0
BWBR0045723  datum_intrekking 2022-01-01   _latestItem=0  expressions=0
BWBR0025304  datum_intrekking 2013-01-01   _latestItem=0  expressions=0
```

**A2 — future / not-yet-in-force laws (`datum_inwerkingtreding` in the future, or only WTI metadata)**

```
BWBR0051108  datum_inwerkingtreding 2027-01-01   _latestItem=0  expressions=0
BWBR0051692  datum_inwerkingtreding 2027-01-01   _latestItem=0  expressions=0
BWBR0051671  (only WTI metadata, added 2025-10-31) _latestItem=0  expressions=0
```

Example manifest (`BWBR0004339`):

```xml
<work label="BWBR0004339" ...>
  <metadata>
    <datum_intrekking>2004-01-03</datum_intrekking>
    ...
  </metadata>
</work>
```

These are **not** harvester defects in parsing — there is genuinely no consolidated text to harvest. Treating them as a hard error is wrong: it burns retries and pollutes the failed queue with laws that can never succeed.

### Proposed fix
Distinguish "manifest is malformed" (real error) from "manifest has no consolidation" (expected, skippable):

1. Add a typed outcome, e.g. `HarvesterError::NoConsolidation { bwb_id, reason }` where `reason` is derived from the metadata:
   - `withdrawn(datum_intrekking)` when `datum_intrekking` is present,
   - `not_yet_in_force(datum_inwerkingtreding)` when a future `datum_inwerkingtreding` is present,
   - `no_consolidation` otherwise (only WTI metadata).
2. In `parse_manifest`, when `_latestItem` and `<expression>` are both absent, read `<metadata>` and return `NoConsolidation` instead of `MissingElement`.
3. In the pipeline worker, map `NoConsolidation` to a **terminal non-failure** law status (e.g. `harvest_skipped` / reuse `harvest_exhausted` with a clear reason) so the job is completed-as-skipped rather than retried. This keeps these laws out of the failed queue.

Open question for the team: do we want withdrawn laws recorded with a `withdrawn` status in `law_entries` (useful for coverage reporting), or simply skipped silently? Recommendation: record the status — it is meaningful corpus metadata.

---

## Issue B — filename too long (`os error 36`)

### Symptom
`yaml/writer.rs::save_yaml` builds the output path as `output_base/{layer_dir}/{slug}/{date}.yaml`, where the per-law directory is the **slug of the official title**:

```rust
// packages/harvester/src/yaml/writer.rs
let law_id = law.metadata.to_slug();
let output_dir = output_base.join(layer_dir).join(&law_id);
fs::create_dir_all(&output_dir)?;   // <-- ENAMETOOLONG (os error 36) here
```

`to_slug()` (`types.rs`) lowercases the title, strips diacritics/non-word chars, and joins with `_` — with **no length cap**.

### Root cause
Linux caps a single path **component** at 255 bytes. Five laws have official titles whose slug exceeds that:

```
BWBR0009790  slug_len=259  "Wijzigingsbesluit Bekostigingsbesluit WBO/OWBO, enz. (totstandbrenging van een Wet op het primair onderwijs …)"
BWBR0027415  slug_len=262  "Wijzigingswet Wet op de architectentitel (beroepservaring, bij- en nascholingsregeling voor stedenbouwkundigen …)"
BWBR0049301  slug_len=267  "Besluit vaststelling aantal tijdstippen waarvan in de Omgevingswet en daarmee verband houdende wet- en regelgeving …)"
BWBR0043651  (long title, same pattern)
BWBR0046751  (long title, same pattern)
```

These are "Wijzigings*"/"Besluit vaststelling …" laws with very long descriptive official titles.

### Proposed fix
Cap the slug to a filesystem-safe length in `to_slug()` (or at the call site in `save_yaml`):

- Truncate to **≤ 200 bytes** (leaves headroom under 255 for the temp-file prefix `.{date}.yaml.tmp`), cutting at the last `_` boundary so the slug stays readable.
- To avoid collisions between two different long-titled laws that share a 200-char prefix, append a short disambiguator derived from the **BWB id** (which is globally unique), e.g. `…_bwbr0009790`. This also makes the directory traceable to its source.

Sketch:

```rust
const MAX_SLUG_BYTES: usize = 200;

fn cap_slug(slug: &str, bwb_id: &str) -> String {
    if slug.len() <= MAX_SLUG_BYTES { return slug.to_string(); }
    let suffix = format!("_{}", bwb_id.to_lowercase());
    let budget = MAX_SLUG_BYTES - suffix.len();
    let mut cut = slug[..budget].to_string();
    if let Some(i) = cut.rfind('_') { cut.truncate(i); }   // cut on a word boundary
    format!("{cut}{suffix}")
}
```

Open question: changing the slug changes the **on-disk path** for these laws. For laws already harvested under a (currently failing → so not yet present) path this is a non-issue, but the rule must be applied consistently so re-harvests are idempotent. Recommend landing B before re-harvesting the 5 affected laws.

---

## Test strategy

- **A:** unit tests in `manifest.rs` with fixtures for (a1) a `datum_intrekking` manifest, (a2) a future `datum_inwerkingtreding` manifest, (a3) a WTI-only manifest — assert each yields `NoConsolidation` with the right reason, and a normal manifest still parses. Pipeline test: a `NoConsolidation` harvest result lands the law in the skipped/withdrawn status, not `harvest_failed`, and does not retry.
- **B:** unit test in `writer.rs`/`types.rs` with a >255-char title — assert the resulting directory component is ≤ 200 bytes, ends with the bwb-id suffix, and that two distinct long titles produce distinct paths.

## Recommendation / next steps
1. Land **B** (slug cap) — small, self-contained, unblocks the 5 long-title laws on re-harvest.
2. Land **A** (graceful no-consolidation handling) — needs a small pipeline status decision (record `withdrawn` vs silent skip); worth a quick team confirmation first.
3. After both merge, re-queue the 13 affected laws (8 for A, 5 for B).

The exact affected law ids are listed per issue above so they can be re-queued and verified individually.
