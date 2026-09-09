---
name: regelrecht-koppeling-audit
description: >
  Controleert of de koppelingen in een regelrecht-corpus daadwerkelijk resolven —
  `implements` naar (law, article, open_term), `open_terms` naar een invuller,
  `legal_character` naar een hook die erop filtert. Gebruik dit wanneer de BDD-suite
  groen is maar je wilt weten of alle delegaties écht aankomen, na het toevoegen of
  hernummeren van artikelen, na een schema- of versiemigratie, bij een AMvB/ministeriële
  regeling die een hogere wet invult, wanneer een waarde verdacht op zijn default blijft
  staan, of wanneer een diagram/README verbindingen toont die je niet kunt terugvinden.
  Vangt de defectklasse die scenario's per definitie niet zien: een koppeling die
  semantisch klopt maar op de sleutel afketst en stil op een default terugvalt.
  Dossier-agnostisch; werkt op elk regelrecht-corpus.
allowed-tools: Read, Grep, Glob, Bash, Write, Edit
---

# Koppeling-audit — resolvet de sleutel, of praat hij alleen zo?

Een koppeling in een regelrecht-corpus heeft twee kanten die los van elkaar fout
kunnen zijn:

- **Semantisch** — bedoelen producent en consument hetzelfde begrip?
  Dat is het terrein van `reference-and-concept-hygiene`. Niet deze skill.
- **Mechanisch** — matcht de sleutel waarop de engine indexeert?
  Dat is deze skill.

De tweede fout is gemener, want hij is **stil**. Een `implements`-blok dat op de
verkeerde sleutel wijst is syntactisch geldig, valideert tegen het schema, leest
juridisch correct, en de engine merkt niets: hij vindt geen invuller en gebruikt
de `default` van de open term. Er komt een plausibel getal uit. Geen enkel
scenario valt om, want de BDD-suite kent alleen uitkomsten, geen bedoelingen.

**Kernregel: een groene suite bewijst niets over koppelingen.** De suite toetst
wat de engine berekent, niet of hij de bron gebruikt die de wetgever aanwees.

## Wanneer wel, wanneer niet

| Situatie | Deze skill? |
|---|---|
| Artikelen toegevoegd, hernummerd of geherstructureerd | Ja |
| AMvB / ministeriële regeling gekoppeld aan een wet | Ja |
| Schema- of wetsversiemigratie afgerond | Ja |
| Waarde blijft verdacht op zijn `default` staan | Ja |
| Diagram of README toont verbindingen — kloppen die nog? | Ja |
| Betekenen producent en consument hetzelfde met dit begrip? | Nee → `reference-and-concept-hygiene` |
| Is de norm-keten zichtbaar geassert in scenario's? | Nee → `regelrecht-scenario-traces` |
| Volledige desk-review van een corpus op modellering | Nee → `regelrecht-stelselanalyse` |

## De drie controles

| Declaratie | Sleutel waarop de engine indexeert | Faalwijze als hij niet matcht |
|---|---|---|
| `implements: [{law, article, open_term}]` | `(law_id, article, open_term_id)` — `implements_index` in `packages/engine/src/resolver.rs` | Geen invuller gevonden; open term valt terug op `default` — stil |
| `open_terms: [{id, default?}]` | zie boven, vanaf de andere kant | Met `default`: stil terugvallen. Zonder: waarde blijft leeg |
| `execution.produces.legal_character` | `(hook_point, legal_character)` — `hooks_index` | Geen hook vuurt; procedurele artikelen doen niets |

Let op: `article` is een **string die letterlijk moet matchen met `number`** van
het doelartikel. Niet met het juridische artikelnummer in de wettekst, niet met
`legal_basis.article`, niet met de URL-fragmentnaam. Alleen met `number`.

## Rookproef — eerst op het testcorpus van de repo

Voer de audit één keer uit op `corpus/regulation` in deze repo. Dat corpus is
klein, publiek en bevat opzettelijk werkende koppelingen, dus je ziet hoe een
gezonde uitslag eruitziet voordat je hem op je eigen dossier loslaat:

```bash
python3 .claude/skills/regelrecht-koppeling-audit/koppeling-audit.py corpus/regulation
```

```
24 wetten geladen (peildatum 9999-12-31)

== DOOD (0)

== ONBEANTWOORD (3)
  [legal_character] INFORMATIEF -> 2 artikel(en)
  [legal_character] TOETS -> 3 artikel(en)
  [legal_character] WAARDEBEPALING -> 4 artikel(en)

== CORRECT (5)
  [implements] regeling_standaardpremie:1 -> wet_op_de_zorgtoeslag:4:standaardpremie
  ...
  [legal_character] BESCHIKKING -> 3 artikel(en)   er filtert een hook op
```

Twee dingen om te herkennen: `regeling_standaardpremie` → `wet_op_de_zorgtoeslag`
is het IoC-patroon zoals het hoort te resolven, en `BESCHIKKING` is het enige
legal_character waar in dit corpus wél een hook op filtert — de andere drie zijn
`ONBEANTWOORD` zonder dat er iets mis is. Zo ziet legitiem onbeantwoord eruit.

## Aanpak

1. **Bepaal het corpus en de peildatum.** Per wet telt alleen de nieuwste versie
   ≤ peildatum, precies zoals de engine laadt. Een koppeling die op de ene
   peildatum resolvet kan op de andere dood zijn.

2. **Draai `koppeling-audit.py`** op die map:

   ```bash
   ./koppeling-audit.py <corpus-dir> --peildatum 2026-07-01
   ```

   `<corpus-dir>` is de map met de `nl/`-laag, of `nl/` zelf. Exitcode 1 zodra er
   een dode koppeling is; `--json` voor machineverwerking.

3. **Classificeer elke bevinding.** Het script doet de mechanische kant; het
   oordeel is van jou.

   | Klasse | Betekenis | Actie |
   |---|---|---|
   | **DOOD** | De sleutel matcht niet. De declaratie doet niets. | Altijd een defect. Repareren of expliciet als bekend gebrek vastleggen. |
   | **ONBEANTWOORD** | De declaratie klopt, maar er is (nog) geen tegenhanger. | Vaak legitiem. Oordeel vereist. |
   | **CORRECT** | Resolvet. | Niets. |

4. **Scheid DOOD en ONBEANTWOORD scherp.** Ze door elkaar halen is de meest
   voorkomende fout bij dit werk: het maakt het rapport onbruikbaar, want elk
   corpus heeft tientallen legitieme onbeantwoorde declaraties en die verdrinken
   het handjevol echte defecten.

5. **Leg per DOOD-bevinding de oorzaak vast, niet alleen het symptoom.** Zie
   hieronder.

## De oorzaak achter bijna elke dode `implements`

**Inconsistente artikelnummering binnen één wet.** Nederlandse wetten die zijn
opgebouwd uit hoofdstukken met eigen nummerreeksen worden in het corpus soms
juridisch genoteerd (`2:22`, `3:63`) en soms positioneel (`142`). Zo'n wet is
niet fout — hij is inconsistent, en een `implements` uit een AMvB die de
juridische notatie uit de "Gelet op"-clausule overneemt ketst dan af op een
artikel dat positioneel genummerd staat.

Diagnose in drie stappen:

```bash
# 1. staat de open term überhaupt in de doelwet?
grep -n "id: <open_term>" <doelwet>.yaml
# 2. onder welk artikelnummer staat hij?
#    (het script zegt dit al: "bestaat ... maar onder artikel N")
# 3. welke notatie gebruikt de rest van dat hoofdstuk?
grep -n "^  - number:" <doelwet>.yaml | sed -n '1,80p'
```

Repareer **de kant die afwijkt van de rest van de wet**, niet reflexmatig de
`implements`. Als de hele wet positioneel nummert, past de `implements` zich aan;
als één hoofdstuk uit de pas loopt, is dat hoofdstuk het defect. Hernummeren
raakt `legal_basis`, `source`, `override` en scenario's — controleer die mee.

## `legal_character` zonder hook is meestal géén defect

Een artikel dat `legal_character: BESCHIKKING` produceert declareert daarmee dat
de Awb-hooks erop van toepassing zijn (motiveringsplicht, bezwaartermijn). Als er
in het geladen corpus geen enkele hook op dat karakter filtert, is de annotatie
nog steeds correct en toekomstvast — er luistert alleen nog niets.

Dat wordt pas een bevinding wanneer **documentatie of een diagram die hooks als
bestaande verbindingen toont**. Controleer dat expliciet:

```bash
grep -rn "3:46\|6:7\|hook\|motivering\|bezwaartermijn" docs/<dossier>/
```

Toont een diagram een pijl die het corpus niet waarmaakt, dan is het diagram het
defect, niet het corpus.

## Cijfercontrole van afgeleide documenten

Dezelfde reflex, andere richting: een handgeschreven inventarisatie (`.md`,
artifact, presentatie) die per regeling aantallen `cross`-verwijzingen,
`open_terms` of `untranslatables` noemt, drift weg van het corpus. Tel ze uit de
bron en vergelijk, in plaats van te lezen:

```bash
python3 - <<'PY'
import sys, yaml
from pathlib import Path
for p in sorted(Path(sys.argv[1]).rglob("*.yaml")):
    if p.name == "status.yaml" or "scenarios" in p.parts:
        continue
    d = yaml.safe_load(p.read_text()) or {}
    tel = {"open_terms": 0, "untranslatables": 0, "implements": 0}
    for a in d.get("articles") or []:
        mr = a.get("machine_readable") or {}
        for k in tel:
            tel[k] += len(mr.get(k) or [])
    if any(tel.values()):
        print(d.get("$id"), tel)
PY
```

Draai dit voordat je een aantal in een document overneemt, en opnieuw voordat je
het document oplevert.

## Veelgemaakte fouten

| Fout | Waarom mis |
|---|---|
| Alleen `implements` controleren | De open term aan de andere kant kan óók verweesd zijn, met een default die het verbergt |
| DOOD en ONBEANTWOORD op één hoop | Tientallen legitieme onbeantwoorde declaraties verdrinken de paar echte defecten |
| Het volledige corpus laden en dan concluderen | Boven de 100 geladen wetten stopt de engine; wat erna komt "bestaat" niet. Zie `regelrecht-bdd-lokaal` |
| De peildatum weglaten | Een koppeling kan op de ene versie resolven en op de andere niet |
| De `implements` aanpassen omdat dat het kleinste diff is | Als de doelwet inconsistent nummert, verplaats je het defect alleen |
| Concluderen dat de suite groen is, dus de koppelingen kloppen | Precies de blinde vlek die deze skill bestaat om te dekken |

## Verificatie voordat je een audit oplevert

- [ ] Peildatum expliciet genoemd, en het aantal geladen wetten staat in het rapport
- [ ] Elke DOOD-bevinding heeft een oorzaak, niet alleen een symptoom
- [ ] Elke ONBEANTWOORD-bevinding heeft een oordeel: legitiem of open actie
- [ ] Aantallen in het rapport zijn uit de corpus geteld, niet overgeschreven
- [ ] Na een reparatie: audit opnieuw gedraaid én de BDD-suite gedraaid — een
      hernummering die de koppeling repareert kan `legal_basis` of scenario's breken
