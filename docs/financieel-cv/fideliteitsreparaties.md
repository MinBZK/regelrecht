# Reparatie van de fideliteitsbevindingen

**Datum:** 24 september 2026 · **Bron:** [`fideliteitsaudit.md`](fideliteitsaudit.md)
**Branch:** `traject/financieel-cv-validatie-df48ddd1` in regelrecht-corpus

Van de ruim zestig bevindingen benoemt de audit er ongeveer vijfendertig
concreet. Die zijn gerepareerd. De rest is uit het rapport niet te herleiden tot
een aanwijsbaar artikel en blijft open.

## Poorten na afloop

| Poort | Uitkomst |
|---|---|
| `script/validate.sh` over de acht bestanden | 8 van de 8 OK, schema v0.7.0 |
| `cross-law-integriteit.py` op de trajectcorpus | clean=22, alle andere tellers 0 |
| BDD bucket corpus | 76 van de 76 scenario's groen |

## Wat er is gerepareerd, per wet

### Participatiewet

- **Het model van de loonkostensubsidie stond onder artikel 10c**, dat alleen
  over de vaststelling van de doelgroep gaat; artikel 10d had geen model.
  Verplaatst naar 10d. Artikel 10c draagt nu de twee bevoegdheden van lid 1 en
  de termijn van lid 2 (een aanvraag kan slechts eenmaal per twaalf maanden).
- **Lid 1 en lid 2 van 10d waren tot één cumulatieve keten samengevoegd**,
  waardoor de route van lid 1 nooit waar kon worden. Het zijn twee zelfstandige
  grondslagen, met lid 3 als uitsluiting over beide.
- **De 70%-bovengrens rekende zonder de werkgeverslastenvergoeding**, terwijl
  lid 4 die uitdrukkelijk in de grondslag opneemt. De subsidie viel daardoor bij
  constructie te laag uit.
- **De 50%-regel van lid 5 stond als marking weggeschreven.** Zij is nu
  gemodelleerd als de route van lid 1 onderdeel b, begrensd op de eerste zes
  maanden, met terugval op lid 4 daarna.
- **De derde zin van lid 4** begrenst de arbeidsduur op de in de sector
  gebruikelijke volledige dienstbetrekking. Die begrenzing wordt toegepast zodra
  de open term is ingevuld.
- **Artikel 10b modelleerde de aanbiedingsplicht van lid 1 niet**, zodat lid 6
  nergens kon ingrijpen. De plicht is nu een uitkomst en vervalt zodra het aantal
  dienstbetrekkingen is gerealiseerd.
- **Artikel 10 maakte van de aanvraag van lid 5 een voorwaarde** voor het bestaan
  van de aanspraak. Lid 1 kent die onvoorwaardelijk toe; de aanvraag staat nu als
  eigen uitkomst ernaast.

### Wet financiering sociale verzekeringen

- **Onderdeel g van 38b lid 1 ontbrak**: WGA-uitkering met het bij wijze van
  experiment ingezette instrument van Wajong 2:20 of 3:63 (artikel 82a lid 1 Wet
  SUWI).
- **De uitsluiting uit de aanhef van lid 1 werd over alle gronden gelegd.** Lid 6
  herhaalt haar woordelijk, lid 2 kent haar niet. Zij geldt nu over lid 1 en lid 6.
- **De grondaanduiding kende geen geval voor lid 6**, zodat twee uitkomsten van
  hetzelfde artikel elkaar tegenspraken.
- **De kernuitkomst heette `behoort_tot_doelgroepregister_banenafspraak`**,
  terwijl 38b het begrip arbeidsbeperkte omschrijft en 38d de registratie regelt.
  Hernoemd naar `is_arbeidsbeperkte`.

### Ziektewet

- **Lid 8 ontbrak volledig.** De uitsluiting van de Wsw-dienstbetrekking in de
  zin van artikel 2 zet het hele artikel opzij en werkt nu door op recht en duur.
- **Lid 2 onderdeel e miste** dat de dienstbetrekking op of na 1 januari 2015 is
  aangevangen.
- **Lid 1 onderdeel b rekende vanaf de WIA-wachttijd**, terwijl de letter zes
  tijdvakken als alternatieven noemt. Er is nu een peildatum die het geldende
  tijdvak leest en op de wachttijd terugvalt.
- **De hoogte van het ziekengeld was niet gemodelleerd en niet gedeclareerd.**
  Lid 5 (70 procent van het dagloon) en lid 7 (vermindering met het naar
  werkdagen herleide Wsw-subsidiebedrag) staan er nu.
- Lid 2 onderdeel e leest voortaan de uitkomst van Participatiewet 10d **lid 2**,
  omdat de letter dat lid uitdrukkelijk noemt.

### Wet tegemoetkomingen loondomein

- **De drie bedragen werden over alle verloonde uren van het kalenderjaar
  gerekend.** De artikelen 2.9, 2.13 en 2.17 tellen alleen de uren van wie aan de
  voorwaarden van 2.6, 2.10 respectievelijk 2.14 voldoet. Elk bedrag heeft nu een
  eigen urengrondslag.
- **Artikel 2.10 lid 2 onderdeel b en lid 3 werden niet getoetst**: Wsw-arbeid in
  de zin van artikel 2 zonder terbeschikkingstelling, en het verstrijken van de
  periode van 2.12.
- **Artikel 2.6 lid 3 onderdeel c ontbrak**, terwijl het spiegelartikel 2.14
  lid 2 onderdeel c wel was gemodelleerd.

### Wet WIA

- **Artikel 35 lid 1 is een kan-bepaling.** De uitkomsten heten nu
  `mag_jobcoaching_toekennen` en `mag_werkplekaanpassing_toekennen`.
- **Lid 2 opent met "worden uitsluitend verstaan"** en werd niet getoetst: het
  enige lid-2-element in de formule was dat er een aanvraag lag. De materiële
  eisen van onderdeel c en d staan er nu naast.
- **Artikel 37 liet de IVA-gerechtigde toe tot de proefplaatsing.** Lid 1 richt
  zich tot de gedeeltelijk arbeidsgeschikte; artikel 5 sluit de volledig en
  duurzaam arbeidsongeschikte uit. Het model leest nu artikel 5 samen met het
  WGA-recht.

### Wajong

- **2:22 lid 1 richt zich tot "de jonggehandicapte".** Het model eiste
  daarbovenop recht op arbeidsondersteuning. Die voorwaarde is eruit; de
  uitkomsten zijn kan-bepalingen en de materiële eisen van lid 2 worden getoetst.
- **De parameter `is_volledig_en_duurzaam_arbeidsongeschikt` droeg dezelfde naam
  als in de Wet WIA**, terwijl artikel 2:4 lid 1 een andere maatstaf hanteert dan
  Wet WIA artikel 4. Hernoemd naar `_wajong`.
- **2:15 lid 2 miste het lid-1-moment**, waardoor een ingangsdatum vóór het
  achttiende jaar kon ontstaan.

### Werkloosheidswet en Reïntegratiebesluit

- **76a lid 2 verloor "onverminderd artikel 20, eerste lid, aanhef en onderdeel
  b"**, zodat de uitkering in het model doorliep na afloop van de uitkeringsduur.
- **De zes-maandentermijn was een losse constante.** Er is nu een einddatum die
  lid 4 met de ziektedagen verlengt. De marking op lid 4 vervalt: `DATE_ADD`
  bestaat, er ontbrak een invoerfeit.
- **De uitkomst heette `mag_proefplaatsing_aangaan`** en legde de bevoegdheid bij
  de werknemer. Nu `uwv_mag_toestemming_verlenen`.
- **Het Reïntegratiebesluit hing zijn `implements` aan artikel 1a**, dat
  uitsluitend grondslagen opsomt. Verplaatst naar artikel 4, waar de nadere regel
  staat.

## Twee bevindingen van de audit zijn zelf onjuist gebleken

**Open termen zijn niet altijd inert.** De audit stelt dat de motor een open term
alleen als symbool declareert. Dat geldt voor een open term zonder `default`. De
werkgeverslastenvergoeding van Pwet 10d draagt wél een `default`-blok met een
actie, en wordt sinds deze reparatie in een formule gebruikt; de typecheck
accepteert dat. Patroon 3 is dus smaller dan het rapport zegt.

**Een niet-doorgegeven optionele parameter levert geen `null` maar `Unknown`.**
Een afwezigheidstoets met `EQUALS … null` vangt dat niet, en de onbekende waarde
plant zich voort tot in het bedrag. De betrokken parameters zijn daarom verplicht
gemaakt, met `nullable: true` waar de waarde werkelijk kan ontbreken.

## Wat open blijft

- De verankering van de Wtl: artikel 2.1 draagt nog steeds de bedragen van 2.9,
  2.13 en 2.17 en de duur van 2.8, 2.12 en 2.16. Die artikelen tellen in elke
  corpustelling als niet gemodelleerd.
- De 23 open termen: per stuk kiezen of zij een actie gaan voeden of weg moeten.
  Gedeclareerd en ongebruikt is de slechtste van de drie toestanden.
- De drie juristkeuzen en de twee wettekst-gevolgen uit de audit. Die zijn geen
  deskwerk.
