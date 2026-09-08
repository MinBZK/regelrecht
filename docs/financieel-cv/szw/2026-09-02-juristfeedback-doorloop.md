# SZW — juristfeedback op de doorloop Koen en Sadee

**Datum:** 2 september 2026 · **Bron:** feedback op het doorloop-artifact
"Koen en Sadee door het stelsel" · **Status:** verwerkt in de corpus op
branch `traject/financieel-cv-validatie-df48ddd1`.

De feedback staat hieronder in citaatblokken, maar onbewerkt en compleet in
[ruwe-feedback.md](ruwe-feedback.md) — inclusief de punten waarop onze
uitwerking bewust afwijkt van de letterlijke tekst (art. 7, art. 8d, "2-4
maanden").

Twee bevindingen, allebei van hetzelfde type als bevinding 2 uit de
[juristvalidatie van 23 juli](2026-07-23-juristvalidatie-notities.md): het
model toonde "geen recht" of "niet van toepassing" waar het recht wél
bestaat, alleen op een andere plaats in het stelsel. Beide zijn tegen de
wetstekst van de 2026-07-01-versies gecontroleerd voordat er iets is
gewijzigd.

---

## Bevinding 1 — WIA art. 35 sluit ook de Participatiewet uit

### Wat de jurist zei

> Artikel 35 WIA sluit ook de Participatiewet uit (lid 4b, echt een
> superomslachtige formulering). Scenario Koen heeft op grond daarvan dus
> geen recht op voorziening/jobcoach/werkplekaanpassing. Maar dat recht is
> er wel gebaseerd op P-wet art. 7-8d / 10e (ingewikkelde hier is: gemeente
> heeft opdracht, maar precieze regels moet gemeente vastleggen in
> verordening; alleen recht op jobcoach voor doelgroep LKS is hard
> vastgelegd in P-wet art. 10da).

### Wat we aantroffen

De **modellering** van WIA art. 35 was al goed. De parameter
`pwet_college_draagt_zorg_uitsluiting` bestond, met een geaccepteerde
`untranslatable` over de samengestelde tijds- en geldcomponent (twee
aaneengesloten jaren minimumloon zonder LKS).

Het **scenario** was fout. `financieel_cv_koen.feature` bij de WIA zette die
parameter op `false` en concludeerde dat Koen jobcoaching en
werkplekaanpassing bij UWV kon aanvragen. Koen ontvangt algemene bijstand en
is via de gemeente in dienst gekomen — het college draagt dus zorg op grond
van Pwet art. 7 lid 1 onderdeel a, en lid 4.b sluit hem uit. Met een
loonwaarde van 60% en lopende loonkostensubsidie zit hij ver van de
tweejaarsgrens af.

De **gemeentelijke route ontbrak volledig**. In de Participatiewet hadden
alleen art. 10b (beschut werk) en 10c (LKS) een `machine_readable`. Een
correctie van het WIA-scenario alleen zou het Financieel CV dus voor de hele
Participatiewet-doelgroep ten onrechte op "geen recht" hebben gezet.

### Wat er is gewijzigd

| Artikel | Wat | Sterkte |
|---|---|---|
| Pwet art. 10 | Aanspraak op ondersteuning bij arbeidsinschakeling, op persoonlijke ondersteuning (lid 1 slotzin) en op de noodzakelijk geachte voorziening | Aanspraak staat in de wet, maar "overeenkomstig de verordening" — vorm, duur en intensiteit zijn gemeentelijk |
| Pwet art. 10da | Aanspraak op begeleiding op de werkplek voor de doelgroep LKS | **Hard.** Eén zin, geen voorbehoud, geen delegatie |
| Pwet art. 8a | Verordeningsplicht (lid 1) en de proefplaatsing van lid 2 onderdeel d | Opdracht is hard, inhoud is gemeentelijk |
| Pwet art. 10e | Uitsluitend `open_terms` — vier AMvB-grondslagen, alle kan-bepalingen | Geen execution: zolang er geen AMvB is verandert 10e niets aan art. 10 lid 1 |
| WIA art. 35 | Koen-scenario gecorrigeerd naar `pwet_college_draagt_zorg_uitsluiting = true`, met spiegelscenario voor ná de tweejaarsgrens | — |

Art. 7 zelf is **niet** gemodelleerd: het is een taakopdracht aan het
college, geen aanspraak van de burger. De aanspraak zit in art. 10 lid 1, en
daar is de doelgroepomschrijving van art. 7 lid 1 onderdeel a in verwerkt.
Art. 8d viel bij controle buiten beeld — dat artikel gaat in de
2026-07-01-versie niet over voorzieningen; de verordeningsplicht die de
jurist bedoelt staat in art. 8a.

### Wat dit voor de presentatielaag betekent

Er zijn nu **twee sterktes van aanspraak** in de gemeentelijke keten, en dat
onderscheid moet zichtbaar blijven. Aan art. 10da kun je een toezegging
hangen; aan art. 10 lid 1 niet, want daar bepaalt de gemeenteraad de
voorwaarden. Het Financieel CV moet voor Koen dus niet "recht op jobcoach"
tonen alsof dat een UWV-beschikking is, maar: harde aanspraak op begeleiding
op de werkplek (10da) plus een gemeentelijke route waarvan de invulling
lokaal is (10 lid 1).

---

## Bevinding 2 — proefplaatsing staat in vier wetten, met verschillende kaders

### Wat de jurist zei

> Proefplaatsing zit ook in meerdere wetten: P-wet artikel 8a lid 2d; Wajong
> art. 2:24; WIA art. 37). Dus niet alleen in de WW waar nu op wordt
> getoetst. Waarbij de kaders soms ook nog eens kunnen verschillen
> (proefplaatsing Wajong bijvoorbeeld max 6 maanden, terwijl in P-wet
> afgebakend tot 2-4 maanden…). Makkelijker konden we het kennelijk niet
> maken :/

### Wat we aantroffen

Alleen **WW art. 76a** had een `machine_readable`. Het Koen-scenario bij de
WW concludeerde terecht dat hij zonder WW-recht deze route niet in kan, maar
lichtte dat toe met "PP met behoud van uitkering is een WW-instrument" en
"voor Pwet geldt een ander re-integratie-traject via de gemeente". Dat eerste
is onjuist en het tweede te vaag: Pwet art. 8a lid 2 onderdeel d ís
proefplaatsing.

### Het vergelijkingsoverzicht

| Wet | Artikel | Duur | Wat er doorloopt |
|---|---|---|---|
| WW | 76a | 6 maanden | WW-uitkering |
| Wet WIA | 37 | 6 maanden | Sollicitatieplicht opgeschort (art. 30 lid 1 b) |
| Wajong | 2:24 | 6 maanden | Arbeidsondersteuning én inkomensvoorziening |
| Participatiewet | 8a lid 2 d | **2 maanden, verlengbaar met max. 4** | Algemene bijstand |

De voorwaarden van WW 76a lid 3, WIA 37 lid 2 en Wajong 2:24 lid 3 zijn
woordelijk gelijk (in staat tot de werkzaamheden, aansprakelijkheids-
verzekering, niet eerder bij dezelfde werkgever, reëel uitzicht op zes
maanden dienstbetrekking). De Participatiewet wijkt op elk punt af: kortere
basistermijn, alleen voor wie algemene bijstand ontvangt, en de voorwaarden
staan niet in de wet maar in de gemeentelijke verordening.

Alleen bij volledige verlenging komt de Participatiewet op dezelfde zes
maanden uit. Zonder verlenging is de gemeentelijke proefplaatsing een derde
van wat de werknemersverzekeringen bieden.

### Wat er is gewijzigd

- `machine_readable` op **Wet WIA art. 37**, **Wajong art. 2:24** en
  **Pwet art. 8a lid 2 onderdeel d**, met de duur als constante zodat het
  verschil toetsbaar is.
- Nieuwe featurebestanden `proefplaatsing.feature` bij de WIA, de Wajong en
  de Participatiewet.
- Het onjuiste commentaar in het WW-Koen-scenario vervangen door het
  vergelijkingsoverzicht hierboven. De assertie zelf klopte en is ongemoeid
  gelaten.

---

## Wat dit raakt buiten de corpus

Het doorloop-artifact "Koen en Sadee door het stelsel" toont voor Koen nog de
oude, onjuiste uitkomst bij jobcoaching en werkplekaanpassing, en behandelt
proefplaatsing nog als WW-instrument. Dat artifact moet opnieuw worden
gegenereerd zodra deze wijzigingen zijn doorgevoerd — zie actie 4 in het
[actieregister](actieregister.md).

De demo-branch `traject/financieel-cv-0bc401e0` waar de RVO-mensen op zitten
is **niet** gewijzigd. Die branch hangt zijn `machine_readable` op andere
wetsversies (2025-01-01 en 2026-01-01, schema v0.5.2), dus cherry-picken
werkt niet: elke wijziging landt op een ander doelbestand. Overzetten vraagt
een aparte branch en een PR, en de demo-zichtbare gevolgen moeten vooraf
worden gemeld — zie actie 5.

---

## Nog niet beantwoord

- Hoe de aanspraak van art. 10da (hard) zich verhoudt tot de persoonlijke
  ondersteuning van art. 10 lid 1 (verordening) en tot de kwaliteitseisen van
  art. 10e lid 2 onderdeel a. De wet zegt er niets over; dit is
  uitvoeringspraktijk. Staat als `untranslatable` bij 10da.
- Of het Financieel CV gemeentelijke verordeningen moet gaan laden. Zonder
  die verordeningen blijft de uitkomst voor de Participatiewet-route "de
  route bestaat", nooit een bedrag. Dat is een scopevraag, geen
  modelleervraag.
- Of proefplaatsing en loonkostensubsidie kunnen samenlopen. Dezelfde
  aggregator-vraag als bij LKS ↔ LKV (Pwet art. 10d lid 9), nog steeds open.
