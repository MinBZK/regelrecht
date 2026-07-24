# Pre-read — juristvalidatie Financieel CV

**Sessie:** donderdag 23 juli 2026 · **Doel:** valideren of onze modellering trouw is aan de wettekst · **Duur:** ± 60-75 min

Dank dat je meedoet. Deze pre-read is bewust kort: hij vertelt wat we
donderdag doen, wat we van jou vragen, en welke vragen je vooraf al even
kunt laten bezinken. Je hoeft niets te installeren of voor te bereiden —
alleen je juridische blik meenemen.

---

## Waar dit over gaat

We hebben zeven regelingen uit de RVO-regelhulp **Financieel CV**
(instrumenten voor werkgevers/werknemers met een afstand tot de
arbeidsmarkt) vertaald naar **machine-leesbare, toetsbare formules**. Een
engine rekent die per persoon door: gegeven iemands situatie zegt het
model welk recht ontstaat, op welke grondslag, en welk bedrag daaruit
volgt.

De regelingen: **NRP** (no-riskpolis, Ziektewet 29b), **LKV**
(loonkostenvoordeel, Wtl 2.1), **LKS** (loonkostensubsidie, Pwet 10c/10d),
**LDP** (loondispensatie, Wajong 2:20), **JC/WPA** (jobcoaching en
werkplekaanpassing, WIA 35), **PP** (proefplaatsing, WW 76a). *LIV*
(lage-inkomensvoordeel) is per 2025 afgeschaft (Wet 36458) — die zit er
alleen nog historisch in.

### De gedeelde grondslag: het doelgroepregister banenafspraak

Onder meerdere regelingen ligt één begrip: hoort iemand in het
**doelgroepregister banenafspraak**? Dat is bij ons niet als losse "Wet
banenafspraak" gemodelleerd, maar op de plek waar de wet het regelt —
**Wet financiering sociale verzekeringen (Wfsv), artikel 38b**
(BWBR0017745). Eén keer gemodelleerd, door de andere regelingen
aangeroepen (cross-law): de **NRP** (Ziektewet 29b lid 2.e) en de **LKV**
(Wtl categorie banenafspraak) hangen hun banenafspraak-status hieraan op.

Wat we in 38b hebben gevangen:

- **Opname-gronden 38b.1.a–f** (Pwet-LKS-toeleiding, WSW-indicatie,
  Wajong-arbeidsondersteuning, AMvB-indicatie, eigen-verzoek-WML-route),
  plus 38b.2 (UWV-oordeel jonggehandicapt) en 38b.6 (blijf-grond).
- Kern-outputs: `behoort_tot_doelgroepregister_banenafspraak`,
  `datum_opname_doelgroepregister` + `grond_opname` (relevant voor de
  LKV-driejaarstermijn), en `vaststelling_door` (UWV).
- Een expliciete uitsluiting: **beschut werk (Pwet 10b) hoort níét in het
  register** — dat is een apart traject, geen arbeidsbeperkte in de zin
  van 38b.

*Voor de jurist:* dit is precies waar de banenafspraak-status van Koen én
Sadee vandaan komt — een goede plek om te toetsen of onze opname-gronden
en de beschut-werk-uitsluiting de wet trouw volgen.

## Wat we van jou vragen

De kernvraag is telkens dezelfde: **klopt onze formule met de wettekst?**

- Per uitkomst tonen we drie dingen naast elkaar: het **wettekst-citaat**,
  de **formule in gewone taal**, en **wat de engine eruit rekent**. Jij
  toetst of de vertaling klopt — ja, nee, of nuance.
- **Het juridische oordeel is aan jou.** De engine-uitkomst is een feit
  (dit rekent het model nu), geen standpunt. Waar wij iets verkeerd lezen,
  willen we dat horen.
- **Je hoeft geen code te lezen.** Alles staat in gewone taal en wettekst.
- Waar het model een norm bewust **niet** vangt (wij noemen dat een
  *untranslatable* — bijv. een open norm of een UWV-discretie), willen we
  jouw duiding: is dat terecht open gelaten, en wat is de meetlat?

Wat er nu ligt: schema-valide en **127 geautomatiseerde scenario's groen**.
Dat betekent "intern consistent doorgerekend", **niet** "juridisch
bekrachtigd" — dat laatste is precies wat we donderdag samen doen.

## De twee verhalen die we doorlopen

We hangen de sessie op aan twee fictieve personen (geen echte
persoonsgegevens). Elk is een verticale doorsnede door de regelingen — zo
zie je de hele keten langs één menselijk verhaal.

**Koen** — 42 jaar, Pwet-doelgroep / banenafspraak, loonwaarde 60% WML,
in dienst bij een logistiek MKB via de gemeente:

| Regeling | Uitkomst (engine) | Grondslag |
|----------|-------------------|-----------|
| NRP (Ziektewet 29b) | Recht, duur **onbeperkt** | lid 2.e — banenafspraak + LKS |
| LKS (Pwet 10c/10d) | Recht, **€ 862 / maand** | loonwaarde 60% van WML+VB |
| LKV (Wtl 2.1) | Recht, **€ 1.680,64 / jaar** | categorie c — banenafspraak |
| JC + WPA (WIA 35) | **Recht** (aanvraag bij UWV) | geen uitsluiting lid 4 |
| LDP (Wajong 2:20) | Geen recht | geen Wajong-status |
| PP (WW 76a) | Geen recht | geen WW-uitkering |
| LIV (Wtl h.3) | Bestaat niet meer | afgeschaft per 2025 |

**Sadee** — Wajong-uitkering / banenafspraak, loonwaarde 70% WML:

| Regeling | Uitkomst (engine) | Grondslag |
|----------|-------------------|-----------|
| NRP (Ziektewet 29b) | Recht, duur **onbeperkt** | lid 2.a (Wajong) + lid 2.e |
| LDP (Wajong 2:20) | Recht; loonbeding **nietig** | arbeidsprestatie < WML |
| LKV (Wtl 2.1) | Recht, **€ 5.075,20 / jaar** | categorie b — arbeidsgehandicapt (wint, art. 4.1 lid 3) |
| JC + WPA (WIA 35) | **Geen recht** | lid 4.a sluit Wajong uit → Wajong-eigen route |
| LKS (Pwet) | Geen recht | geen Pwet-doelgroep |
| PP (WW 76a) | Geen recht | geen WW-uitkering |
| LIV (Wtl h.3) | Bestaat niet meer | afgeschaft per 2025 |

## De scherpe contrasten — hier zit jouw waarde

De twee personen zijn zo gekozen dat ze bij dezelfde instrumenten
**tegengesteld** uitkomen. Dat zijn de plekken waar de routeringslogica
(welk lid, welke uitsluiting) staat of valt:

1. **JC/WPA — WIA art. 35 lid 4.a.** Koen krijgt het (via UWV), Sadee
   niet: als Wajonger valt zij buiten art. 35 en loopt het via
   Wajong-eigen voorzieningen (art. 2:22 e.v., nog niet gemodelleerd).
   *Klopt die uitsluiting en de doorverwijzing?*
2. **LKV-hoogte — Wtl art. 4.1 lid 3.** Sadee voldoet aan twee
   categorieën; wij passen "hoogste bedrag wint" toe → € 5.075
   (arbeidsgehandicapt) in plaats van € 1.680 (banenafspraak).
   *Klopt die voorrangsregel — de wet/MvT is er niet expliciet over.*
3. **NRP-duur onbeperkt.** Beiden komen op "onbeperkt" uit, maar via een
   ander lid (Koen 2.e, Sadee 2.a + 2.e). *Klopt de lid-toewijzing?*
4. **Samenloop.** We rekenen per regeling onafhankelijk "recht = true".
   De samenloopverboden — LKS ↔ LKV (Pwet 10d lid 9) bij Koen, LIV ↔ LKV
   (Wtl 4.1 lid 3) bij Sadee — zitten nu **nog niet** in het model.
   *Hoe moet die samenloop juridisch uitpakken?* (open punt)

## Open vragen — kun je vooraf over nadenken

Uit onze MvT-analyse; per regeling de scherpste (volledige lijst in
`mvt-referenties.md`):

- **NRP:** beschut werk (Pwet 10b) was in MvT 34194 (2015) *uitgesloten*,
  nu via lid 2.f *ingesloten* — welke lezing geldt nu?
- **LKS:** loonwaarde mét of zonder vakantiebijslag? En de 50%-regeling
  eerste 6 maanden (lid 5) — hoe zwaar weegt die?
  De vraag aan de jurist: moet het Financieel CV die eerste zes maanden apart tonen, of volstaat het structurele bedrag? En hoe vaak loopt het in de praktijk via lid 1.b?
  1.b is de route waarin gemeente en werkgever samen besluiten de loonwaardemeting over te slaan zodat iemand meteen aan de slag kan. Je ziet de
  koppeling ook letterlijk in de tekst: 1.a verwijst naar lid 4, 1.b naar lid 5. Dat is precies waarom lid 5 die 50%-forfait kent — er ís nog geen
  gemeten loonwaarde om het verschil mee te berekenen. Na die periode stelt het college de loonwaarde alsnog vast en gaat lid 4 gelden.
- **JC/WPA:** de 2-jaars-/geen-LKS-toets (lid 4.b) staat nu als
  *untranslatable* — hoe zou jij die lezen?
- **LDP:** "duidelijk minder dan minimumloon" — welke meetlat hanteert
  UWV? En: lid 2 nietigheid modelleren wij als harde constante `true` —
  akkoord?
- **LKV:** bij meerdere categorieën tegelijk zwijgt de MvT over de
  volgorde; wij nemen "hoogste bedrag wint" (zie contrast 2).

## Agenda donderdag

1. Kort — wat is regelrecht en hoe lees je één uitkomst (± 10 min)
2. **Koen** helemaal door (± 20 min)
3. **Sadee** helemaal door, met de contrasten hierboven (± 20 min)
4. Open vragen + wat naar een vervolgsessie gaat (± 15 min)

## Praktisch

- **Meenemen:** je blik op de genoemde artikelen — Ziektewet 29b, Wtl 2.1
  / 4.1, Pwet 10c/10d, Wajong 2:20, WIA 35, WW 76a, en Wfsv 38b
  (doelgroepregister banenafspraak).
- **Niets installeren:** we tonen alles live én op papier.
- **Willen verdiepen?** `output-walkthrough.md` (wettekst + formule per
  output) en `mvt-referenties.md` (volledige bronnen en open vragen)
  liggen klaar, maar zijn geen verplichte kost.
