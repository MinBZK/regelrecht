# textguard — per-lid tekstgetrouwheid

Golden-text fixtures die bewaken dat de `text:`-blokken in `corpus/regulation/`
niet stilletjes wijzigen ten opzichte van een geverifieerde baseline. Per-lid
granulariteit, deterministisch, hermetisch — bruikbaar als CI-gate.

## Twee lagen

Tekstgetrouwheid splitst in een hermetisch (CI) en een niet-hermetisch (methodologisch) deel:

1. **Capture & bless** — *methodologisch, periodiek.* De geldende wettekst leeft op
   wetten.overheid.nl (een remote, mutabele bron die geïnterpreteerd moet worden — geen
   CI-orakel). De `law-version-drift-check`-skill haalt die tekst op, normaliseert en
   verifieert hem, en legt het resultaat hier vast. `just textguard-bless` capture't de
   huidige corpus-tekst als fixture; de skill upgrade't `verified_against` van `pending`
   naar een `wetten.overheid.nl/<bwb>/<datum>`-bron.

2. **Gate** — *deterministisch, elke commit.* `just textguard-check` (CI-job
   `text-fidelity`) herberekent per lid de genormaliseerde hash uit de corpus en
   vergelijkt met de fixture. Mismatch → faal. Zuivere functie van repo-inhoud.

**De eerlijke grens:** de gate bewaakt *"YAML-tekst ≡ laatst geverifieerde tekst"*, niet
*"YAML-tekst ≡ huidige live wet"*. Een stille YAML-edit faalt direct in CI. Verandert de
wet zélf in de wereld, dan vangt alleen de periodieke drift-check (laag 1) dat — daarna
re-bless je.

## Wat is een chunk

Eén chunk = één lege-regel-gescheiden alinea van een artikel-`text:` — in deze corpus
precies één lid / onderdeel / aanhef. Identiteit is positioneel (artikelnummer + index);
het label (`aanhef` / `lid 1` / `onderdeel a`) is afgeleid, puur voor leesbaarheid.

Normalisatie is de enige toegestane regel uit `law-version-drift-check/reference.md` §2:
binnen een alinea elke witruimte-reeks naar één spatie, trimmen; alinea-grenzen blijven.

## Fixture-formaat

Sidecar per regeling, gespiegeld pad onder `textguard/` (buiten `corpus/regulation/**`
zodat de wet-schema-validator ze niet oppakt). Per chunk: `sha256` (de gate), `text` (de
genormaliseerde tekst, mens-auditeerbaar) en herkomst (`verified_against`, `captured_at`).

## Workflow bij een tekstwijziging

1. Wijzig de `text:` in de corpus (na verificatie tegen de geldende wet).
2. `just textguard-bless` — herbevestig de fixtures.
3. Bekijk de fixture-diff (de hash + tekst veranderen zichtbaar mee).
4. Commit corpus + fixtures samen. CI's `text-fidelity` is daarna weer groen.

Wie de tekst wél wijzigt maar de fixture niet her-blesst, krijgt een rode gate — precies
de bedoeling.
