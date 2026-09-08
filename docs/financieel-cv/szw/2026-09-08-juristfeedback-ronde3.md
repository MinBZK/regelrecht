# SZW — juristfeedback ronde 3

**Datum:** 8 september 2026 · **Bron:** reactie op de terugkoppeling van het
modelleerwerk · **Status:** verwerkt in de corpus op branch
`traject/financieel-cv-validatie-df48ddd1`.

Vier punten. Eén modelleerfout, één begrijpelijkheidsprobleem aan onze kant,
en twee bevestigingen die de reden erbij geven. De onbewerkte tekst staat in
[ruwe-feedback.md](ruwe-feedback.md).

---

## Punt 1 — Wfsv 38b onderdeel c was een uitsluiting, maar is een insluiting

### Wat de jurist zei

> Wajongeren die duurzaam geen arbeidsvermogen hebben, vallen wel onder de
> banenafspraak als ze aan het werk zijn (Wsfv 38b, lid c: "[…] met dien
> verstande dat de persoon die duurzaam geen mogelijkheden tot
> arbeidsparticipatie heeft […] slechts wordt aangemerkt als arbeidsbeperkte
> indien die persoon arbeid verricht in een dienstbetrekking."

### Wat we aantroffen

Dit klopt, en ons model had het fout. De slotzin van onderdeel c was
gemodelleerd als een absolute uitsluiting:

```yaml
voldoet_aan_grond_38b_1_c = AND(
  heeft_wajong_arbeidsondersteuning_of_uitkering,
  NOT heeft_wajong_duurzaam_geen_mogelijkheden)
```

Ook de parameterbeschrijving zei het met zoveel woorden: *"Sluit grond
38b.1.c uit."* De wettekst zegt iets anders. "Slechts […] indien" is geen
uitsluiting maar een **voorwaardelijke insluiting**: wie duurzaam geen
mogelijkheden tot arbeidsparticipatie heeft valt buiten onderdeel c zolang
hij niet werkt, en telt wél mee zodra hij arbeid verricht in een
dienstbetrekking.

Dat is precies de groep waar de banenafspraak om draait, dus het gevolg was
niet cosmetisch: een werkende Wajonger zonder arbeidsvermogen viel bij ons
buiten het doelgroepregister, en daarmee ook buiten de LKV-categorie
banenafspraak (Wtl 2.1) en buiten de no-riskpolis via Ziektewet 29b lid 2
onderdeel e. Beide wetten halen die status via een cross-law-verwijzing bij
Wfsv 38b op.

### Wat er is gewijzigd

```yaml
voldoet_aan_grond_38b_1_c = AND(
  heeft_wajong_arbeidsondersteuning_of_uitkering,
  OR(NOT heeft_wajong_duurzaam_geen_mogelijkheden,
     verricht_arbeid_in_dienstbetrekking))
```

- Nieuwe parameter `verricht_arbeid_in_dienstbetrekking` bij Wfsv 38b, en
  doorgegeven vanuit de twee aanroepende wetten (Wtl en Ziektewet), die hem
  ook zelf declareren.
- De beschrijving van `heeft_wajong_duurzaam_geen_mogelijkheden` rechtgezet;
  daar stond het omgekeerde van wat de wet zegt. Ook de verwijzing
  uitgebreid: de wet noemt Wajong 1a:1 lid 1, 2:4 lid 1 én 3:8a lid 1, wij
  noemden alleen 1a:1.
- Drie scenario's bij `doelgroepregister_banenafspraak.feature` dekken samen de
  drie assen af: duurzaam geen mogelijkheden zonder werk (buiten onderdeel c),
  met werk (binnen onderdeel c), en een dienstbetrekking zonder Wajong-recht
  (opent niets — de slotzin werkt alleen binnen onderdeel c). Daarvan zijn er
  **twee nieuw**; het eerste bestond al en is hernoemd en aangescherpt, want de
  oude titel beweerde dat de uitsluiting absoluut was. De suite gaat daarmee van
  74 naar 76 scenario's.

**Onderdeel f is gecontroleerd en blijft ongewijzigd.** Dat kent een
vergelijkbare constructie, maar daar staat "met uitzondering van de persoon
[…] die duurzaam geen mogelijkheden tot arbeidsparticipatie *meer* heeft",
zonder dienstbetrekking-clausule. Dat is wél een absolute uitsluiting.

### Verificatie

De fix is rood-groen aangetoond: met de oude regel faalt precies het nieuwe
scenario *"Wajong met duurzaam geen mogelijkheden telt wel mee zodra hij
werkt"* (76 scenario's, 1 gefaald); met de nieuwe regel draaien alle 76 groen
(521 steps, 21 features, exitcode 0). Schemavalidatie v0.5.4 op alle drie de gewijzigde YAML's: nul
fouten.

---

## Punt 2 — onze vraag was niet te volgen

### Wat de jurist zei

> "Wat bij die nieuwe artikelen een aangeleverd feit is gebleven": deze vraag
> begrijp ik niet

Terecht. "Aangeleverd feit" is ons jargon voor een parameter: een gegeven dat
de engine niet zelf kan afleiden en dat dus van buiten moet komen — van de
gebruiker, van UWV, van de gemeente. Tegenover een waarde die de engine uit
andere artikelen berekent.

De vraag had moeten zijn: **welke gegevens moet iemand aanleveren voordat
deze artikelen een uitkomst kunnen geven, en wie heeft die gegevens?** Het
antwoord voor de artikelen uit ronde 2:

| Artikel | Aan te leveren gegevens | Wie heeft ze |
|---|---|---|
| Pwet 8a (proefplaatsing) | behoort tot doelgroep art. 7 lid 1 a; ontvangt algemene bijstand; college verleent toestemming | gemeente |
| Pwet 10 (voorzieningen) | ontvangt algemene bijstand; is WIA-uitstromer (34a/35/36); heeft Anw-nabestaandenuitkering; is niet-uitkeringsgerechtigde; valt onder lid 2 wegens voorziening; college acht voorziening noodzakelijk; kan taken niet verrichten zonder ondersteuning; aanvraag ingediend | gemeente, deels UWV |
| Pwet 10da (begeleiding werkplek) | behoort tot doelgroep loonkostensubsidie | gemeente |
| Pwet 10e | — geen; uitsluitend `open_terms`, geen berekening | (wacht op AMvB) |
| Wet WIA 37 (proefplaatsing) | heeft recht op WIA-uitkering; in staat tot de werkzaamheden; aansprakelijkheidsverzekering aanwezig; niet eerder proefplaatsing bij dezelfde werkgever; reëel uitzicht op zes maanden dienstbetrekking | UWV, werkgever |
| Wajong 2:24 (proefplaatsing) | idem, met arbeidsondersteuning in plaats van WIA-uitkering | UWV, werkgever |

Opvallend: geen van deze artikelen leidt iets af uit een andere wet. Alles
komt van buiten. Dat is de kern van de scopevraag over verordeningen (actie
2.7) — bij de gemeentelijke route is er geen enkel gegeven dat het stelsel
zelf kan leveren.

**Les voor de volgende terugkoppeling:** geen modelleerjargon. Niet
"aangeleverd feit", "untranslatable" of "open term", maar wat het in de
praktijk betekent.

---

## Punt 3 — waarom de doelgroepverklaring bij de WIA blijft

### Wat de jurist zei

> "De doelgroepverklaring is nu een harde eis geworden". Dat klopt. In de
> banenafspraak kon de doelgroepverklaring afgeschaft worden omdat het
> doelgroepregister gekoppeld kan worden met de polisadministratie (en
> daarmee de loonaangifte). Daar is een doelgroepverklaring dus niet meer
> nodig. Bij de WIA kan dat helaas (nog) niet, en blijft een
> doelgroepverklaring dus noodzakelijk.

Bevestiging, met de reden erbij. Het verschil tussen de LKV-categorieën is
**uitvoeringstechnisch, niet inhoudelijk**: waar het feit al in een register
staat dat aan de loonaangifte hangt, hoeft de werknemer het niet nog eens te
laten verklaren.

Vastgelegd bij `heeft_geldige_doelgroepverklaring_2_15` in
`wet_tegemoetkomingen_loondomein/2026-01-01.yaml`, en als kruisverwijzing in
de untranslatable over samenloop, waar tot nu toe alleen stond *dát* de
doelgroepverklaring bij de banenafspraak is afgeschaft.

Dit raakt ook de dataminimalisatie-redenering: een gegeven dat via een
registerkoppeling beschikbaar is, hoeft niet aan de burger gevraagd te
worden. Zie `dataminimalisatie.md`.

---

## Punt 4 — de drie aanvaarde bepalingen worden een disclaimer

### Wat de jurist zei

> "Drie niet-berekenbare bepalingen staan voorlopig op "aanvaard"". Lijkt me
> heel ingewikkeld dit op enige manier mee te nemen in de berekening omdat
> gebruiker dan die informatie zelf moet aanleveren. Mogelijk wel als een
> soort disclaimer meenemen in uiteindelijke tool.

Dit is een richtinggevende uitspraak, geen vraag. Het gaat om de drie
untranslatables bij de LKV in de Wtl:

1. het doelgroepverklaring-vereiste binnen drie maanden (art. 2.3, 2.6);
2. de twaalfmaanden-uitsluiting bij aanvang dienstbetrekking (anti-misbruik);
3. de samenloop met no-riskpolis, LKS en loondispensatie.

Alle drie blijven op `accepted: true` staan — de engine rekent door en de
bepaling blijft zichtbaar gemarkeerd. Wat verandert is de **bestemming**: ze
worden niet omgezet in extra invoervelden, maar horen thuis als disclaimer in
de uiteindelijke tool. De uitspraak is bij elk van de drie in de YAML
genoteerd, zodat dit niet opnieuw ter discussie komt bij een volgende ronde.

Dit sluit aan bij actie 2.6 (presentatielaag): net als het onderscheid tussen
een harde aanspraak en een aanspraak onder verordeningsvoorbehoud is dit iets
dat de tool moet tónen, niet berekenen.

---

## Wat dit raakt buiten de corpus

- Het doorloop-artifact moet opnieuw (actie 2.4) — een werkende Wajonger
  zonder arbeidsvermogen kreeg daar de verkeerde uitkomst.
- De presentatielaag krijgt er een derde ding bij om te tonen in plaats van
  te berekenen (actie 2.6): de disclaimer uit punt 4.
- De RVO-demobranch is niet gewijzigd. Punt 1 is een echte inhoudelijke fout,
  dus dit verhoogt de urgentie van actie 2.5.
