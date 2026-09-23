# Hoe diep gaat een variabele

**Datum:** 23 september 2026 · **Uitgewerkt voorbeeld:**
`is_uitgesloten_beschut_werk_pwet_10b`, de chapeau-uitsluiting van Wfsv 38b

Een parameter in een `machine_readable`-blok is een bodem die je zelf legt. Waar
die bodem ligt bepaalt of een uitkomst herleidbaar is tot de wet of tot de
aanleveraar. Dit document trekt één variabele zo ver mogelijk uit, en gebruikt
dat om te bepalen voor welke van de 120 gegevens die oefening loont.

## De ladder, helemaal uitgetrokken

| Niveau | Wat er ligt | Stand |
|---|---|---|
| 0 | Wfsv 38b leest de waarde | sinds 23 september een `source`-aanroep |
| 1 | Participatiewet 10b levert `is_uitsluitend_aangewezen_op_beschut_werk` | gemodelleerd, pass-through van de collegevaststelling |
| 2 | Pwet 10b lid 2 en 3: het UWV verricht de werkzaamheden op grond van een AMvB en adviseert het college | open term, AMvB niet in het corpus |
| 3 | **Besluit advisering beschut werk, BWBR0035947** | **bestaat, geldend sinds 1 juli 2022, niet in het corpus** |
| 4 | Besluit uitvoering sociale werkvoorziening en begeleid werken, artikel 4 lid 3, in de redactie van vóór de Invoeringswet Participatiewet | **in het corpus**, versie 2012-07-01 |
| 5 | Wajong 2:5, het arbeidsvermogenonderzoek | wet zit al in het dossier, artikel niet gemodelleerd |
| 6 | Wet SUWI 73a, de tweede grondslag van het besluit | in het corpus, niet gemodelleerd |
| 7 | Besluit SUWI 3.2 lid 3: de registratie eindigt de dag ná de vaststelling | in het corpus, niet gemodelleerd |

Eerder stond in dit document dat niveau 3 onbereikbaar was. Dat klopte niet: ik
had één BWB-nummer gegokt, kreeg een 404 en concludeerde te snel. Het besluit is
gevonden via de SRU-dienst van `repository.overheid.nl` en daarna door de
BWB-nummerreeks van eind 2014 af te zoeken.

## Wat er op niveau 3 staat

Het besluit is klein, vijf artikelen, en artikel 3 is een volwaardige
beslisregel — geen open norm.

**Artikel 2.** Het UWV adviseert *"binnen acht weken nadat het hiertoe van het
college of van de persoon een verzoek heeft ontvangen"*. Een harde termijn, die
in het model nergens voorkomt.

**Artikel 3 lid 1.** Twee onderzoeksvragen: is de persoon bij het verrichten van
werkzaamheden aangewezen op

- *"een of meer technische of organisatorische aanpassingen die niet binnen redelijke grenzen door een werkgever kunnen worden gerealiseerd"*, of
- *"permanent toezicht of intensieve begeleiding die niet binnen redelijke grenzen door een werkgever kan worden aangeboden"*.

**Artikel 3 lid 2.** *"Uitsluitend indien"* ten minste één van beide bevestigend
wordt beantwoord, adviseert het UWV positief. Dat is een limitatieve
voorwaarde, precies de vorm die het formaat aankan.

**Artikel 3 lid 3 en 4 — twee kortsluitingen.** Zonder onderzoek beide vragen
**ontkennend** bij een Wsw-indicatie met een eerder advies "in staat tot
begeleid werken", of bij een eigen aanvraag binnen twaalf maanden na een eerder
advies. En zonder onderzoek ten minste één vraag **bevestigend** bij een
Wsw-indicatie met een advies "niet in staat tot begeleid werken".

**Artikel 3 lid 5 en 6 — twee derogaties.** Toch onderzoek wanneer de aanvrager
*"nieuw gebleken feiten of veranderde omstandigheden"* vermeldt.

**Artikel 3 lid 7 — hergebruik van gegevens.** Het UWV mag gegevens gebruiken uit
een onderzoek op grond van Wajong 2:5 uit de twee voorafgaande jaren.

## Wat dat betekent

De bodem is niet "het UWV beslist". De bodem is **twee vragen met een
maatstaf** — *"niet binnen redelijke grenzen door een werkgever"* — met daarbovenop
een beslisboom van kortsluitingen en derogaties die volledig gebonden is.

Van de acht niveaus is er dus precies één waar echt menselijk oordeel zit: de
invulling van "redelijke grenzen" in artikel 3 lid 1. Al het andere is regel.

Dat is het tegenovergestelde van wat de parameternaam suggereert. Eén boolean
`is_uitgesloten_beschut_werk_pwet_10b` verbergt een besluit met vijf artikelen,
een termijn van acht weken, twee kortsluitingen, twee derogaties en een
hergebruikclausule.

## Twee dingen die de tijdlijn raken

**Er ligt een wijziging voor.** Op 24 februari 2026 is een ontwerpbesluit tot
aanpassing van dit besluit voorgehangen bij de Tweede Kamer (Kamerstuk 34352,
nr. 351, met beslisnota's). En wetten.overheid.nl meldt bij artikel 3 een
*"toekomstige wijziging voorzien met ingang van 1 juli 2028"*. Wie dit
modelleert, modelleert een bepaling die beweegt.

**Niveau 4 vraagt om een historische versie.** Artikel 3 lid 3 en 4 verwijzen
naar het Besluit uitvoering sociale werkvoorziening *"zoals die artikelen
luidden vóór de inwerkingtreding van artikel II van de Invoeringswet
Participatiewet"*. Het corpus heeft dat bestand in drie versies, waaronder
`2012-07-01.yaml` — de redactie van vóór 2015. Een verwijzing naar een bevroren
oude tekst is precies waar RFC-020 (`as_of`) voor bestaat, en die is concept.

## Wat een source-aanroep kost

De Ziektewet doet dit al in artikel 29b:

```yaml
- name: verricht_arbeid_in_beschut_werk
  type: boolean
  source:
    regulation: participatiewet
    output: verricht_arbeid_in_beschut_werk
    parameters:
      bsn: $bsn
      behoort_tot_doelgroep_10b_lid_1: $behoort_tot_doelgroep_10b_lid_1
      college_heeft_vastgesteld_uitsluitend_beschut_werk: $is_uitgesloten_beschut_werk_pwet_10b
      heeft_dienstbetrekking_beschut_werk: $heeft_dienstbetrekking_beschut_werk
```

Een aanroep haalt de waarde uit de bronwet, maar de parameters van die bronwet
moet de aanroeper alsnog meeleveren. Dieper modelleren levert dus niet vanzelf
minder invoer op; het levert **herleidbaarheid** op. Minder invoer ontstaat pas
wanneer een niveau de waarde echt uitrekent — en dat is precies wat niveau 3 hier
zou doen.

De fideliteitsaudit voegde daar een correctie aan toe: de aanroep die op 23
september is gelegd geeft twee parameters mee die de chapeau niet nodig heeft,
`behoort_tot_doelgroep_10b_lid_1` en `heeft_dienstbetrekking_beschut_werk`. Die
horen bij de andere output van 10b.

## Moet dit voor alle 120?

Nee. Deze oefening kostte een half uur onderzoek en raakte zeven regelingen. Maal
120 is niet in verhouding, en voor de meeste gegevens levert het niets op: een
geboortedatum uit de BRP heeft geen ladder.

Wat wel loont is een **goedkope voorselectie** over alle 120, met drie vragen die
grotendeels uit de bestaande indeling te beantwoorden zijn:

1. **Wijst de wettekst een lagere regeling aan die deze waarde invult?** Zo nee, dan is de bodem bereikt en is de vraag alleen nog wie het feit levert.
2. **Staat die regeling in het corpus?** Zo ja, dan is het modelleerwerk. Zo nee, dan eerst inwinnen — en zoals dit voorbeeld laat zien is "niet in het corpus" iets anders dan "bestaat niet".
3. **Is wat overblijft een oordeel of een regel?** Artikel 3 van dit besluit ziet er van bovenaf uit als een oordeel en is van binnen een beslisboom. Dat verschil zie je pas als je kijkt.

De kandidaten die nu al bekend zijn, uit de indeling in
[`gegevensherkomst.md`](gegevensherkomst.md) en de
[`fideliteitsaudit.md`](fideliteitsaudit.md):

| Gegeven of norm | Lagere regeling | In corpus |
|---|---|---|
| Beschut werk, Pwet 10b | Besluit advisering beschut werk, BWBR0035947 | nee |
| Kan het WML verdienen, Wfsv 38b a en e | Besluit SUWI 3.5, drempelfuncties | ja |
| Loonwaarde, Pwet 10d | Besluit loonkostensubsidie Participatiewet | ja |
| Evenredige vermindering, Wajong 2:20 | Besluit loondispensatie Wajong | ja |
| Dagloon, ZW 29b lid 5 en WIA | Dagloonbesluit werknemersverzekeringen | ja |
| WML plus vakantiebijslag | Wet minimumloon, artikel 8 en 15 | ja, ook gemodelleerd in corpus-poc |
| AMvB-indicatie, Wfsv 38b lid 1 d | onbekend, nog niet gezocht | onbekend |

Zes van de zeven liggen er al. Dat is een afgebakende lijst, geen programma van
120.

De les uit dit ene blokje is niet dat alles dieper moet, maar dat **"niet in het
corpus" geen eindpunt is**. Drie keer in twee dagen bleek iets bereikbaar dat als
afwezig genoteerd stond: Besluit SUWI 3.5, de WML in `corpus-poc`, en nu dit
besluit.
