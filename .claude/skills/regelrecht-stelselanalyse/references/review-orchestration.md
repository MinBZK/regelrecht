# Multi-agent review-orkestratie

Hoe je een corpus-review verdeelt over parallelle assen, de uitkomsten samenvoegt, en
twijfel heroverweegt. Generiek — pas de assen aan op de cyclus-scope.

## Review-assen

Verdeel de review over onafhankelijke assen. Eén as = één reviewer/sub-agent. Typische
assen:

1. **Machine-readable correctheid** — kloppen de formules/actions met de wettekst?
2. **Untranslatables-correctheid** — zijn open normen correct gemarkeerd (factual vs
   judgment), en ontbreken er untranslatables?
3. **Wetten-coverage** — welke wetten in de keten ontbreken nog in het corpus?
4. **machine_readable-coverage** — welke artikelen/leden hebben nog geen MR-logica?
5. **Cross-law source-refs** — zijn verwijzingen als echte `source:` gelegd of staan ze
   alleen in `description`? Dit is een verplichte, niet-overslaanbare integriteitsscan
   (zie SKILL.md stap 3): bouw `regulation → outputs`, vlag elke `source`-binding naar een
   niet-bestaande output (**DANGLING**) en elke "conceptueel/forward/tijdelijk"-input zonder
   `source:`-blok (**PLAIN-PARAM**). Beide zijn altijd **modellering-fout**, nooit engine-
   limitatie. Rapporteer `clean / dangling / plain-param`; source-clean = beide 0. Draai
   `references/cross-law-integriteit.py <corpus-root>` als reproduceerbare preflight.
6. **Diagrammen** — kloppen de relatie-/flow-diagrammen met de YAML's?
7. **Wetgevings-fouten** — fouten in de bron-regelgeving zelf (zie `defect-taxonomy.md`).

## Parallelle uitvoering met sub-agents

Bij een brede review: start één `Agent` per as (bij voorkeur in één bericht, zodat ze
parallel lopen). Geef elke sub-agent:
- de scope (welke wetten/artikelen),
- de as + wat als bevinding telt,
- de **vier-weg-classificatie** (zie `classification.md`) met de opdracht elke
  bevinding te labelen,
- een vast bevindingen-format (classificatie + ernst + locatie + korte omschrijving).

Elke sub-agent levert een per-as review-doc op (`templates/validatie-review.md`).

## Synthese

Voeg de per-as reviews samen tot één synthese (`templates/synthese.md`):
- **Wat is gefixt deze ronde** (met commit-verwijzing; bevestig dat tests groen blijven).
- **Wat is gedocumenteerd, niet gefixt** — tabel per as/agent met aantal + classificatie.
- **Discoveries uitgelicht** — de belangrijkste, met bron en gevolg.
- **Meta-bevindingen** — bijv. de features-vs-YAML-check.
- **Engine-correctheid behouden** — validatie- en scenario-commando's met N/N-uitslag.

## Telling & ernst

Houd expliciete tellingen bij (aantal wetgevings-fouten per categorie, per ernst). Deze
tellingen komen terug in de samenvatting van de fouten-analyse en het eindrapport — dus
houd ze consistent over documenten heen.

## Heroverweging van twijfel-claims

Markeer onzekere bevindingen expliciet (bijv. een `TWIJFEL`- of `NEEDS_HUMAN_REVIEW`-
status). In een aparte heroverwegings-pass:
- verifieer of de claim standhoudt;
- bij **weerlegging**: schrap de claim **zichtbaar** (doorgestreept + reden + verwijzing
  naar de heroverwegings-notitie), en pas de tellingen aan met -1;
- bij **verplaatsing** naar een andere categorie: noteer de verplaatsing, telling
  ongewijzigd.

Nooit stilzwijgend verwijderen — de redeneerketen moet traceerbaar blijven.
</content>
