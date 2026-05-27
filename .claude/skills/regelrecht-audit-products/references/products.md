# Productcatalogus

Per product: doel, input, modus + automatiseringsgraad, hoe te bouwen, en het
bijbehorende sjabloon. **Modus** volgt het hybride principe uit SKILL.md:
`auto` (uit YAML), `semi-auto` (analist legt focus), `mens-gedreven` (skill
structureert observaties van de analist).

## Onderlinge volgorde

```
1. Scope-analyse + wet-graph        (auto)            ── fundament
2. Per-artikel audit-doc            (semi-auto)       ── per kernartikel
        │
        ├─► 3. Workshop-draaiboek   (semi-auto)
        │       ├─► 4. Facilitator-brief    (auto, condenseert 3)
        │       ├─► 5. Jargon-spiekbriefje  (auto + casus-voorbeelden)
        │       └─► 6. Claude-orakel-prompt (semi-auto)
        │
        └─► 7. Testcase-scenario's  (semi-auto, vereist engine-uitkomsten)
                └─► vervolgsessie-werkvorm (in zelfde sjabloon)

na de sessie:
   8. Verslag intern   (mens-gedreven)
   9. Verslag extern   (mens-gedreven, afgeleid van 8)

alternatief spoor (geen live sessie):
   → zie de zusterskill `regelrecht-stelselanalyse` (desk-review & corpus-completion)
```

Bouw 1 en 2 altijd eerst; de facilitatie-producten leunen erop.

---

## 1. Scope-analyse + wet-graph — `templates/scope-analyse.md`
**Doel.** Het wettelijk stelsel rond de casus in kaart: welke wetten, in welke laag,
hoe ze samenhangen (source / legal_basis / override / data), en welke scope-keuzes
de experts moeten bekrachtigen.
**Input.** Scope-manifest + YAML's (legal_basis, source, overrides).
**Modus.** `auto` — leid de lagen, relaties en de mermaid-graph af uit de YAML's.
Laat de analist de laag-indeling en de scope-beslispunten bevestigen.
**Bouw.** (a) Wetten-tabel met laag + rol + YAML-pad. (b) Mermaid-graph met de vier
relatie-types. (c) Runtime-afhankelijkheden (source-calls). (d) legal_basis-keten.
(e) Override-relaties. (f) Scope-beslispunten (S1..Sn) als checkboxes.

## 2. Per-artikel audit-doc — `templates/audit-doc.md`
**Doel.** Kern-validatiedocument: koppelt elke `machine_readable` output aan de
wettekst, zodat een jurist rij-voor-rij kan controleren. Bevat untranslatables en
open punten.
**Input.** De YAML van het artikel + de wettekst-bron.
**Modus.** `semi-auto` — extraheer outputs/formules/untranslatables uit de YAML,
maar **bepaal samen met de analist welke artikelen kernartikelen zijn** en waar de
focus ligt. Vraag dit met `AskUserQuestion` voordat je alle artikelen uitwerkt.
**Bouw.** Per output: wettekst-excerpt (letterlijk, met subsectie + link), formule
(natuurlijke notatie), interpretatie, YAML-locatie, en review-checkboxes (punten die
de jurist apart bevestigt). Daarna: untranslatables-tabel (factual vs judgment),
externe overrides met grondslag-nuance, en open punten voor de sessie.

## 3. Workshop-draaiboek — `templates/workshop-draaiboek.md`
**Doel.** Inhoudelijk draaiboek voor de facilitator: agenda, walk-through per output,
beslispunten, werkvormen. Niet bedoeld om op scherm te tonen.
**Input.** Audit-docs (2) + scope-analyse (1) + sessie-doel + deelnemers.
**Modus.** `semi-auto` — skelet en walk-through-volgorde uit de audit-docs; tijden,
werkvormen en beslispunten samen met de analist (sessie-duur, aantal deelnemers,
zwaartepunten). Vraag doel/duur/deelnemers op.
**Bouw.** Gebruik de patronen uit `facilitation-patterns.md` (protocol A/B/C,
1-2-4-all, time-boxing). Per output een time-box en key-reminders; beslispunten met
verwacht weerwoord + comeback.

## 4. Facilitator-brief — `templates/facilitator-brief.md`
**Doel.** 1-A4 spiekbriefje voor tijdens de sessie (klaarzetten, blok-voor-blok
reminders, risico's + back-pockets, checklist).
**Input.** Het draaiboek (3).
**Modus.** `auto` — condenseer het draaiboek naar het korte format. Voeg niets nieuws
toe; dit is een afgeleide.

## 5. Jargon-spiekbriefje — `templates/jargon-spiekbriefje.md`
**Doel.** Schema-/engine-termen in mensentaal, met één concreet casus-voorbeeld per
term, plus back-pocket-antwoorden op lastige vragen.
**Input.** `method-glossary.md` + casus-voorbeelden.
**Modus.** `auto` voor de generieke uitleg; de analist levert de concrete
casus-voorbeelden per term aan (of je leidt ze af uit de YAML).

## 6. Claude-orakel-prompt — `templates/claude-orakel-prompt.md`
**Doel.** Systeem-prompt die Claude in een YAML-lookup-rol zet tijdens de sessie:
citeert de YAML, simuleert de engine, signaleert gaps — geeft **geen** juridisch
oordeel.
**Input.** De casus-keten (welke YAML's, welke outputs, welke source-calls).
**Modus.** `semi-auto` — vul de context-sectie (casus, bronnen, scope, outputs,
delegaties) uit de YAML's; de rol-instructies zijn generiek.

## 7. Testcase-scenario's (+ vervolgsessie-werkvorm) — `templates/testcase-scenarios.md`
**Doel.** Een set scenario's die elk een ander beoordelings-pad raken, met de
engine-uitkomst per scenario; plus een draaiboek voor een testcase-sessie ("komen
jullie op hetzelfde uit als de engine, en hoe rekenen jullie?").
**Input.** De engine (voor uitkomsten) + de hoofdpaden uit de audit-docs.
**Modus.** `semi-auto` — de analist kiest welke beoordelings-paden de scenario's
moeten raken; de skill stelt scenario's voor en haalt engine-uitkomsten op (of
vraagt ze). **Scenario's zijn fictief of sterk-gelijkend — nooit persoonsgegevens.**

## 8. Verslag intern — `templates/verslag-intern.md`
**Doel.** Eerlijk, reflectief verslag voor het eigen team: wat is werkelijk gedaan,
onderzoek-/correctiepunten, sessie-dynamiek, tooling-leerpunten, way forward.
**Input.** Sessie-notities/observaties van de analist.
**Modus.** `mens-gedreven` — de skill structureert; de inhoud (observaties,
correctiepunten) komt van de analist. Stel gerichte vragen om de secties te vullen.

## 9. Verslag extern — `templates/verslag-extern.md`
**Doel.** Hoogover, waarderende terugkoppeling voor de klant: wat we samen deden,
wat het opleverde (op hoofdlijnen), vervolg.
**Input.** Het interne verslag (8).
**Modus.** `mens-gedreven` — afgeleid van het interne verslag, maar **abstraheer**:
geen detail-bevindingen, geen interne reflecties, geen vertrouwelijke details.

## 10. Analytische desk-review → **aparte skill `regelrecht-stelselanalyse`**
Het analytische desk-spoor (corpus-review op correctheid/coverage, wetgevings-/
stelselfouten-analyse, multi-agent review + synthese, resolutie-tracker met bronnen-
verificatie, eindrapport per cyclus) is **geen workshop-product** en woont in de
zusterskill `regelrecht-stelselanalyse`. Gebruik die skill zodra het gaat om fouten in
de *wet zelf* of de *engine* i.p.v. validatie van onze modellering met experts.
</content>
