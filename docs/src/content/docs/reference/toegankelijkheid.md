---
title: Toegankelijkheid
description: Toegankelijkheidsverklaring voor de RegelRecht-documentatiesite (WCAG 2.1 AA, concept).
lang: nl
---

# Toegankelijkheidsverklaring

Deze verklaring beschrijft in hoeverre de documentatiesite van RegelRecht
voldoet aan de toegankelijkheidseisen voor overheidswebsites: WCAG 2.1
niveau AA, zoals vastgelegd in EN 301 549 en de verplichting onder het
Tijdelijk besluit digitale toegankelijkheid overheid. De verklaring geldt voor
de site die zowel op `regelrecht.rijks.app` als op `docs.regelrecht.rijks.app`
wordt aangeboden, inclusief de landingspagina en het aanmeldformulier.

**Dit is een concept.** De status hieronder is gebaseerd op een geautomatiseerde
toets in de bouwstraat en een handmatige controle door het team, niet op een
onderzoek door een onafhankelijke partij. Een formele audit en publicatie in het
register van DigiToegankelijk moeten nog plaatsvinden voordat dit een
rechtsgeldige verklaring is.

## Nalevingsstatus

Gedeeltelijk. De geautomatiseerde toets vindt geen fouten op de criteria die zij
kan meten, en de handmatige controle dekt de criteria die geen tool betrouwbaar
test. Er resteren bekende beperkingen, hieronder benoemd. Zolang er geen externe
audit is geweest, claimt RegelRecht geen volledige naleving.

## Hoe dit getoetst is

**Geautomatiseerd, bij elke wijziging.** Een toegankelijkheidstoets draait in CI
op elke wijziging die de documentatie raakt. De toets bouwt de site en draait
[pa11y-ci](https://github.com/pa11y/pa11y-ci) met twee onafhankelijke engines,
HTML_CodeSniffer en axe-core 4.11, tegen elke gegenereerde pagina (op dit moment
61). De URL-lijst wordt uit de build afgeleid, zodat een nieuwe pagina
automatisch meegetoetst wordt en niet stilletjes buiten de controle valt.
Dezelfde toets is lokaal te draaien met `just docs-a11y`.

**Handmatig, voor wat tools niet zien.** HTML_CodeSniffer en axe dekken de
klassieke criteria (contrast, labels, koppenstructuur, landmarks, alt-teksten),
maar niet alles. Het team controleerde daarnaast met de hand:

- alle nieuwe WCAG 2.2-succescriteria, over deze thema's: focus niet afgedekt
  (minimum en uitgebreid), focusverschijning, sleepbewegingen, klikdoelgrootte,
  consistente hulp, herhaalde invoer en toegankelijke authenticatie (minimum en
  uitgebreid);
- de werking met alleen toetsenbord, inclusief de sla-over-link en de
  focusvolgorde;
- de donkere modus, op contrast en leesbaarheid;
- weergave bij 200% zoom en 400% herschaling.

## Bekende beperkingen

Drie groepen elementen worden door de geautomatiseerde contrasttoets gemeld,
maar voldoen feitelijk wel. De engines kunnen het contrast er niet betrouwbaar
meten; het team heeft de werkelijke verhouding in de browser nagemeten en
gecontroleerd dat die ruim boven de eis ligt. Deze elementen zijn daarom
uitgesloten van de geautomatiseerde meting, met een toelichting in de
configuratie:

- **Diagrammen (Mermaid).** Diagrammen worden als inline-SVG getekend, met
  tekst in transparante lagen. axe kan de achtergrond achter die tekst niet
  bepalen. De werkelijke verhouding is ongeveer 13:1 (donkerblauw op lichtblauw)
  voor stroom- en toestanddiagrammen en ongeveer 10:1 (wit op donkerblauw) voor
  de C4-diagrammen. De diagramkleuren komen uit het NLDD-palet.
- **Codevoorbeelden.** Codeblokken dragen een licht- én een donker thema in
  hetzelfde element; de toets verwart de twee kleurensets. De werkelijke tekst
  haalt 18,9:1 in de lichte modus en 17,6:1 in de donkere.
- **De hero op de landingspagina.** De titeltekst staat op een
  donkerblauwe verloopachtergrond. axe kan contrast over een verloop niet
  beoordelen; beide uiteinden van het verloop zijn donker genoeg (wit op de
  lichtste stop ongeveer 9:1).

De toegankelijke naam van elk diagram (`role="img"` plus een `aria-label`) wordt
in dezelfde gate apart tegen de bouwoutput gecontroleerd, los van de uitgesloten
contrastmeting.

## Wat nog niet gedekt is

- Er is **geen onafhankelijke audit** geweest. De claims hierboven berusten op
  de eigen toets en controle van het team.
- De geautomatiseerde toets dekt **WCAG 2.1**; de negen 2.2-criteria zijn
  handmatig beoordeeld, niet door een tool.
- De interface bevat web-componenten van het NLDD-designsysteem die hun
  inhoud in een schaduw-DOM tekenen. Hun eigen focusindicatie en interne
  toegankelijkheid vallen buiten de site-eigen controle en zijn afhankelijk van
  het designsysteem.

## Probleem melden

Kom je een toegankelijkheidsprobleem tegen, of lukt iets niet? Mail naar
[regelrecht@minbzk.nl](mailto:regelrecht@minbzk.nl). Beschrijf wat er misging en
op welke pagina, dan pakken we het op.

## Opgesteld

Deze conceptverklaring is opgesteld op 20 mei 2026, op basis van de toets en
controle van datzelfde moment. RegelRecht is een verkenning en wordt nog
ontwikkeld; de verklaring wordt bijgewerkt als de site verandert of na een
formele audit.
