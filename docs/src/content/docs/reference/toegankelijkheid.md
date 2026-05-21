---
title: Toegankelijkheid
description: Toegankelijkheidsverklaring voor RegelRecht (WCAG 2.2 AA, concept).
lang: nl
---

# Toegankelijkheidsverklaring

Deze verklaring is in het Nederlands en geldt als de bindende versie. Er is ook
een [Engelse vertaling](/reference/accessibility) voor lezers van de
Engelstalige documentatie; bij verschil tussen beide is deze Nederlandse tekst
leidend.

De verklaring beschrijft in hoeverre RegelRecht voldoet aan de
toegankelijkheidseisen voor overheidswebsites. De norm is WCAG 2.2 niveau AA,
zoals die sinds oktober 2024 via EN 301 549 geldt en in Nederland verplicht is
onder het Tijdelijk besluit digitale toegankelijkheid overheid. Ze geldt voor de
site die op `regelrecht.rijks.app` en op `docs.regelrecht.rijks.app` wordt
aangeboden, dus voor de landingspagina, het aanmeldformulier en de documentatie.

**Dit is een concept.** De status hieronder berust op een geautomatiseerde toets
in de bouwstraat en een handmatige controle door het team. Een onderzoek door een
onafhankelijke partij heeft niet plaatsgevonden, en de verklaring staat nog niet
in het register van DigiToegankelijk. Tot dat allebei rond is, is dit geen
rechtsgeldige verklaring en draagt RegelRecht geen toegankelijkheidslabel.

## Nalevingsstatus

Gedeeltelijk. De geautomatiseerde toets vindt geen fouten op de criteria die zij
meet, en de handmatige controle dekt de criteria die geen tool betrouwbaar test.
Een paar bekende beperkingen blijven over; die staan hieronder. Zonder externe
audit claimt RegelRecht geen volledige naleving.

## Taal van de site

De documentatie is grotendeels Engels, de landingspagina en het aanmeldformulier
zijn Nederlands. Elke pagina draagt een `lang`-attribuut dat overeenkomt met de
taal van de inhoud (`en` voor de docs, `nl` voor deze verklaring en de landing),
zodat een schermlezer de juiste uitspraak kiest. Waar binnen een pagina een
losse term in de andere taal staat, krijgt dat fragment een eigen `lang`. Dit
dekt WCAG 3.1.1 (taal van de pagina) en 3.1.2 (taal van onderdelen).

## Hoe dit getoetst is

**Geautomatiseerd, bij elke wijziging.** Een toegankelijkheidstoets draait in CI
op elke wijziging die de documentatie raakt. De toets bouwt de site en draait
[pa11y-ci](https://github.com/pa11y/pa11y-ci) met twee onafhankelijke engines,
HTML_CodeSniffer en axe-core 4.11, tegen elke gegenereerde pagina (op dit moment
61). De URL-lijst komt uit de build, zodat een nieuwe pagina automatisch
meegetoetst wordt en niet buiten de controle valt. De toets is lokaal te draaien
met `just docs-a11y`.

Die twee engines dekken een groot deel van WCAG 2.2 dat machinaal te meten is:
contrast, formulierlabels, koppenstructuur, landmarks, alt-teksten, en een deel
van de in 2.2 toegevoegde criteria. Wat een tool niet betrouwbaar ziet, is met de
hand gecontroleerd.

**Handmatig, voor de rest van 2.2.** Het team liep de negen succescriteria na die
WCAG 2.2 toevoegt: focus niet afgedekt (minimum en uitgebreid), focusverschijning,
sleepbewegingen, klikdoelgrootte, consistente hulp, herhaalde invoer, en
toegankelijke authenticatie (minimum en uitgebreid). Daarnaast is met de hand
getest:

- de werking met alleen een toetsenbord, inclusief de sla-over-link en de
  focusvolgorde;
- de donkere modus, op contrast en leesbaarheid;
- weergave bij 200% zoom en bij 400% herschaling.

## Bekende beperkingen

Een aantal elementen wordt door de geautomatiseerde contrasttoets gemeld, terwijl
het contrast feitelijk ruim voldoet. De engines kunnen de verhouding bij deze
elementen niet betrouwbaar bepalen. Het team heeft de werkelijke verhouding in de
browser gemeten en deze elementen daarom uitgesloten van de geautomatiseerde
meting, met een toelichting in de configuratie:

- **Diagrammen (Mermaid).** Diagrammen worden als inline-SVG getekend, met tekst
  in transparante lagen waar axe de achterliggende kleur niet van kan aflezen. De
  gemeten verhouding is 14,4:1 voor de tekst in stroom- en toestanddiagrammen
  (donkerblauw op lichtblauw) en ruim boven de eis voor de C4-diagrammen (wit op
  donkerblauw). De diagramkleuren komen uit het NLDD-palet.
- **Codevoorbeelden.** Codeblokken dragen een licht- en een donker thema in
  hetzelfde element; de toets verwart de twee kleurensets en meet een mengsel dat
  niet op het scherm verschijnt. De werkelijke tekst haalt in beide modi ruim de
  4,5:1 die AA vraagt.
- **De hero op de landingspagina.** De titeltekst staat in wit op een donkerblauwe
  verloopachtergrond. axe kan een verloop niet beoordelen. De zwakste plek van het
  verloop, de lichtste stop, geeft wit-op-donkerblauw een verhouding van 11,4:1;
  naar de donkere kant loopt dat op tot 15,5:1.

De toegankelijke naam van elk diagram (`role="img"` met een `aria-label`) wordt in
dezelfde gate apart tegen de bouwoutput gecontroleerd, los van de uitgesloten
contrastmeting.

## Wat nog niet gedekt is

- Er is **geen onafhankelijke audit** geweest. Alles hierboven berust op de eigen
  toets en controle van het team.
- De interface gebruikt web-componenten uit het NLDD-designsysteem die hun inhoud
  in een schaduw-DOM tekenen. Hun eigen focusindicatie en interne toegankelijkheid
  vallen buiten de site-eigen controle en hangen af van het designsysteem.

## Probleem melden

Kom je een toegankelijkheidsprobleem tegen, of lukt iets niet? Mail naar
[regelrecht@minbzk.nl](mailto:regelrecht@minbzk.nl). Beschrijf wat er misging en
op welke pagina; dan pakken we het op.

## Opgesteld

Deze conceptverklaring is opgesteld op 21 mei 2026, op basis van de toets en
controle van dat moment. RegelRecht is een verkenning en wordt nog ontwikkeld; de
verklaring wordt bijgewerkt als de site verandert of na een formele audit.
