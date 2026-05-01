# Audit — URI 1990 art 14 (netto-besteedbaar inkomen)

**Wet**: Uitvoeringsregeling Invorderingswet 1990
**`$id`**: `uitvoeringsregeling_invorderingswet_1990`
**Wet-URL**: https://wetten.overheid.nl/BWBR0004766/2026-01-01#Artikel14
**YAML-bestand**: `corpus/regulation/nl/ministeriele_regeling/uitvoeringsregeling_invorderingswet_1990/2026-01-01.yaml`
**Laatste review**: —
**Reviewer(s)**: —

---

## Werkwijze

Art 14 definieert *wat telt als netto-besteedbaar inkomen* voor art 13.
Vijf inkomenscategorieën (lid 1 onder a-c + lid 2 + lid 4), verminderd
met de uitgaven uit art 15. Heeft 2 `untranslatables` voor de kunstenaars-
en AOW-netto-terugval. In de keten werkt art 14 documentair: de caller
leveret in de praktijk een kant-en-klaar `netto_besteedbaar_inkomen_maand`
als parameter aan (zie HHNK-leidraad art 26).

---

## Output 1 — `totaal_inkomen_maand`

**Wettekst-excerpt** — art 14 lid 1 onder a, b, c + lid 2 + lid 4:

> "1. Onder het netto-besteedbare inkomen, bedoeld in artikel 13, wordt
> verstaan het met de in artikel 15 vermelde uitgaven verminderde
> gezamenlijke bedrag van:
> a. de aan inhouding van loonbelasting/premie voor de volksverzekeringen
>    onderworpen inkomsten verminderd met de wettelijke inhoudingen en
>    de ingehouden pensioenpremies en premies ziektekostenverzekering;
> b. uitkeringen voor levensonderhoud ingevolge de artikelen 157, 158 of
>    404 van Boek 1 van het Burgerlijk Wetboek;
> c. overige inkomsten met uitzondering van […lange lijst uitzonderingen…]
> 2. Tot de inkomsten, bedoeld in het eerste lid, onderdeel c, wordt ook
>    gerekend de voorlopige teruggaaf […met uitzondering van kinderkorting…]
> 4. Voor […] kunstenaars worden […] ook gerekend de inkomsten uit de
>    beroepsuitoefening."

| | |
|---|---|
| **Formule** | `totaal_inkomen = loonbelastingplichtig + alimentatie + overige + voorlopige_teruggaaf + kunstenaars` |
| **YAML-locatie** | `articles[14].machine_readable.actions[0]` |

**Review**:

- ☐ Vijf optellingen dekken lid 1 a/b/c + lid 2 + lid 4.
- ☐ Uitzonderingen uit lid 1 onder c (kinderbijslag, PGB, bepaalde ABW-subsidies) — niet in formule; caller moet die uit `overige_inkomsten_maand` filteren.
- ☐ Kinderkorting uit voorlopige teruggaaf (lid 3) — niet in formule; caller moet die uit `voorlopige_teruggaaf_maand` filteren.

---

## Output 2 — `netto_besteedbaar_inkomen_maand`

**Wettekst-excerpt** — art 14 lid 1 inleiding ("verminderd met de in artikel 15 vermelde uitgaven"):

| | |
|---|---|
| **Formule** | `netto_bi = max(0, totaal_inkomen − uitgaven_totaal)` |
| **Bron uitgaven_totaal** | source → art 15 (binnen zelfde wet) |
| **YAML-locatie** | `articles[14].machine_readable.actions[1]` |

**Review**:

- ☐ Subtract correct — netto = bruto − uitgaven.
- ☐ MAX met 0 correct — negatief inkomen onzinnig.
- ☐ In de HHNK-ketenproductie wordt `netto_besteedbaar_inkomen_maand` als *parameter* aangeleverd, niet via art 14 output. Dit artikel is daarmee documentair — klopt dat?

---

## Niet-getranslateerd (`untranslatables`, accepted)

| Construct | Wettekst-kern | Reden |
|---|---|---|
| **Lid 5: inkomen gelijkstellen aan bijstandsnorm voor kunstenaars zonder uitkering** | "Voor de belastingschuldige […] die […] geen uitkering heeft genoten […] worden de inkomsten gesteld op de op hem van toepassing zijnde bijstandsnorm" | Vereist case-analyse (wie valt onder art 10 lid 2, welke norm) — niet generiek in formule te vangen. |
| **AOW-netto alternatief (via Regeling medeoverheden art 3)** | Verordeningsoptie: pensioengerechtigden gebruiken netto-ouderdomspensioen i.p.v. bijstandsnorm | URI art 16 heeft alternatieve definities; keuze welk pad ligt bij verordening. |

**Review**:

- ☐ Beide untranslatables bewust als "accepted: true" gemarkeerd — geen verdoken halve implementatie.

---

## Open punten voor workshop

1. **Caller-filtering**: de formule vertrouwt erop dat caller *al* de uitzonderingen (kinderbijslag, PGB, kinderkorting) heeft gefilterd voor de overige_inkomsten- en voorlopige_teruggaaf-parameters. Is dat expliciet genoeg gedocumenteerd?
2. **Art 14 wordt niet in de HHNK-keten gebruikt** — caller levert `netto_besteedbaar_inkomen_maand` direct aan URI art 13. Blijft dit artikel relevant voor validatie?
3. **Wet IB 2001 art 14** (voorlopige teruggaaf) — tekstueel genoemd maar niet via `source:` gekoppeld. Refactor-kandidaat.
