---
name: regelrecht-stelselanalyse
description: Voert de iteratieve desk-review-&-completion-cyclus uit op een regelrecht-corpus — plannen, harvesten/migreren/uitbreiden, multi-agent valideren, bevindingen 4-weg classificeren (modellering-fout / wetgevings-fout / engine-limitatie / acceptabele untranslatable), synthese + heroverweging, documenteren (wetgevings-/stelselfouten-analyse, fixes-plan, engine-limitaties, diagrammen, corpus-status), verifiëren tegen externe bronnen (resolutie-tracker + bronnen-dossier), en een eindrapport per cyclus. Gebruik dit bij corpus-review op correctheid/coverage zonder live expert-sessie, het opstellen van een wetgevings-notitie-bron, schema-migratie, het harvesten van nieuwe wetten, of het aansturen van een autonome completion-loop. Dossier-agnostisch; de regelrecht-methode is de vaste taal. Voor live-validatie met experts: zie regelrecht-audit-products.
allowed-tools: Read, Write, Edit, Grep, Glob, Bash, WebFetch, WebSearch, Agent, AskUserQuestion, ScheduleWakeup, Monitor
---

# Regelrecht stelselanalyse — desk-review & corpus-completion

De analytische desk-tegenhanger van `regelrecht-audit-products` (live-validatie met
experts). Deze skill drijft de **iteratieve cyclus** waarmee een machine-leesbaar
corpus stap voor stap correcter en completer wordt, en levert de bijbehorende
producten op.

Het verschil met een audit-doc: een audit-doc vraagt *"klopt ons stappenplan?"*. Deze
skill vraagt óók *"klopt de wet zelf, is hij uitvoerbaar, en hoe ver komt de engine?"*.
Zie `references/classification.md`.

Geen casus-inhoud in de skill; concrete inhoud komt uit het corpus en de externe
bronnen tijdens de cyclus.

## Routing & handoff

Dit is de desk-laag. Oordeels-/praktijkvragen (acceptabele untranslatables, formule-vs-
praktijk, scope-keuzes) routeren naar de workshop-skill **`regelrecht-audit-products`**;
feitelijke defecten blijven hier. De *judgment*-set untranslatables wordt de beslispunten
van een workshop; de scope-analyse daar komt uit de desk-producten `cross-law-diagram` +
`corpus-status`. Workshop-correctiepunten komen terug de cyclus in (fixes + verificatie).
Twijfel je waar te beginnen → **`regelrecht-dossier`** (router). Canonieke flow:
`regelrecht-dossier/references/routing.md`.

## De cyclus

```
plan ─► (harvest / migreer / breid uit) ─► valideer (multi-agent, per as)
   ▲                                              │
   │                                              ▼
eindrapport ◄─ verifieer (bronnen + tracker) ◄─ classificeer (4-weg)
                                                  │
                                                  ▼
                                        synthese + heroverweging
                                                  │
                                                  ▼
                              documenteer (fouten / fixes / limitaties / diagrammen / status)
```

Eén cyclus pakt één **thema/scope** (niet alles tegelijk). Werk in micro-cycli.

## Kernprincipe: vier-weg-classificatie

Elke bevinding krijgt precies één label. Dit bepaalt waar hij landt en welke actie
volgt — verkeerd labelen = verkeerde actie. Zie `references/classification.md`:

1. **Modellering-fout** — onze YAML/feature wijkt af van de (correcte) wettekst →
   wij fixen het (`modellering-fixes-plan`).
2. **Wetgevings-fout** — de wet zelf is onjuist/achterhaald/onuitvoerbaar, niet door
   interpretatie te repareren → aanbevolen wetgevings-actie (`wetgevingsfouten-analyse`).
3. **Engine-limitatie** — wet + modellering kloppen, engine kan het (nog) niet →
   engine-issue (`engine-limitaties`).
4. **Acceptabele untranslatable** — open norm die bewust niet gemodelleerd wordt en
   dat ook niet hoeft → markeren, geen actie.

De grens tussen (2) en (4) is de subtielste en belangrijkste: een open norm is
*acceptabel untranslatable* bij een kenbare beslisser + toetsbaar kader, en een
*wetgevings-fout* als beslisser/kader/grond ontbreekt.

**Meta-check, altijd uitvoeren**: maken de features en de YAML dezelfde fout? Zo ja,
dan valideert de BDD-suite de YAML en niet de wet — groene tests bewijzen dan niets
over juridische correctheid.

## Werkstroom

1. **Plan de cyclus.** Bepaal thema/scope en deel op in micro-cycli
   (`templates/cyclus-plan.md`). Voor autonome uitvoering: schrijf een self-driving
   loop-prompt (`templates/loop-prompt.md`). Zie `references/cycle-workflow.md`.

2. **Doe het corpus-werk.** Harvest een nieuwe wet, migreer naar een nieuwe
   schemaversie, of breid MR-logica uit — elk met een rapport
   (`harvest`/`mr`/`schema-migratie`-sjablonen).

3. **Valideer, eventueel multi-agent.** Verdeel over review-assen (correctheid,
   untranslatables, coverage, source-refs, diagrammen, wetgevings-fouten). Bij
   parallel werk: één sub-agent per as via de `Agent`-tool. Zie
   `references/review-orchestration.md`.

4. **Classificeer** elke bevinding 4-weg (zie boven). Voer de meta-check uit.

5. **Synthese + heroverweging.** Voeg parallelle reviews samen; markeer twijfel-claims;
   schrap weerlegde claims (doorgestreept + reden) en pas tellingen aan.

6. **Documenteer** vanuit de sjablonen: wetgevings-fouten, fixes-plan, engine-limitaties,
   cross-law-diagrammen, corpus-status, engine-tests.

7. **Verifieer met externe bronnen.** Ontgin Staatsbladen / nota's van toelichting /
   rekenregels via WebFetch; leg status vast in `resolutie-tracker.md`
   (vocabulaire: verifieerd / weerlegd / gedeeltelijk / onverifieerbaar / out-of-scope)
   en in het bronnen-dossier. Zie `references/verification-cycle.md`.

8. **Eindrapport** per cyclus (`templates/eindrapport.md`): doel vs resultaat, geleverd,
   discoveries, open punten voor de volgende cyclus.

9. **Schrijf naar het corpus/dossier, niet in de skill. Commit/push alleen op verzoek.**

## Bestanden in deze skill

**references/**
- `classification.md` — vier-weg-classificatie + onderscheid met de audit-doc + de
  beslisregel open-norm-vs-wetgevingsfout. Het conceptuele hart.
- `review-orchestration.md` — multi-agent review: assen, sub-agent-verdeling, synthese,
  heroverweging, telling, de features-vs-YAML meta-check.
- `defect-taxonomy.md` — categorieën wetgevings-fouten, ernst-niveaus, per-fout format.
- `verification-cycle.md` — resolutie-tracker, status-vocabulaire, scope-markering,
  bronnen-dossier + INDEX, WebFetch-ontginning.
- `cycle-workflow.md` — de cyclus-motor: planning, loop-prompts, harvest, schema-migratie,
  eindrapport, micro-cycli.

**templates/** — `cyclus-plan`, `loop-prompt`, `scheduled-routine`, `schema-migratie`,
`harvest-rapport`, `mr-uitbreiding-rapport`, `validatie-review`, `synthese`,
`wetgevingsfouten-analyse`, `modellering-fixes-plan`, `engine-limitaties`,
`resolutie-tracker`, `bronnen-INDEX`, `cross-law-diagram`, `corpus-status`,
`engine-tests`, `eindrapport`.

Autonome uitvoering (`/loop` self-paced voor een afgebakende batch, `/loop <interval>`
voor polling, `/schedule`-routine voor onbeheerd terugkerend werk): zie
`references/cycle-workflow.md` en de sjablonen `loop-prompt` + `scheduled-routine`.

**scripts/** — `assert-private-repo.sh`: fail-closed guard die alleen doorlaat als de
push-target een **private** GitHub-repo is (autonome routines mogen committen/pushen, maar
alleen naar private repos). Draai als preflight vóór elke push; PUBLIC/INTERNAL → geweigerd.

## Belangrijke regels

- **Classificeer vóór je documenteert.** Het label bepaalt het product en de actie.
- **Wetgevings-fouten zijn een formeel product** (bron voor een wetgevings-notitie):
  feitelijk, met wettekst-citaat, implicatie en concrete reparatie — geen losse meningen.
- **Bewaar de redeneerketen bij heroverweging**: geschrapte claims blijven zichtbaar
  (doorgestreept + reden), tellingen aangepast — geen stille verwijdering.
- **Scheid corpus- en engine-issues**: een engine-limitatie is geen wetgevings- of
  modellering-fout; track 'm apart zodat het corpus niet onterecht "fout" lijkt.
- **Dossier-agnostisch blijven**; niets pushen zonder toestemming.
</content>
