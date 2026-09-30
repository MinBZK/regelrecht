# Voorbereiding juristsessie: tekstcontrole, gaten in het stelsel, en wat de RFC's toelaten

Feitelijke onderbouwing onder [`juristsessie-draaiboek.md`](juristsessie-draaiboek.md).
Gemeten op 20 september 2026 tegen de branch
`traject/financieel-cv-validatie-df48ddd1` en de geldende wettekst op
wetten.overheid.nl.

Drie vragen, drie delen: klopt de tekst die we hebben, welke regelgeving
ontbreekt er nog voor het geheel, en kunnen de relaties die we nodig hebben met
de huidige RFC's.

---

## Deel 1: Klopt de tekst nog

### Wat ik heb gedaan, en wat niet

De harvester heeft in deze omgeving geen toolchain (`cargo` ontbreekt), dus
opnieuw inwinnen kon niet. In plaats daarvan heb ik de vergelijking gedaan
waar een herharvest uiteindelijk tegen wordt afgerekend: per gemodelleerd
artikel de geconsolideerde tekst opgehaald van
`wetten.overheid.nl/<bwb_id>/<valid_from>`, de YAML-tekst ontdaan van de
markdown-verwijzingen (`[artikel 3][ref1]` wordt `artikel 3`, de
`[ref1]: https://...`-regels vervallen), witruimte genormaliseerd volgens de
spiegelregel uit `law-version-drift-check`, en woord voor woord gediff'd.

Twee beperkingen die in het rapport horen. De skill eist calibratie tegen
vooraf vastgelegde bekende drifts in `docs/drift-ijkpunten.md` van het corpus;
dat bestand bestaat niet, dus dit rapport is **ongekalibreerd**. En ik heb de
geconsolideerde weergave gebruikt, niet de Staatsblad-stack; voor artikelen met
hangende wijzigingen (zie hieronder) is dat het verschil tussen "wat nu geldt"
en "wat er aankomt".

### Uitkomst: 25 van de 25 artikelen tekstueel schoon

| Wet | YAML | Artikelen met `machine_readable` | Tekst gelijk |
|---|---|---|---|
| Ziektewet | 2026-07-01 | 29b | 1/1 |
| Participatiewet | 2026-07-01 | 8a, 10, 10b, 10c, 10da, 10e | 6/6 |
| Wajong | 2026-07-01 | 2:15, 2:20, 2:22, 2:24 | 4/4 |
| Wtl | 2026-01-01 | 2.1, 2.6, 2.14 | 3/3 |
| Wet WIA | 2026-07-01 | 4, 5, 23, 35, 37, 43, 47, 54 | 8/8 |
| WW | 2026-07-01 | 76a | 1/1 |
| Wfsv | 2026-07-01 | 38b, 38f | 2/2 |

Geen enkel artikel wijkt inhoudelijk af van de wettekst op de datum die de YAML
zelf claimt. Dat is het goede nieuws, en het betekent dat de sessie niet over
tekstfouten hoeft te gaan.

### Vier bevindingen die wél opgelost moeten worden

**1. De Wajong-artikelen dragen verkeerde nummers en dode ankers.** In de YAML
heten de vier gemodelleerde artikelen `135`, `140`, `142` en `144`. Dat zijn
posities in het bestand, geen wetsartikelen: het zijn 2:15, 2:20, 2:22 en 2:24.
De `url` wijst naar `#Artikel220`, terwijl het anker op wetten.overheid.nl
`#Hoofdstuk2_Afdeling5_Artikel2:20` is. De tekst klopt, de verwijzing ernaartoe
niet, en een jurist die een nummer wil natrekken komt op een lege pagina. De
oorzaak zit in de dubbelepuntnotatie van de Wajong; de zes andere wetten hebben
het probleem niet. Dit is desk-werk vóór donderdag, en het is precies wat een
herharvest met een verbeterde harvester zou moeten oplossen.

**2. Acht artikelen dragen een hangende wijziging.** Op ZW 29b, Pwet 8a, Pwet 10,
Wtl 2.1, Wtl 2.6, Wtl 2.14, WIA 43 en Wfsv 38b meldt wetten.overheid.nl
"wijziging(en) zonder datum inwerkingtreding aanwezig". Wtl 2.6 en 2.14 hebben
daarnaast een wijziging met datum: **1 januari 2027**. Onze tekst is dus juist
voor vandaag en verouderd op een moment dat al vaststaat. Dat is een vraag voor
de jurist, geen desk-vraag: modelleren we de geldende versie, of leggen we de
komende versie er alvast naast?

**3. Drie peildatums door elkaar in één dossier.** De zes wetten staan op
2026-07-01, de Wtl op 2026-01-01 en het Reïntegratiebesluit op 2026-05-02. Zolang
een wet een waarde uit een andere wet leest zonder datum mee te geven, leest hij
die uit een bestand van een andere peildatum. Zie deel 3, RFC-020.

**4. In de Wtl is paragraaf 2.2 vervallen per 1 januari 2026.** Het LKV voor
oudere werknemers bestaat niet meer. Dat past bij het LIV-verhaal uit de
host-briefing en het maakt de scope-vraag scherper dan hij in de README staat.

---

## Deel 2: Wat er nog ontbreekt voor het geheel

### De stand van het corpus

Het corpus telt **4138 ingewonnen regelingen**, waarvan er **8** een
`machine_readable`-sectie hebben: de zeven wetten van dit dossier plus het
Reïntegratiebesluit. Inwinnen is dus niet het knelpunt; modelleren wel.

### Regelgeving die er ligt maar niet is gemodelleerd

Deze staan als tekst in het corpus en kunnen zonder inwinning worden opgepakt.
Per regeling: wat het oplost, en van wie het antwoord moet komen.

| Regeling | Wat het invult | Wie |
|---|---|---|
| `besluit_loonkostensubsidie_participatiewet` | De loonwaardemethode onder Pwet 10c/10d, en daarmee de 50%-regeling en de evenredige vermindering | Desk, na een inhoudelijke bevestiging |
| `besluit_loondispensatie_wajong` | "Vermindering naar evenredigheid" en "duidelijk minder dan het minimumloon" uit Wajong 2:20 | Jurist bevestigt dat dit de juiste grondslag is, dan desk |
| `besluit_suwi` en `regeling_suwi` | De administratie van het doelgroepregister onder Wfsv 38b, waaronder de AMvB-indicatie van 38b lid 1 d | Desk |
| `wet_banenafspraak_en_quotum_arbeidsbeperkten` | De quotumsystematiek achter Wfsv 38f | Scope-besluit: hoort dit in een regelhulp voor een burger |
| `dagloonbesluit_werknemersverzekeringen` | Het dagloon onder ZW en WIA, dus de hoogte van uitkeringen | Scope-besluit |
| `wet_sociale_werkvoorziening` | Nu afgevangen met de parameter `is_wsw_werknemer` | Scope-besluit |
| `wet_minimumloon_en_minimumvakantiebijslag` | WML plus vakantiebijslag, de rekenbasis onder LKS | Desk |
| `wet_op_de_loonbelasting_1964` | Het loonbegrip waar Wtl en Wfsv op leunen | Desk |

### Regelgeving die er helemaal niet is

**De gemeentelijke verordening ex Pwet 8a.** De Participatiewet delegeert naar de
gemeenteraad, en de YAML legt dat correct vast als `open_term` met
`delegated_to: gemeenteraad`. Maar het corpus kent alleen `nl/wet`, `nl/amvb`,
`nl/beleidsregel` en `nl/waterschaps_verordening`: geen enkele gemeentelijke
verordening. De `law-download`-skill kan CVDR aan en RFC-010 (Federated Corpus)
is aanvaard en geïmplementeerd, dus het kan. De vraag is welke gemeente je kiest,
want het antwoord verschilt per gemeente. Dat is een ontwerpvraag voor het
dossier en niet voor de jurist alleen.

**UWV-beleidsregels.** De map `nl/beleidsregel` bevat niets van UWV. Juist de
open normen waar de jurist het over gaat hebben, "reëel uitzicht", "structurele
functionele beperking", de loonwaardebepaling, worden in de praktijk daar
ingevuld. Vraag aan de jurist: welke beleidsregel of welk protocol is hier
leidend, en heeft het een vindplaats die wij kunnen inwinnen. Eén vindplaats is
hier meer waard dan drie meningen.

---

## Deel 3: Kunnen de relaties met de huidige RFC's

Schema v0.5.4, dat deze branch gebruikt, kent de velden `implements`,
`open_term(s)`, `hooks`, `overrides`, `source`, `legal_basis`,
`legal_character`, `actions` en `definitions`. De nieuwste schemaversie is
v0.7.0, drie minor versies verder.

### Wat we nodig hebben, en of het kan

| Relatie die het stelsel vraagt | Mechanisme | RFC | Status | In dit dossier |
|---|---|---|---|---|
| Waarde uit een andere wet lezen | `input` met `source.regulation` + `output` | RFC-007 | Aanvaard, deels geïmplementeerd | **18 aanroepen**, werkt |
| AMvB vult een open norm | `open_term` + `implements` in de AMvB | RFC-003 | Aanvaard, geïmplementeerd | **1 keer**, correct aan twee kanten |
| Verordening vult een open norm | `open_term` met `delegated_to` | RFC-003 + RFC-010 | Aanvaard, geïmplementeerd | Term ligt er, de verordening niet |
| Meerdere uitvoerders in één keten | `competent_authority` per output | RFC-009 | Voorgesteld, deels | UWV, gemeente en Belastingdienst staan erin |
| Beschikking en Awb-gevolgen | `legal_character` | RFC-002, RFC-008 | Aanvaard; 008 deels | `legal_character` in alle zeven |
| Bedragen en afronding | quantities, rounding | RFC-023, RFC-024 | Voorgesteld, geïmplementeerd | Beschikbaar, nog niet gebruikt voor de Pwet-afronding |
| Rekenen over een verzameling personen | collection operations | RFC-016 | Aanvaard, geïmplementeerd | Nodig voor Wfsv 38f en "evenwichtig verdeeld" |
| Vervallen regeling | `valid_to`, verlopen verwijzingen | RFC-019 | Aanvaard, geïmplementeerd | Past op LIV en op Wtl §2.2 |
| Onbekend tegenover afwezig | absent/unknown | RFC-036 | Voorgesteld, geïmplementeerd | Relevant voor "geen recht" tegenover "niet vastgesteld" |
| Herleidbare uitkomst tonen | provenance, traces | RFC-013, RFC-039 | Aanvaard; 039 deels | De trace is het sessie-instrument |
| **Verwijzen naar een wet op een peildatum** | `as_of` | **RFC-020** | **Concept, niet geïmplementeerd** | Knelpunt, zie hieronder |
| **Samenloop als stelselregel** | geen | geen | ontbreekt | Nu proza in untranslatables |

### De twee echte knelpunten

**Datum-bewuste verwijzing ontbreekt.** RFC-020 is concept en niet
geïmplementeerd. Een `source`-aanroep noemt alleen de regeling en de output, geen
peildatum. Dit dossier heeft drie peildatums door elkaar, dus een WIA-waarde van
1 juli wordt gelezen door een Wtl-model van 1 januari. Voor de twee persona's
maakt dat vandaag geen verschil, maar de constructie is niet houdbaar zodra een
wet halverwege het jaar wijzigt, en dat gebeurt bij vier van de acht artikelen
met een hangende wijziging. Dit is geen juristvraag: het is een engine-gat dat
als RFC-tekst thuishoort, en de jurist kan er hooguit voor bevestigen hoe erg
het is.

**Samenloop heeft geen mechanisme.** De Ziektewet-YAML zegt het zelf: samenloop
met LKV, LKS en loondispensatie is "een eigenschap van het stelsel, geen regel
binnen dit artikel". Er is geen constructie die een stelselregel uitdrukt.
`hooks` en `overrides` bestaan in het schema maar worden in geen van de zeven
wetten gebruikt, en ze zijn bedoeld voor reactieve interactie tussen twee
regelingen, niet voor een regel boven het stelsel. Wat de jurist donderdag
bevestigt over cumulatie kan dus wel worden vastgelegd, maar niet worden
uitgevoerd. Zeg dat er hardop bij, anders bevestigt hij iets waarvan hij denkt
dat het daarna in de applicatie zichtbaar wordt.

### Wat de migratie naar v0.7.0 verandert

v0.7.0 schrapt `untranslatables`, `construct`, `enables`, `defaults`, `for`,
`interface` en `suggestion`, en voegt onder meer `markings`, `about`,
`resolution`, `resolved_by`, `target`, `voids`, `collection`, `filter`,
`combine`, `nullable` en `applies_from` toe. De 64 untranslatables van dit
dossier moeten dus hoe dan ook door een sorteerslag, en RFC-031 bepaalt waar ze
heen gaan: een `marking` is een taalgat, niet een openstaande rechtsvraag. Dat
is de reden dat het draaiboek de sessie met die sortering laat beginnen.

---

## Deel 4: Wat er nodig is om dit in het echt te laten werken

De zeven wetten rekenen niet met de wereld, maar met 107 parameters die iemand
moet aanleveren. Precies: **107 parameters komen van buiten** en **15 waarden
komen uit een andere wet** via een `source`-aanroep. Die verhouding is het
antwoord op de vraag welke omliggende wetten nodig zijn: niet als tekst om te
citeren, maar als grondslag voor de gegevens en voor het besluit.

### De gegevens en hun herkomst

De volledige lijst staat in [`gegevensherkomst.md`](gegevensherkomst.md): 120
gegevens, elk met type, de wetten die het gebruiken, en de bron waar het
vandaan zou moeten komen. Samengevat:

| Herkomst | Aantal |
|---|---|
| Oordeel of registratie van een uitvoerder | 78 |
| Waarde uit een andere wet, via een `source`-aanroep | 15 |
| Basisregistratie (BRP, doelgroepregister, justitie) | 11 |
| Werkgever of loonaangifte | 10 |
| Aanvraag of wilsuiting van burger of werkgever | 5 |
| Bedrag uit de wet | 1 |

Per organisatie: **UWV 52, gemeenten 18, Belastingdienst 6, SVB 1**. Vijf van de
120 zijn een handeling van de burger of de werkgever; al het andere ligt elders
vast of is door een uitvoerder geveld.

### De ringen om de zeven wetten heen

**Ring 1, de inhoud van de norm.** Regelgeving die de open normen invult, en
zonder welke de uitkomst een schatting blijft: het Besluit loonkostensubsidie
Participatiewet, het Besluit loondispensatie Wajong, het Reïntegratiebesluit
(al gemodelleerd), het Besluit en de Regeling SUWI, het Dagloonbesluit, de
gemeentelijke verordening ex Pwet 8a, en de UWV-beleidsregels achter de open
normen. Zie deel 2: op de verordening en de beleidsregels na liggen ze in het
corpus.

**Ring 2, de bevoegdheid om te beslissen.** De **Algemene wet bestuursrecht**
is de grote ontbrekende. Aanvraag, beschikking, motivering, beslistermijn,
bezwaar en beroep zijn in de zeven wetten wel als `legal_character` benoemd,
maar de Awb zelf is ingewonnen en niet gemodelleerd. RFC-008 (Awb Administrative
Procedures) is aanvaard en deels geïmplementeerd, en er draait een
`awb-parity-test` in CI, dus het fundament ligt er. Wil je van een berekening
een besluit maken, dan is dit de eerste wet die erbij moet.

**Ring 3, de gegevens en de grondslag om ze te gebruiken.** De **Wet SUWI** voor
de polisadministratie, het doelgroepregister en de gegevensuitwisseling tussen
UWV en gemeenten; de **Wet BRP** voor persoon en woonplaats; de **AOW** voor de
pensioengerechtigde leeftijd; de **WML** voor het bedrag; de **Wet LB 1964**
voor het loonbegrip waar Wtl en Wfsv op leunen; en de **AVG met de UAVG** voor
de vraag of je deze gegevens bij elkaar mag brengen. Alle zes liggen als tekst
in het corpus, geen ervan is gemodelleerd.

### Het mechanisme dat hiervoor ontbreekt

Een parameter kan vandaag niet zeggen waar hij vandaan komt. RFC-017 (Native
Data Source Metadata) beschrijft precies dat, en staat op **concept, niet
geïmplementeerd**. Gevolg: de 107 parameters hebben geen herkomst in het model,
en het verschil tussen "dit weet de burger", "dit staat in de polisadministratie"
en "dit is een oordeel van een arbeidsdeskundige" leeft alleen in de hoofden van
de modelleurs.

Dat is een concreet agendapunt voor de sessie en voor de roadmap tegelijk. Voor
de jurist: welke van deze gegevens mag een uitvoerder op grond waarvan
hergebruiken. Voor de engine: zonder RFC-017 kan het antwoord nergens worden
vastgelegd.

---

## Deel 5: De poort, gemeten

De Rust-engine draait in deze omgeving wel degelijk; `cargo` stond alleen niet
in `PATH`. Met de musl-toolchain en de zig-wrappers bouwt de engine in 37
seconden en zijn de 815 unittests groen. Daarmee kon de poort uit het draaiboek
echt worden gedraaid, en die staat op rood.

### `just validate`: twee van de acht bestanden falen

Beide zijn schema-valide en stranden op de typecheck (RFC-036, RFC-037):

| Bestand | Artikel | Melding |
|---|---|---|
| `wet_werk_en_inkomen_naar_arbeidsvermogen/2026-07-01.yaml` | 23 | `wachttijd_einddatum`: [N3] IF zonder default kan null opleveren; geef een default of maak de output nullable |
| `wet_arbeidsongeschiktheidsvoorziening_jonggehandicapten/2026-07-01.yaml` | 2:15 | `ingangsdatum_recht_op_arbeidsondersteuning`: [N2] een letterlijke null mag alleen de waarde zijn van een nullable output |

De melding stelt "maak de output nullable" voor, en dat kan hier niet: `nullable`
bestaat pas in schema **v0.7.0** en deze branch staat op **v0.5.4**, waar het
veld nul keer voorkomt. Er zijn dus twee routes, en het is een modelleerkeuze
welke: een `else`-tak of default in de `IF`, of de branch naar v0.7.0 tillen en
de outputs eerlijk als nullable declareren. Inhoudelijk is de tweede de juiste,
want een wachttijd-einddatum bestáát niet als er geen wachttijd loopt, en dat is
precies het onderscheid dat RFC-036 maakt.

### Bucket A: 40 van de 76 scenario's slagen

`BDD_BUCKET=corpus` over een mini-corpus met de acht dossierwetten (45
wetsversies, ruim onder de engine-cap van 100) geeft 21 features, 76 scenario's,
**40 geslaagd en 36 gefaald**, 377 van de 413 stappen groen.

De 36 hebben één oorzaak, en het is de typecheck hierboven. De WIA laadt niet,
dus elke cross-law-aanroep erheen faalt: met alleen de nieuwste versies in de
corpus met `Law not found: wet_werk_en_inkomen_naar_arbeidsvermogen`, en met
historische versies erbij met `Output 'heeft_recht_op_iva_uitkering' not found`,
omdat de oudere WIA-versies geen `machine_readable` hebben. Eén kapot bestand
zet dus een derde van de wetsvalidatie stil, en het valt niet op zolang niemand
de suite draait.

Dat is meteen het antwoord op de vraag of deze sessie validerend kan zijn. Nu
niet. Met deze twee reparaties wel, en dan is bucket A de eerste echte meting
van wat het model doet.

### Zo draai je het zelf

```bash
# eenmalig per shell: cargo staat niet in PATH, en er is geen systeem-C-toolchain
export PATH="$HOME/.cargo/bin:$HOME/.local/bin:$PATH"
RUSTLIB="$HOME/.rustup/toolchains/1.96.0-aarch64-unknown-linux-gnu/lib/rustlib/aarch64-unknown-linux-gnu/bin"
export CARGO_TARGET_AARCH64_UNKNOWN_LINUX_MUSL_LINKER="$RUSTLIB/rust-lld"
export RUSTFLAGS="-C linker-flavor=ld.lld -C link-self-contained=yes"
export CARGO_BUILD_TARGET=aarch64-unknown-linux-musl

script/validate.sh <pad naar de acht bestanden>
REGULATION_PATH=<mini-corpus> BDD_BUCKET=corpus cargo test -p regelrecht-engine --test bdd
```

De mini-corpus is nodig omdat de engine maximaal honderd wetsversies laadt en
het corpus er duizenden heeft. Neem per wet de nieuwste versie onder elke
peildatum die in de scenario's voorkomt; dat waren er hier 45. `REGULATION_PATH`
wijst naar de map **boven** `nl/`, en stuurt zowel het laden van de wetten als
het vinden van de features.

---

## Wat dit betekent voor donderdag

**Vóór de sessie, desk:**

1. Wajong-artikelnummers en ankers rechtzetten. Vier artikelen, en het raakt elke
   verwijzing die de jurist zelf wil natrekken.
2. De twee typecheck-fouten uit deel 5 repareren (WIA artikel 23 en Wajong
   artikel 2:15). Zonder die twee laadt de WIA niet en faalt 36 van de 76
   wetsvalidatie-scenario's.
3. Besluiten of de Wtl mee wordt getild naar 2026-07-01, of dat de peildatum
   bewust op 2026-01-01 blijft staan met een genoteerde reden.
4. Een `docs/drift-ijkpunten.md` aanleggen met de vier bevindingen hierboven, dan
   is de volgende driftcontrole wel te kalibreren.
5. De restgroep van 42 parameters uit deel 4 indelen naar bron. Dat is
   invulwerk, geen denkwerk, en het maakt de vraag aan de jurist over
   gegevensgebruik concreet.

**Aanpassingen in de agenda van het draaiboek:**

- Scope-besluit S3 over het Reïntegratiebesluit vervalt als vraag: het besluit is
  ingewonnen en heeft een `implements`-blok naar WIA 35 en Wajong 2:22, met
  passende `open_terms` aan beide kanten. Wat overblijft is de check of de
  juiste grondslag is gekozen. Open vraag 9 in de README is daarmee verouderd.
- Er komt een cluster bij over de **hangende wijzigingen** op acht artikelen,
  waaronder de Wtl-wijziging per 1 januari 2027.
- Het cluster over open normen krijgt een concretere vraag: niet "wat betekent
  dit", maar "welke beleidsregel vult dit in en waar staat die", want het corpus
  heeft geen enkele UWV-beleidsregel.
- Bij cluster 2 hoort de mededeling dat samenloop vastgelegd kan worden maar niet
  uitgevoerd, zolang er geen mechanisme voor is.
