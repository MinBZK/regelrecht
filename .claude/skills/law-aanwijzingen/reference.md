# Reference — kernset Aanwijzingen voor de regelgeving

Gecondenseerde, machine-bruikbare kernset van de Aanwijzingen voor de regelgeving
(KCBR), toegespitst op de kwaliteit van **wetteksten op artikelniveau**. Dit is een
werkbare startset, geen volledige weergave van alle aanwijzingen.

> **Nummering:** de nummers hieronder volgen de hoofdstukindeling van de Aanwijzingen
> (hfst. 3 vormgeving, hfst. 4 algemene bestanddelen, hfst. 5 bijzondere bestanddelen).
> Behandel de exacte nummers als indicatief: noem het nummer als je zeker bent, gebruik
> anders `aanwijzing_nr: null` en beschrijf het kwaliteitsprobleem inhoudelijk. De volledige,
> actuele tekst staat op https://www.kcbr.nl/ontwikkelen-beleid-en-regelgeving/aanwijzingen-voor-de-regelgeving

Per item: **kern** + **toetsvraag** (stel die op de artikeltekst; "ja" → mogelijke bevinding).

## 1. Definities en begripsbepalingen

- **3.x Begripsbepalingen vooraan.** Kern: definities horen geconcentreerd in een
  begripsbepalingenartikel (meestal artikel 1), niet verspreid.
  Toetsvraag: introduceert dit artikel een eigen definitie ("wordt verstaan onder…",
  "in dit artikel wordt … aangeduid als…") die in het begripsbepalingenartikel thuishoort?
- **3.x Definieer alleen wat afwijkt.** Kern: definieer geen term die in normaal
  spraakgebruik al duidelijk is; definieer juist wel een term met een afwijkende betekenis.
  Toetsvraag: wordt een alledaagse term overbodig gedefinieerd, óf wordt een vaktechnische
  term zonder definitie gebruikt?
- **3.x Gebruik gedefinieerde termen consequent.** Kern: gebruik een gedefinieerde term
  overal in dezelfde betekenis; introduceer geen synoniemen.
  Toetsvraag: wordt voor hetzelfde begrip nu eens term A, dan term B gebruikt?

## 2. Terminologie en consistentie

- **3.56 (e.o.) Consistent taalgebruik.** Kern: gebruik in de hele regeling dezelfde term
  voor hetzelfde begrip en vermijd wisselende formuleringen.
  Toetsvraag: duiken binnen dit artikel of t.o.v. eerdere artikelen verschillende termen op
  voor wat hetzelfde lijkt (bv. "aanvrager" vs "belanghebbende" vs "verzoeker")?
- **3.x Eenduidige werkwoordsvormen.** Kern: druk een verplichting/bevoegdheid eenduidig uit.
  In NL-wetgeving: gebonden norm = tegenwoordige tijd ("De minister stelt vast"); bevoegdheid =
  "kan". Vermijd "zal", "dient te", "moet" door elkaar.
  Toetsvraag: worden normstellende werkwoordsvormen inconsistent of vaag gebruikt?

## 3. Normstellende vs. beschrijvende/toelichtende tekst

- **3.x Geen toelichtende of motiverende tekst in de regeling.** Kern: de regeling bevat
  normen; toelichting hoort in de memorie van toelichting, niet in het artikel.
  Toetsvraag: bevat de tekst uitleg, motivering of voorbeelden ("dit betekent dat…",
  "met als doel…", "bijvoorbeeld") die in een MvT thuishoren?
- **3.x Geen overbodige bepalingen.** Kern: neem niets op wat al uit hogere regeling of
  algemeen recht volgt; herhaal geen wettelijke regel die elders al geldt.
  Toetsvraag: herhaalt dit artikel iets wat al elders dwingend geregeld is?
- **3.x Normen, geen intenties.** Kern: een artikel formuleert een rechtsgevolg, geen
  beleidswens.
  Toetsvraag: is de bepaling een streven/intentie ("streeft naar", "bevordert") zonder
  concreet rechtsgevolg, waar een norm bedoeld is?

## 4. Verwijzingen

- **3.x Verwijs zo precies en stabiel mogelijk.** Kern: verwijs naar het specifieke
  artikel/lid; vermijd vage of ketenverwijzingen ("het bepaalde in de vorige artikelen").
  Toetsvraag: bevat de tekst een vage verwijzing, of een verwijzing die door wijziging snel
  onjuist wordt?
- **3.x Vermijd verwijzingen naar verwijzingen.** Kern: geen verwijzing naar een bepaling
  die zelf alleen maar doorverwijst (cirkel/keten).
  Toetsvraag: leidt de verwijzing de lezer via een omweg, in plaats van rechtstreeks naar de norm?

## 5. Delegatie

- **2.x / 5.x Delegatiegrondslag duidelijk en begrensd.** Kern: delegatie ("bij of
  krachtens algemene maatregel van bestuur", "bij ministeriële regeling") moet onderwerp en
  grenzen aangeven; delegeer geen hoofdelementen naar een te laag niveau.
  Toetsvraag: delegeert dit artikel iets zonder de reikwijdte te begrenzen, of naar een lager
  niveau dan past bij het gewicht van de norm?
- **5.x Juiste delegatieterminologie.** Kern: "bij amvb" (gebonden), "bij of krachtens"
  (subdelegatie mogelijk), "bij ministeriële regeling" (alleen voor details/uitvoering).
  Toetsvraag: past de gekozen delegatieformulering bij wat gedelegeerd wordt?

## 6. Artikelopbouw en leesbaarheid

- **3.x Eén gedachte per artikel/lid.** Kern: splits samengestelde normen; gebruik leden en
  onderdelen voor opsommingen.
  Toetsvraag: propt dit artikel meerdere zelfstandige normen in één lange volzin die beter
  in leden/onderdelen uiteenvalt?
- **3.x Korte, enkelvoudige zinnen.** Kern: vermijd lange zinnen met veel ingebedde
  voorwaarden; gebruik een opsomming.
  Toetsvraag: is de zin zo lang/genest dat de norm moeilijk te volgen is?
- **3.x Actieve, concrete formulering.** Kern: benoem wie wat moet/mag; vermijd onnodig
  passief en nominalisaties.
  Toetsvraag: verbergt een passieve constructie wie de normadressaat is ("er wordt vastgesteld"
  zonder dat duidelijk is door wie)?
- **3.x Geen dubbele ontkenningen / vermijdbare ambiguïteit.** Kern: formuleer ondubbelzinnig.
  Toetsvraag: bevat de zin een dubbele ontkenning of een "en/of"-constructie die meerdere
  lezingen toelaat?

## Toepassing

- Geef per echte bevinding precies één finding-object (zie SKILL.md outputcontract).
- Citeer altijd de letterlijke passage (`exact_quote`) — nooit een parafrase.
- Bij twijfel of iets echt fout is: laat het weg. Liever drie rake suggesties dan twintig vage.
- Severity-richtlijn: `belangrijk` = juridisch risico (vage delegatie, overbodige bepaling die
  conflicteert); `suggestie` = duidelijke kwaliteitswinst (terminologie, opbouw);
  `info` = stilistisch detail.
