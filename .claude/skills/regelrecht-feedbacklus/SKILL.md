---
name: regelrecht-feedbacklus
description: >
  Houdt de feedbacklus met externe experts (juristen, uitvoerders, beleidsmakers) over
  meerdere rondes navolgbaar: onbewerkte feedback bewaren, per ronde verwerken, acties
  doorlopend bijhouden, en terugkoppelen in taal die de expert leest. Gebruik dit bij een
  tweede of latere validatieronde op hetzelfde dossier, wanneer feedback binnenkomt per
  mail of chat, wanneer een expert een artikelnummer noemt dat niet klopt met de
  gemodelleerde versie, wanneer een expert aangeeft een vraag niet te begrijpen, of
  wanneer niet meer te achterhalen is waarom iets anders is gebouwd dan gevraagd.
  Waar feedback aan één vindplaats hangt is de editor-notitie (RFC-005/RFC-018) de
  drager; de bestanden dragen de rest. Dossier-agnostisch; `templates/` bevat kant-en-klare skeletten. Voor de producten
  rond één sessie: zie regelrecht-audit-products.
allowed-tools: Read, Write, Edit, Grep, Glob, Bash, AskUserQuestion
---

# De feedbacklus — meerdere rondes met dezelfde expert

`regelrecht-audit-products` levert de producten *rond één sessie*: draaiboek,
audit-checklists, testcases, verslag intern en extern. Deze skill dekt wat daarna
komt: ronde twee, drie en verder, met dezelfde expert, op hetzelfde dossier.

Wat daar misgaat is niet inhoudelijk maar administratief. De feedback wordt
meteen vertaald naar onze artikelnummers en onze modelleerbegrippen, en dan is de
oorspronkelijke formulering weg. Twee rondes later is niet meer vast te stellen
wat er letterlijk gevraagd is — alleen nog wat wij ervan gemaakt hebben.

## Kernregel: verbatim eerst, dan pas interpreteren

Bewaar elke feedback woordelijk voordat je hem verwerkt. Inclusief typefouten,
inclusief afkortingen, inclusief artikelnummers waarvan je denkt dat ze niet
kloppen. **Juist die.**

Twee terugkerende gevallen waarom:

- **De expert noemt een artikelnummer dat in de gemodelleerde versie ergens anders
  over gaat.** Dat heeft drie mogelijke oorzaken — de expert vergiste zich, wij
  lazen het verkeerd, of het nummer is verschoven tussen hun versie en de onze — en
  ze vragen elk een ander antwoord. Zonder de letterlijke tekst zijn ze niet meer
  uit elkaar te houden. Binnen de editor speelt dit niet: zie
  [de notitie als drager](#de-notitie-is-de-drager).
- **De expert begrijpt onze vraag niet.** Dat is een signaal over onze
  formulering, niet over de wet. Het kost een hele ronde en levert geen inhoud op.
  Zie [de jargontabel](#terugkoppeling-zonder-modelleerjargon).

In een gestructureerde notitie zien die twee er identiek uit: een bevinding die
niet klopt. In de onbewerkte tekst zijn ze te scheiden.

## De notitie is de drager

Waar de feedback aan één vindplaats in de tekst hangt, hoort hij als **notitie**
in de editor en niet in een markdown-bestand. RFC-005 en RFC-018 zijn allebei
`Accepted` en `Implemented`; de editor kan notities ook aanmaken
(`NoteCreator.vue`, tekstselectie → notitie), via
`PUT /api/trajects/{ref}/corpus/laws/{law_id}/annotations`.

Wat dat oplevert boven een bestand:

| Eigenschap | Wat het voor de lus betekent |
|---|---|
| `TextQuoteSelector` op de tekst | De notitie volgt het fragment als het artikel hernummerd wordt. Artikelnummers zijn in de resolutie **niet gezaghebbend** (RFC-018 §4) — ze worden alleen als hint bewaard |
| `creator` + gezagsniveau (gezaghebbend · adviserend · persoonlijk · gegenereerd) | Herkomst als veld, in plaats van een rol in proza |
| `motivation: questioning` | "Hier zit een open interpretatiekwestie" — de vorm van vrijwel elke expertvraag |
| `motivation: commenting` | Een opmerking zonder openstaande vraag |
| Taak-schakelaar met open/afgerond | De primitief onder het actieregister, per notitie |

### De valkuil: `visibility`

Een notitie met `regelrecht:visibility: personal` gaat naar Postgres, gesleuteld
op het account van de schrijver, **nooit naar git**. Elke andere waarde gaat naar
de gedeelde sidecar-YAML op de branch van het traject en kan een PR opleveren.
De GET voegt de persoonlijke notities van de aanroeper zichtbaar gemarkeerd toe,
dus in de editor zien ze er hetzelfde uit.

**Feedback van een expert hoort altijd gedeeld.** Staat hij op `personal`, dan is
hij onzichtbaar voor iedereen behalve de schrijver, komt hij niet in het corpus en
niet in een PR — precies het stille verlies waar deze skill tegen bestaat.
Controleer dit voordat je een ronde afsluit.

### Wat de bestanden blijven dragen

- Feedback die per mail of chat binnenkomt, vóór iemand hem in de editor zet
- Opmerkingen die niet aan één vindplaats hangen (scope, planning, aanpak)
- De afwijkingstabel: gevraagd versus gebouwd, met reden
- Het ronde-ritme zelf, en de terugkoppeling

De poort die aftekenen tegenhoudt zolang er zwaarwegende notities open staan is
nog niet gebouwd — RFC-034 staat op `Draft` / `Not implemented` en benoemt dat
die poort alleen in een methodedocument bestaat. Tot die tijd is de
**Openstaand**-tabel in het actieregister die poort.

## Beginnen op een nieuw dossier

```bash
mkdir -p docs/<dossier>/<opdrachtgever>
cp .claude/skills/regelrecht-feedbacklus/templates/ruwe-feedback.md \
   .claude/skills/regelrecht-feedbacklus/templates/actieregister.md \
   docs/<dossier>/<opdrachtgever>/
cp .claude/skills/regelrecht-feedbacklus/templates/rondenotitie.md \
   docs/<dossier>/<opdrachtgever>/<datum>-<onderwerp>.md
```

Vul de `<...>`-plaatshouders in. Gebruik voor personen een **rol**, geen naam:
deze repo is publiek.

## De vier bestanden

Eén map per dossier, bijvoorbeeld `docs/<dossier>/<opdrachtgever>/`. De
`templates/`-map naast deze skill bevat de drie skeletten; hieronder staat waar
elk bestand voor is.

| Bestand | Inhoud | Wanneer bijwerken |
|---|---|---|
| `ruwe-feedback.md` | Feedback letterlijk, per ronde, met datum. Plus de afwijkingstabel. | Zodra feedback binnenkomt, vóór verwerking |
| `<datum>-<onderwerp>.md` | Gestructureerde rondenotitie: per bevinding wat we aantroffen en wat er is gewijzigd | Tijdens de verwerking |
| `actieregister.md` | Doorlopend, alle rondes, één regel per actie | Bij elke statuswijziging |
| terugkoppeling | Wat er terug naar de expert gaat, zonder modelleerjargon | Aan het eind van de ronde |

### `ruwe-feedback.md`

Nieuwste ronde bovenaan, citaten in blockquote, onaangeraakt. Skelet:
`templates/ruwe-feedback.md`.

De afwijkingstabel is het punt van dit bestand. "Gebouwd zoals gevraagd" hoeft er
niet in; alles waar we van afweken wel, met de reden. Zonder die tabel leest een
latere ronde onze keuzes als een misverstand.

### `actieregister.md`

Één tabel per ronde, doorlopend genummerd `<ronde>.<volgnummer>`, en onderaan een
samenvatting van wat openstaat. Skelet: `templates/actieregister.md`.

Statuswaarden — gebruik deze vijf, geen andere:

| Status | Betekenis |
|---|---|
| `open` | Nog niets aan gedaan |
| `loopt` | In behandeling |
| `gedaan` | Klaar, met bewijsplaats in de kolom *Waar* |
| `belegd` | Bij iemand buiten het team; wij wachten |
| `vervallen` | Niet meer van toepassing, met reden |

`gedaan` zonder iets in *Waar* is niet `gedaan`. De kolom moet naar een bestand,
commit of scenario wijzen dat het waarmaakt.

## Terugkoppeling zonder modelleerjargon

Onze woorden zijn geen Nederlands. Schrijf op wat het in de praktijk betekent, en
**wie het gegeven moet aanleveren**.

| Niet schrijven | Wel schrijven |
|---|---|
| aangeleverd feit | gegeven dat de gebruiker of <bronsysteem> moet invullen |
| afgeleide waarde | waarde die het model zelf uitrekent |
| open term | punt waar de wet naar een lagere regeling verwijst; die regeling bepaalt de invulling |
| untranslatable | bepaling die we niet kunnen doorrekenen, en waarom |
| leaf-parameter | invoerveld |
| resolvet niet | de verwijzing komt niet aan; er wordt met een standaardwaarde gerekend |
| legal_character | wat voor besluit dit juridisch is |
| cross-law reference | verwijzing naar een andere wet |

Toets voor elke zin: **zou de expert deze vraag kunnen beantwoorden zonder ons
YAML te kennen?** Zo nee, herschrijven. Een onbegrepen vraag kost een hele ronde
en levert geen informatie op.

Bij een vraag over een nieuw of gewijzigd artikel: zet erbij welke gegevens dat
artikel nodig heeft en wie ze levert. Dat is meestal precies waar de vraag over
gaat.

## De ronde

1. **Vastleggen.** Feedback letterlijk in `ruwe-feedback.md`, met datum en
   herkomst. Vóór alles. Hangt een punt aan één vindplaats, zet het daarnaast als
   gedeelde notitie op die tekst — `motivation: questioning` bij een openstaande
   vraag, `commenting` bij een opmerking.
2. **Toetsen tegen het corpus.** Alleen nodig voor feedback die buiten de editor
   binnenkwam; een notitie op de tekst hangt al op de goede plek. Elk genoemd
   artikelnummer opzoeken in de *gemodelleerde versie* — niet in de huidige
   wettekst, niet uit het hoofd.
   ```bash
   python3 -c "
   import yaml,sys
   d=yaml.safe_load(open(sys.argv[1]))
   for a in d['articles']:
       if str(a.get('number'))==sys.argv[2]:
           print(a.get('text','')[:400])
   " <wet>/<versie>.yaml 8d
   ```
   Klopt het niet, dan is dat een bevinding op zichzelf: noteer in de
   rondenotitie wat de expert noemde, wat daar in onze versie staat, en welk
   artikel wel bedoeld lijkt. Corrigeer het citaat in `ruwe-feedback.md` **niet**.
3. **Verwerken.** Rondenotitie: per bevinding wat we aantroffen en wat er is
   gewijzigd, met verwijzing naar wet-YAML, scenario of commit.
4. **Registreren.** Acties in `actieregister.md`, statussen bijwerken, ook die van
   eerdere rondes. Notities die een actie zijn: taak-schakelaar aan, open of
   afgerond bijhouden.
5. **Terugkoppelen.** Zonder jargon. Wat is er veranderd, wat blijft open, welke
   vraag ligt terug bij hen.
6. **Afwijkingen vastleggen.** Alles waar we van de letterlijke vraag afweken, met
   reden, in de afwijkingstabel.

## Veelgemaakte fouten

| Fout | Gevolg |
|---|---|
| Feedback direct gestructureerd opschrijven | De oorspronkelijke formulering is weg; latere correcties zijn niet navolgbaar |
| Een fout artikelnummer stilzwijgend corrigeren | Niet meer te zien of de expert zich vergiste of wij verkeerd lazen |
| Modelleerjargon in de terugkoppeling | Een onbegrepen vraag kost een ronde en levert niets op |
| Per ronde een nieuw actielijstje | Acties uit ronde 1 verdwijnen ongemerkt |
| Een expertnotitie persoonlijk opslaan | Onzichtbaar voor de rest, nooit in git; het stille verlies dat deze skill moet voorkomen |
| `gedaan` zonder bewijsplaats | Bij navraag niet terug te vinden |
| Alleen vastleggen wat is gebouwd | Waar we bewust van afweken leest later als een misverstand |
| Het artikelnummer opzoeken in de huidige wettekst | De expert en het model kunnen op verschillende versies zitten |

## Verificatie voordat je een ronde afsluit

- [ ] Elk citaat staat letterlijk in `ruwe-feedback.md`, met datum en herkomst
- [ ] Geen enkele expertnotitie staat op `regelrecht:visibility: personal` — die
      komt niet in het corpus en niet in een PR
- [ ] Elk genoemd artikelnummer is opgezocht in de gemodelleerde versie
- [ ] Elke bevinding staat in de rondenotitie met wat er is gewijzigd
- [ ] Elke actie staat in het actieregister; statussen van eerdere rondes bijgewerkt
- [ ] Elke `gedaan` heeft een bestand, commit of scenario in de kolom *Waar*
- [ ] De terugkoppeling bevat geen term uit de jargontabel hierboven
- [ ] Elke afwijking van de letterlijke vraag staat in de afwijkingstabel, met reden
