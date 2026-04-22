# Audit — URI 1990 art 15 (uitgaven)

**Wet**: Uitvoeringsregeling Invorderingswet 1990
**`$id`**: `uitvoeringsregeling_invorderingswet_1990`
**Wet-URL**: https://wetten.overheid.nl/BWBR0004766/2026-01-01#Artikel15
**YAML-bestand**: `corpus/regulation/nl/ministeriele_regeling/uitvoeringsregeling_invorderingswet_1990/2026-01-01.yaml`
**Laatste review**: —
**Reviewer(s)**: —

---

## Werkwijze

Art 15 somt zes categorieën uitgaven op (a t/m f) die bij het
netto-besteedbaar inkomen mogen worden afgetrokken. De netto-woonlasten
hebben een specifieke cap: alleen het deel boven een drempel en onder
een maximum uit de (oude) Huursubsidiewet telt mee. De andere categorieën
zijn simpelweg opgesomd.

---

## Output 1 — `netto_woonlasten_maand`

**Wettekst-excerpt** — art 15 onder b (uittreksel):

> "b. het bedrag van de voor rekening van de belastingschuldige komende
> netto-woonlasten tot maximaal het bedrag, genoemd in artikel 13, eerste
> lid, onderdeel a, van de Huursubsidiewet, voorzover dit meer is dan het
> bedrag, genoemd in artikel 17, tweede lid, van die wet. Onder
> netto-woonlasten wordt verstaan: de op de belastingschuldige drukkende
> huurprijs […] of hypotheekrente en erfpachtcanon […] verminderd met de
> ontvangen huursubsidie en bijzondere bijdrage […]"

| | |
|---|---|
| **Formule** | `netto_wl = max(0, bruto_woonlasten − huursubsidie)` |
| **YAML-locatie** | `articles[15].machine_readable.actions[0]` |

**Review**:

- ☐ Subtract correct — netto = bruto minus ontvangen subsidies.
- ☐ "Bijzondere bijdrage" (art 26b Huursubsidiewet) en "woonkostentoeslag" ook onder `huursubsidie_ontvangen_maand`?
- ☐ MAX met 0 correct.

---

## Output 2 — `woonlasten_in_aanmerking_maand`

**Wettekst-excerpt** — zelfde passage art 15 onder b, volledige cap-regel:

| | |
|---|---|
| **Formule** | `wl_in_aanmerking = max(0, min(netto_wl, woonlasten_maximum) − woonlasten_drempel)` |
| **Maximum** | €879.66/maand (art 13 lid 1 a Huursubsidiewet, feitelijk Wet op de huurtoeslag) |
| **Drempel** | €258.15/maand (art 17 lid 2 Huursubsidiewet) |
| **YAML-locatie** | `articles[15].machine_readable.actions[1]` |

**Review**:

- ☐ Maximum = huurtoeslag-max cap: alleen het deel onder het max telt, niet het totaal.
- ☐ Drempel: alleen het deel *boven* de drempel telt als uitgave.
- ☐ Volgorde MIN dan SUBTRACT correct: eerst cap, dan drempel aftrekken.
- ☐ Waarden €879,66 + €258,15 zijn 2026-waarden; SHORTCUT-documentatie in YAML-definitions zou helpen (analoog aan art 16 bijstandsnormen).

**Open**:

- Huursubsidiewet is sinds 2020 vervangen door Wet op de huurtoeslag. URI-tekst verwijst nog naar oude wet. Getallen zijn wel actueel. Moet verwijzing naar WHT in comment?

---

## Output 3 — `uitgaven_totaal_maand`

**Wettekst-excerpt** — art 15 onderdelen a t/m f gezamenlijk:

> "a. betalingen op belastingschulden, met uitzondering van die genoemd in
>    artikel 8, tweede lid;
> b. het bedrag van de voor rekening van de belastingschuldige komende
>    netto-woonlasten [met cap];
> c. de niet door de werkgever ingehouden premies ziektekostenverzekering
>    en de nominale premies ingevolge de Ziekenfondswet en de Algemene Wet
>    Bijzondere Ziektekosten;
> d. betaalde uitkeringen voor levensonderhoud ingevolge de artikelen 157,
>    158 of 404 van Boek 1 van het Burgerlijk Wetboek;
> e. aflossingen op leningen voor zover die zijn aangewend voor de betaling
>    van belastingschulden;
> f. de met het houden van kostgangers verbonden kosten […gecapt aan art 32/34
>    Uitvoeringsregeling loonbelasting…]"

| | |
|---|---|
| **Formule** | `uitgaven = betalingen_belastingschulden + wl_in_aanmerking + premies_ziektekosten + betaalde_alimentatie + aflossingen_belastingschulden + kostgangerskosten` |
| **YAML-locatie** | `articles[15].machine_readable.actions[2]` |

**Review**:

- ☐ Zes categorieën gedekt (a t/m f).
- ☐ Onder a: exclusief de aanslag waarvoor kwijtschelding wordt aangevraagd (art 8 lid 2) — caller-verantwoordelijkheid om dit uit `betalingen_belastingschulden_maand` te filteren?
- ☐ Onder f kostgangerskosten: caller moet zelf cap toepassen (art 32/34 Uitvoeringsregeling loonbelasting 2001 + niet-hoger-dan-inkomsten). In formule onbeknot.
- ☐ Geen zevende categorie of vergeten onderdeel.

---

## Niet-getranslateerd

Geen `untranslatables` in dit artikel; wel impliciete caller-verantwoordelijkheden (zie review-punten).

---

## Open punten voor workshop

1. **Caps onder f (kostgangerskosten)** — verwijzing naar art 32/34 Uitvoeringsregeling loonbelasting 2001 is complex (dagtarief, maaltijd-tarief). Caller moet dit zelf uitrekenen. Is dat aanvaardbaar of moeten we een apart `source:` hebben naar Uitvoeringsregeling loonbelasting?
2. **Art 8 lid 2 exclusie op onderdeel a en e** — expliciet uit formule weglaten via caller-filter of apart modelleren als input-validatie?
3. **Nominale premies ZFW + AWBZ** (onder c) — ZFW is afgeschaft per 2006 (opgegaan in Zvw), AWBZ per 2015 (opgegaan in Wlz). Tekst is achterhaald; praktische invulling is premie Zvw + eventueel Wlz-bijdrage?
