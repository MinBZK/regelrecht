# Memorie-van-toelichting-referenties per regeling

Voor de workshop met de makers van de regelhulp Financieel CV en het
juridisch reviewmoment. Per regeling hieronder de MvT-bronnen die we
hebben aangetroffen, met de meest concrete passages over werking en
intentie. Citaten met dossiernummer en kamerstuk; volledige tekst via
de gelinkte URLs.

## NRP — No-riskpolis (Ziektewet artikel 29b)

**Hoofdbron:** Kamerstuk 34194 nr. 3 (MvT bij Wet harmonisatie
instrumenten arbeidsdeelname arbeidsbeperkten),
<https://zoek.officielebekendmakingen.nl/kst-34194-3.html>.

Concrete passages:

- **§ 2.1 — werkingsmechanisme.** "De no-riskpolis is een instrument
  in artikel 29b van de Ziektewet ... De uitkering bedraagt in beginsel
  100% van het dagloon in het eerste jaar van ziekte en 70% in het
  tweede jaar."
- **Lid 4-toelichting.** "Op basis van het vierde lid is de no-riskpolis
  tevens van toepassing als een werkgever de dienstbetrekking met een
  werknemer voortzet nadat de WIA-aanspraak is vastgesteld." (zie ook
  rb. ECLI-uitspraken bij toepassing lid 4)
- **Doelgroep banenafspraak (lid 2.e/2.f).** "Voor mensen waarvan UWV
  heeft vastgesteld dat ze het wettelijk minimumloon niet kunnen
  verdienen en die op grond van de Participatiewet bij de
  arbeidsinschakeling worden ondersteund."

**Aanvullende bron:** Verzamelwet SZW 2026 (internetconsultatie,
2025) — context over recente eigenrisicodragerschap-gevolgen,
<https://www.internetconsultatie.nl/verzamelwetszw2026/document/13670>.

**Open vragen voor jurist:**

1. § 2.1 noemt 100%/70% — onze YAML modelleert alleen recht-vraag,
   niet ziekengeldhoogte (lid 5/6). Akkoord?
2. Beschut werk (Pwet 10b) was tijdens MvT 34194 (2015) UITGESLOTEN,
   nu via lid 2.f INGESLOTEN. Welke MvT-passage geldt nog?
3. Tijdelijke 5-jaars-uitbreiding 2016-2021 voor langdurig werklozen
   is verlopen. Ontbreekt iets in onze YAML voor lid 1.b?

## PP — Proefplaatsing (Werkloosheidswet artikel 76a)

**Hoofdbron:** Kamerstuk 31767 nr. 3 (MvT bij Vergroten kansen op werk
voor langdurig werklozen),
<https://www.parlementairemonitor.nl/9353000/1/j9vvij5epmj1ey0/vi0197gisjrq>.

**Aanvullende bron:** Kamerstuk 33556 nr. 3 (Verzamelwet 2013, technische
correctie verwijzing lid 5),
<https://zoek.officielebekendmakingen.nl/kst-33556-3.html>.

Concrete passages:

- **Art. 76a + 20 lid 9 koppeling.** "Het recht op uitkering loopt
  ook tijdens het recht op loondoorbetaling bij ziekte als bedoeld in
  artikel 629 BW7 en artikel 76a, eerste lid, van de Ziektewet."
- **Geen substantiële beleidswijziging in 33556.** Technische
  redactiecorrectie in lid 5; inhoudelijk ongewijzigd.

**Open vragen voor jurist:**

1. Geen specifieke "rekenvoorbeelden" gevonden in MvT — duur is hard
   gecodeerd op 6 maanden (art. 76a lid 1). Akkoord met die literal?
2. Lid 4 (onderbreking wegens ziekte) — onze YAML behandelt dit als
   untranslatable; moet UWV dit echt day-by-day boekhouden?
3. Lid 5 (open_term ministeriële regeling) — welke regeling
   implementeert dit nu, indien aanwezig?

## LIV — Lage-inkomensvoordeel (Wtl artikel 3.1 + 3.2)

**Hoofdbron:** Kamerstuk 34304 nr. 3 (MvT bij Wet tegemoetkomingen
loondomein, 2015),
<https://www.parlementairemonitor.nl/9353000/1/j9vvij5epmj1ey0/vjxdibwsx4wq>.

**Afschaffing:** Kamerstuk 36458 (afschaffing LIV per 2025-01-01),
<https://www.eerstekamer.nl/wetsvoorstel/36458_wijziging_van_de_wet>.

Concrete passages:

- **Doel LIV.** "Een generieke fiscale subsidie op aannemen en in
  dienst houden van mensen met een loon tot maximaal 120% van het
  wettelijk minimumloon ... om de werkgeverskosten aan de onderkant
  van de arbeidsmarkt te verlagen."
- **Effectiviteit ondermaats (36458).** "Gegeven de beperkte
  effectiviteit van het lage-inkomensvoordeel wordt deze regeling
  met dit voorstel afgeschaft."

**Open vragen voor jurist:**

1. Onze YAML peildatum 2024-01-01 (LIV-laatste-jaar). Voor demo
   prima — voor productie zou dit een open_term moeten zijn met
   tijdgebonden geldigheid?
2. Bedragen 2024: € 14,33-€ 14,91 uurloongrenzen, € 0,49/uur, € 960
   max. Bron-link naar Regeling lage-inkomensvoordeel ontbreekt nog.

## LKV — Loonkostenvoordelen (Wtl artikel 2.1 + categorieën)

**Hoofdbron:** Kamerstuk 34304 nr. 3 (zelfde MvT als LIV).

Concrete passages:

- **Vier categorieën** (a t/m d uit art 2.1). "De loonkostenvoordelen
  zijn doelgroepgericht en vervangen de premiekortingen voor oudere
  werknemers en arbeidsgehandicapten ... realisatieprobleem
  (premiekorting werd niet altijd gerealiseerd) wordt opgelost door
  automatische uitkering via Belastingdienst."
- **Bedragen per categorie** (art. 2.7, 2.9, 2.13, 2.17): € 3,05/uur
  voor a, b, d (max € 6.000); € 1,01/uur voor c
  (banenafspraak, max € 2.000).

**Open vragen voor jurist:**

1. IF-volgordelogica voor categoriebepaling (oudere → arbeidsgehandicapt
   → herplaatsing → banenafspraak): wat zegt de wet als meerdere
   tegelijk gelden? MvT zwijgt expliciet hierover.
2. Doelgroepverklaring binnen 3 maanden (art. 2.3, 2.6) — onze YAML
   markeert dit als untranslatable. Moet dit als parameter terugkomen
   ('doelgroepverklaring_tijdig_aangevraagd')?
3. LKV banenafspraak (c) is per 2025 structureel geworden zonder
   doelgroepverklaring. YAML peildatum 2024 — verschillen overleggen?

## LKS — Loonkostensubsidie (Pwet artikel 10c + 10d)

**Hoofdbron:** Kamerstuk 33161 nr. 3 (MvT bij Wet werken naar vermogen,
voorganger Participatiewet) — minder direct toegankelijk.

**Wijzigingsbron:** MvT bij Wijziging Pwet 2024-25 m.b.t. uniformering
loonwaardevaststelling (zoek op "loonwaardebepaling
participatiewet" voor recente stukken).

Concrete passages: in onze YAML zelf zijn de hoogteformules direct uit
art. 10d lid 4 ontleend ("verschil tussen WML+VB en loonwaarde+VB,
ten hoogste 70%").

**Open vragen voor jurist:**

1. **Lid 5 50%-regeling eerste 6 maanden** zonder loonwaardevaststelling
   — hoe gaan we hiermee om in een toekomstige iteratie?
2. **Lid 7 jaarlijkse herziening** — moet dit als runtime-proces of
   in YAML?
3. **Lid 4 evenredigheid bij parttime <36 uur** — onze YAML
   normaliseert naar 36 uur. Is dat conform praktijk?
4. **Werkgeverslastenvergoeding** (open_term) — welke ministeriële
   regeling vult dit?

## LDP — Loondispensatie (Wajong artikel 2:20)

**Hoofdbron:** Kamerstuk 31780 nr. 3 (MvT bij Wajong 2010 — Bevorderen
participatie jonggehandicapten),
<https://www.parlementairemonitor.nl/9353000/1/j9vvij5epmj1ey0/vi0d8o1z62zu>.

**Aanvullende bron:** Kamerstuk 35213 (Harmonisatie Wajong-regimes 2019),
<https://www.eerstekamer.nl/wetsvoorstel/35213_verdere_activering>.

Concrete passages:

- **Werking loondispensatie.** "Werknemer levert ten gevolge van ziekte
  of gebrek arbeidsprestatie die duidelijk minder is dan minimumloon-
  rechtvaardigend. UWV vermindert de geldelijke beloning naar
  evenredigheid; werkgever betaalt verminderd loon, werknemer behoudt
  Wajong-aanvulling tot maatmanloon."
- **Reden niet-LKS-vervanging (35213).** "Toepassing LKS-instrument
  in Wajong leidt tot toenemende complexiteit. Loondispensatie wordt
  al jaren toegepast in de Wajong; bezwaren m.b.t. administratieve
  lasten gelden hier niet. Daarom blijft loondispensatie binnen Wajong."

**Open vragen voor jurist:**

1. **"Duidelijk minder dan minimumloon-equivalent"** — UWV-discretie.
   Welke meetlat hanteert UWV in de praktijk?
2. **Dispensatiepercentage** is open_term BELEIDSREGEL — welke
   beleidsregel implementeert dit nu (UWV-Beleidsregels
   loondispensatie)?
3. **Lid 2 nietigheidsclausule** — strikt dwingend recht; onze YAML
   produceert die als constante `true`. Akkoord?

## JC + WPA — Jobcoaching en werkplekaanpassingen (Wet WIA art. 35)

**Hoofdbron:** Kamerstuk 30034 nr. 3 (MvT bij Wet WIA, 2005).

**Aanvullende bron:** Reïntegratiebesluit (BWBR0018394), AMvB op grond
van art. 35 lid 5.

Concrete passages:

- **Lid 4 onderdeel b uitsluiting Pwet 7.1.a-cliënten.** "Voor zover
  voor diens ondersteuning bij arbeidsinschakeling op grond van
  artikel 7 lid 1 a Pwet het college zorg draagt ... tot het moment
  dat het inkomen ... gedurende 2 aaneengesloten jaren ten minste
  het minimumloon ... bedraagt en in die 2 jaren geen LKS verleend."
  Bedoeling: voorkomen dat zowel UWV als gemeente voor dezelfde persoon
  voorzieningen verstrekken.
- **Lid 2 onderdeel d ("noodzakelijke persoonlijke ondersteuning").**
  In Reïntegratiebesluit verder uitgewerkt als jobcoaching met
  taakgerichte begeleiding op de werkvloer.

**Open vragen voor jurist:**

1. **"Structurele functionele beperking"** — UWV-arts beoordeelt op
   basis van Schattingsbesluit. Welke meetlat?
2. **Lid 4 onderdeel b**: 2-jaars/LKS-toets — onze YAML markeert dit
   als untranslatable. Hoe modelleren in toekomstige iteratie?
3. **Reïntegratiebesluit** als implementing regulation — moeten we
   die alsnog harvesten en als `implements`-relatie aansluiten op
   `open_term nadere_regels_voorzieningen_artikel_35`?
4. **Werkgevers-subsidie (artikel 36) voor niet-meeneembare WPA** —
   nu nog niet in YAML. Apart artikel modelleren?

## Algemene observaties

- **MvT's zijn vooral werking en intentie**, weinig hard rekenwerk
  dat zich leent voor 1-op-1 BDD-conversie. Ons grootste
  rekenvoorbeeld kwam uit MvT 34194 § 2.1 (NRP + LKS-samenloop) en
  de hoogteformule LKS uit artikel 10d lid 4 zelf.

- **De tijdgebonden constructies** (lid 4 NRP "5 jaar vanaf
  vaststelling", lid 5 LKS "50% eerste 6 mnd", lid 4.b WIA-WPA
  "2 jaar minimumloon zonder LKS") komen consistent terug als
  untranslatable. In een toekomstige iteratie van regelrecht
  (DATE_DIFF + period-arithmetic operations) zouden deze
  modelleerbaar worden.

- **Discretionaire UWV-oordelen** ("structurele functionele
  beperking", "duidelijk minder", "naar het oordeel") zijn als
  parameter binnengehaald. De MvT geeft inhoudelijke richting maar
  geen meetlat — die zit in lagere regelgeving (Schattingsbesluit,
  Beleidsregels UWV).
