# Cyclus-workflow — de motor

Hoe je een cyclus plant, autonoom laat draaien, en afsluit. De producten hier zijn de
"workflow-motor": ze sturen het corpus-werk aan in plaats van bevindingen te documenteren.

## Micro-cycli

Eén cyclus pakt één thema/scope; deel hem op in **micro-cycli** met elk een eigen,
afgebakend doel (bijv. "harvest wet X", "migreer naar schema vN", "review-as Y +
synthese"). Dit houdt elke stap toetsbaar en maakt een eindrapport per cyclus mogelijk.

## Cyclus-plan

`templates/cyclus-plan.md`: legt het thema vast, de micro-cycli, de doelen per micro-
cyclus, de scope-afbakening (wat wél/niet), en de relatie tot de tracker. Verwijst naar
het bronnen-dossier en de vorige eindrapporten.

## Autonome uitvoering: loop vs schedule

Drie manieren om werk autonoom te draaien. Kies op situatie:

| Situatie | Mechanisme | Sjabloon |
|---|---|---|
| Afgebakende batch die zichzelf afmaakt, jij bij de sessie | **`/loop` self-paced** (geen interval; Claude bepaalt cadans en stopt zelf) | `loop-prompt.md` |
| Wachten op een externe run (validatie/CI) binnen je sessie | **`/loop <interval>`** (vaste polling) | `loop-prompt.md` |
| Terugkerend toezicht dat terminal-sluiting moet overleven | **routine (`/schedule`)** — cloud, onbeheerd, cron, min. 1 uur, géén permission-prompts | `scheduled-routine.md` |
| Eén run op een toekomstig moment | éénmalig: `/schedule <tijd> <prompt>` | `scheduled-routine.md` |

Vuistregels:
- **`/loop` is lokaal + sessie-gebonden** (stopt bij terminal-sluiting, max 7 dagen). Voor
  een corpus-completion-microcyclus is self-paced de beste fit: elke iteratie doet echt
  werk en de loop eindigt zodra de werklijst leeg is.
- **Routines draaien onbeheerd zónder bevestiging.** Committen/pushen mag, maar **alleen
  naar private repos**: draai `scripts/assert-private-repo.sh` als fail-closed preflight
  vóór elke push (PUBLIC/INTERNAL → geweigerd). Escape-hatch voor een bewuste publieke
  push: `ALLOW_PUBLIC_PUSH=1`.
- **Harde afdwinging lokaal**: een globale PreToolUse-hook
  (`~/.claude/hooks/git-push-private-guard.sh`) blokkeert elke `git push` naar een
  niet-private repo in lokale sessies en `/loop`-runs. **Cloud-`/schedule`-routines**
  draaien in een aparte omgeving en gebruiken die lokale hook niet — daar is de
  guard-in-de-routine-prompt de primaire beveiliging. Zie `templates/scheduled-routine.md`.
- Cadans-wijsheid voor self-paced loops: werk-iteratie → direct door; wachten op een run →
  poll ~270s (cache warm); idle-fallback ~1200s+. Vermijd precies 300s.

Gebruik autonomie alleen voor repetitief, goed-afgebakend werk; voor verkennend/oordeels-
werk niet automatiseren — markeer twijfel als `NEEDS_HUMAN_REVIEW` en ga door.

## Harvest (nieuwe wet)

`templates/harvest-rapport.md`: een nieuwe wet/regeling uit de officiële tekst het corpus
in brengen. Het rapport legt vast: bron-id + versie, welke artikelen zijn opgenomen, de
gemaakte interpretatie-keuzes, cross-law-koppelingen, en wat (nog) niet is opgenomen.

## Schema-migratie

`templates/schema-migratie.md`: het corpus naar een nieuwe schemaversie brengen. Werk in
twee passes:
- **Mechanische pass** — URL-bumps, hernoemde operaties, gewijzigde syntax (zoek-en-
  vervang-achtig, per file).
- **Semantische pass** — nieuwe schema-features benutten (nieuwe velden, nieuwe
  structuren), per artikel beoordeeld.
Noteer **discoveries** over het schema (welke aannames klopten/niet) — die zijn vaak het
waardevolst voor de volgende cyclus.

## Eindrapport

`templates/eindrapport.md` sluit elke cyclus af: doel vs resultaat (tabel), wat is
geleverd, discoveries (schema + wetgeving), commits, wat bewust níet is geleverd, en open
punten voor de volgende cyclus. Verwijst naar alle producten die de cyclus opleverde.

## Volgorde binnen een cyclus

```
cyclus-plan ─► [loop-prompt] ─► corpus-werk (harvest / schema-migratie / mr-uitbreiding)
   ─► validatie-review(s) ─► synthese + heroverweging
   ─► documentatie (wetgevings-fouten / fixes-plan / engine-limitaties / diagrammen / corpus-status / engine-tests)
   ─► verificatie (bronnen + resolutie-tracker)
   ─► eindrapport
```
</content>
