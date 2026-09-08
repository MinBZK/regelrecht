# SZW — ruwe feedback, onbewerkt

Dit bestand bewaart de feedback van de SZW-jurist **letterlijk**, zoals
ontvangen. Geen interpretatie, geen herschikking, geen aangevulde
artikelverwijzingen. De verwerking staat elders:

- [actieregister.md](actieregister.md) — wat we ermee gedaan hebben
- [2026-07-23-juristvalidatie-notities.md](2026-07-23-juristvalidatie-notities.md) — ronde 1, gestructureerd
- [2026-09-02-juristfeedback-doorloop.md](2026-09-02-juristfeedback-doorloop.md) — ronde 2, gestructureerd

**Waarom apart.** In de gestructureerde notities is de feedback al vertaald
naar onze artikelnummers en onze modelleerbegrippen. Bij ronde 2 bleek die
vertaalslag ergens te ver te gaan: de jurist noemde art. 8d, maar dat artikel
gaat in de 2026-07-01-versie niet over voorzieningen — de verordeningsplicht
staat in art. 8a. Zulke correcties zijn alleen navolgbaar als de oorspronkelijke
formulering ergens onaangeraakt bewaard blijft. Bij twijfel over wat er precies
gevraagd is, wint dit bestand van de gestructureerde notitie.

---

## Ronde 2 — 2 september 2026

Feedback op het doorloop-artifact "Koen en Sadee door het stelsel".
Twee punten, letterlijk overgenomen:

> - Artikel 35 WIA sluit ook de Participatiewet uit (lid 4b, echt een
>   superomslachtige formulering). Scenario Koen heeft op grond daarvan dus
>   geen recht op voorziening/jobcoach/werkplekaanpassing. Maar dat recht is
>   er wel gebaseerd op P-wet art. 7-8d / 10e (ingewikkelde hier is: gemeente
>   heeft opdracht, maar precieze regels moet gemeente vastleggen in
>   verordening; alleen recht op jobcoach voor doelgroep LKS is hard
>   vastgelegd in P-wet art. 10da).
> - Proefplaatsing zit ook in meerdere wetten: P-wet artikel 8a lid 2d;
>   Wajong art. 2:24; WIA art. 37). Dus niet alleen in de WW waar nu op wordt
>   getoetst. Waarbij de kaders soms ook nog eens kunnen verschillen
>   (proefplaatsing Wajong bijvoorbeeld max 6 maanden, terwijl in P-wet
>   afgebakend tot 2-4 maanden…). Makkelijker konden we het kennelijk niet
>   maken :/

### Afwijkingen tussen deze tekst en wat we gebouwd hebben

| Ruwe tekst | Wat we deden | Waarom |
|---|---|---|
| "P-wet art. 7-8d" | art. 7 **niet** gemodelleerd; art. 8a en 10 wel | Art. 7 is een taakopdracht aan het college, geen aanspraak van de burger. De doelgroepomschrijving van 7 lid 1 onderdeel a is verwerkt in art. 10 |
| "art. 8d" | art. **8a** gemodelleerd | Art. 8d gaat in de 2026-07-01-versie niet over voorzieningen; de bedoelde verordeningsplicht staat in 8a lid 1 |
| "10e" | als **uitsluitend `open_terms`**, geen execution | Vier AMvB-grondslagen, alle kan-bepalingen. Zolang de AMvB er niet is verandert 10e niets aan art. 10 lid 1 |
| "P-wet afgebakend tot 2-4 maanden" | `max_duur` 2, `max_duur_verlenging` 4, `max_totale_duur` 6 | De wet zegt twee maanden, verlengbaar met ten hoogste vier. "2-4" leest als een bandbreedte; het is een basis plus een verlenging |

---

## Ronde 1 — 23 juli 2026

**Geen letterlijke tekst bewaard.** Van deze sessie bestaan alleen
aantekeningen die ter plekke al gestructureerd zijn opgeschreven; er is geen
onbewerkte bron om hier neer te zetten. Eén formulering is als citaat in de
notities overgeleverd, over onze eigen open vraag bij Wajong art. 2:20 lid 2:

> lid 2 nietigheid modelleren wij als harde constante — akkoord?

→ akkoord bevonden.

De rest van ronde 1 staat in
[2026-07-23-juristvalidatie-notities.md](2026-07-23-juristvalidatie-notities.md),
met de kanttekening die daar zelf al staat: *"Verwijzingen naar wetsartikelen
zijn toegevoegd op basis van de gemodelleerde corpus; de inhoudelijke punten
zijn van de jurist."* Waar in die notitie een artikelnummer staat, is dat dus
mogelijk van ons en niet van de jurist.

---

## Werkafspraak voor volgende rondes

Feedback komt hier **eerst** binnen, letterlijk en met datum, vóór er iets
mee gebeurt. Pas daarna de gestructureerde notitie en het actieregister. Zo
blijft achteraf te zien wat er gevraagd is en wat onze interpretatie was.
