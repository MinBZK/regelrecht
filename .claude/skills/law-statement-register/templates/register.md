# Statement-register — {{DOCUMENT}} ({{VERSIE}})

> **Methode.** Statements ontgonnen uit *{{DOCUMENT_TITEL}}* volgens de skill
> `law-statement-register`: betegelen → ankeren → verankeren → classificeren, met vier
> gates (verbatim / coverage / anchor / signaalnet) als bewijs.
>
> **Kernprincipe.** *De norm is leidend; deze tekst is uitleg, géén norm.* Per statement:
> **verbatim citaat → verankering in een norm? → classificatie → actie.** Buckets:
> **MODELFOUT** (model ≠ letter → fix richting de letter) · **WETTEKST-GEVOLG** (model =
> letter, letter geeft een vreemde uitkomst → rapporteren, niet fixen) ·
> **LETTER-vs-TOELICHTING** (tekst en norm divergeren → jurist beslist + wetgevings-signaal).
>
> **Een statement wordt nooit op eigen gezag een modelwijziging.**

## Bron en dekking

| | |
|---|---|
| Document | {{DOCUMENT_TITEL}} |
| Status | {{beleidsregel / toelichting / werkinstructie / handboek / faq}} |
| Bindendheidsclausule | *"{{verbatim disclaimerzin, of: geen}}"* |
| URL | {{URL}} |
| `source_sha256` | `{{HASH}}` |
| Opgehaald | {{YYYY-MM-DD}} |
| Grondslag | {{regeling + artikel waar dit document uitvoering aan geeft}} |

| Dekking | |
|---|---|
| Betegeld | {{100,0}}% van {{N}} tekens |
| Segmenten | {{N}} normatief · {{N}} informative · {{N}} navigational · {{N}} duplicate · {{N}} non-textual |
| Signaalnet | {{N}} treffers, {{N}} ongedekt |
| Statements | {{N}} |
| Gates | verbatim ✅ · coverage ✅ · anchor ✅ · signaalnet ✅ |

*Als een gate niet schoon is, staat hier waarom en wat er open blijft — niet een vinkje.*

---

## S1 — {{korte titel}}

- **Slug.** `{{slug}}` · **{{§-verwijzing}} (p{{n}})**
- **Verbatim.** *"{{letterlijk citaat uit canonical.md}}"*
- **Verankering.** {{verankerd / geparafraseerd / niet-gevonden}}.
  - *verankerd/geparafraseerd*: {{norm_ref}} — *"{{verbatim norm-citaat}}"*
  - *niet-gevonden*: gezocht op {{zoektermen}} in {{zoekruimte}} — geen normtekst bevat dit.
- **Bindendheid.** {{hard / soft-default / guidance / informative}}{{, met toelichting als de
  formulering zacht is}}
- **Afwijkingsklasse.** {{toelichting-bleed / ontbrekend-bestanddeel / verkeerde-verankering /
  herformulering / bindendheid-vervlakking / buitenwettelijk / geen}}
- **Classificatie: {{BUCKET}}.** {{waarom dit label en niet het aangrenzende}}
- **Actie / jurist-vraag.** {{concreet; bij LETTER-vs-TOELICHTING: de vraag zelf, plus het
  wetgevings-signaal (hoort dit in de regeling?), plus wat het model tot die beslissing doet}}

## S2 — {{...}}

{{herhaal}}

---

## Samenvatting per bucket

| Bucket | Statements | Actie |
|---|---|---|
| **MODELFOUT (fix richting letter)** | {{S…}} | {{}} |
| **WETTEKST-GEVOLG (rapporteren)** | {{S…}} | geen fix; model is letter-getrouw |
| **LETTER-vs-TOELICHTING (jurist)** | {{S…}} | {{de vraag + wetgevings-signaal}} |
| **Letter-getrouw ✅** | {{S…}} | geen wijziging |
| **Scope** | {{S…}} | {{beslisvraag}} |

## Bewust niet als statement opgenomen

Segmenten met een niet-normatieve `disposition`, zodat zichtbaar is wát is overgeslagen en
waarom. Dit is geen bijlage maar het bewijs dat er niets stil is weggevallen.

| Segment | Disposition | Reden |
|---|---|---|
| {{s0nn}} {{kop}} | {{informative}} | {{reden}} |

## Rekenvoorbeelden

{{Genummerde rekenvoorbeelden met bedragen zijn direct bruikbaar als BDD-scenario
(`law-mvt-research`, `regelrecht-scenario-traces`). Als het document er geen bevat, zeg dat
expliciet — een lege sectie is een bevinding, geen weglating.}}

## Open punten

{{Wat deze ronde niet is opgelost, met wie het moet beslissen. Bij een tweede versie: zie
de diff-sectie hieronder.}}

---

## Diff ten opzichte van {{VORIGE_VERSIE}}

*Alleen bij een herziening. Het nieuwe register is blind ontgonnen; dit is de vergelijking
achteraf. Zie `references/diff.md`.*

| Categorie | Slug | v{{1}} | v{{2}} | Oordeel |
|---|---|---|---|---|
| `added` | {{slug}} | — | *"{{citaat}}"* | {{bucket + betekenis}} |
| `candidate-removed` | {{slug}} | *"{{citaat}}"* | — | {{geschrapt / verplaatst / anker te specifiek}} |
| `changed-strekking` | {{slug}} | *"{{citaat}}"* | *"{{citaat}}"* | {{wat er inhoudelijk wijzigde}} |
| `reworded` | {{slug}} | *"{{citaat}}"* | *"{{citaat}}"* | same-strekking |
| `unchanged` | {{n}} statements | | | |

**Dekkingsvergelijking.** v1: {{N}} statements bij {{N}} signaalnet-treffers · v2: {{N}} bij
{{N}}. {{Als het aantal statements stijgt zonder dat het document groeide: dat meet de vorige
ronde, geen beleidswijziging — zeg dat hier.}}

**Blijvend open.** {{Statements die in beide versies `niet-gevonden` zijn: een jurist-vraag
die een herziening heeft overleefd.}}
