# Audit — URI 1990 art 16 (kostennorm)

**Wet**: Uitvoeringsregeling Invorderingswet 1990
**`$id`**: `uitvoeringsregeling_invorderingswet_1990`
**Wet-URL**: https://wetten.overheid.nl/BWBR0004766/2026-01-01#Artikel16
**YAML-bestand**: `corpus/regulation/nl/ministeriele_regeling/uitvoeringsregeling_invorderingswet_1990/2026-01-01.yaml`
**Laatste review**: —
**Reviewer(s)**: —

---

## Werkwijze

Art 16 definieert de *kosten van bestaan* waarmee het netto-besteedbaar
inkomen wordt verminderd (via art 13). Het is gebaseerd op de bijstandsnorm
(federaal, Participatiewet art 21-24) of, voor pensioengerechtigden, op het
netto-ouderdomspensioen (via Regeling medeoverheden art 3). De verordening
van een medeoverheid mag het kostennorm-*percentage* aanpassen tussen 90 en
100% (open_term `kostennorm_percentage`).

**LET OP** — dit artikel bevat 7 `definitions` die als **SHORTCUT** in de
YAML zijn gemarkeerd: normatief horen bijstandsnormen thuis in de
Participatiewet, en AOW-netto-uitkomsten in Regeling medeoverheden art 3.
URI hardcodet ze hier voor 2026-stabiliteit.

---

## Output — `kostennorm_bedrag`

**Wettekst-excerpt** — art 16 lid 1 en 2 (verkort):

> "1. De kosten van bestaan, bedoeld in artikel 13, eerste lid, bedragen
> voor belastingschuldigen die worden aangemerkt als:
> a. echtgenoten als bedoeld in artikel 3 van de Algemene bijstandswet: 90
>    percent van […] de bijstandsnorm, genoemd in artikel 30, eerste lid,
>    onderdeel c […] doch ten hoogste 90 percent van die bijstandsnorm
>    nadat deze is verminderd met het bedrag, genoemd in artikel 33, tweede
>    lid, van die wet;
> b. alleenstaanden: 90 percent […]
> c. alleenstaande ouders: 90 percent […]
> 2. Voor belastingschuldigen die de pensioengerechtigde leeftijd hebben
>    bereikt: 90 percent […]"

| | |
|---|---|
| **Formule** | `kostennorm = kostennorm_percentage × basis_bedrag` |
| **Open_term percentage** | default 0.9; voor HHNK gezet op 1.0 via verordening art 2 (`implements`) |
| **basis_bedrag formule** | `IF is_pensioengerechtigd THEN (huishoudtype=echtgenoten ? AOW_echtgenoten : AOW_alleenstaand) ELSE (huishoudtype=echtgenoten ? bijstand_echtgenoten : huishoudtype=alleenstaande_ouder ? bijstand_ao : bijstand_alleen)` |
| **YAML-locatie** | `articles[16].machine_readable.actions[0]` |

**Review**:

- ☐ Percentage-open_term delegatie naar verordening (HHNK = 1.0) klopt.
- ☐ IF-boom over `is_pensioengerechtigd` en `huishoudtype` klopt conform lid 1 (niet-AOW) en lid 2 (AOW).
- ☐ Alleenstaande-ouder norm valt in onze modellering samen met alleenstaande norm (omdat Pw-hervorming 2015 die afzonderlijke norm heeft afgeschaft).
- ☐ De uit wettekst lid 1 onder a expliciet genoemde *maxima* (bv. "ten hoogste 90% nadat verminderd met het bedrag genoemd in artikel 33 lid 2 van die wet") zijn NIET letterlijk gemodelleerd. We rekenen enkel `percentage × basis`. Is dat correct genoeg?

---

## SHORTCUTS — de zeven `definitions`

| Definition | 2026-waarde | Normatieve bron | Status |
|---|---|---|---|
| `bijstandsnorm_alleenstaand` | €1401,50 | Participatiewet art 21 onderdeel a | SHORTCUT |
| `bijstandsnorm_alleenstaande_ouder` | €1401,50 | Pw art 21 onderdeel a (sinds 2015 gelijk aan alleenstaand) | SHORTCUT |
| `bijstandsnorm_echtgenoten` | €2002,13 | Pw art 21 onderdeel b | SHORTCUT |
| `bijstandsnorm_aow_alleenstaand` | €1564,69 | Pw art 22 onderdeel a | SHORTCUT |
| `bijstandsnorm_aow_echtgenoten` | €2144,16 | Pw art 22 onderdeel b | SHORTCUT |
| `netto_ouderdomspensioen_alleenstaand` | €1558,15 | Regeling medeoverheden art 3 lid 3 (formule) | SHORTCUT (uitkomst hardcoded) |
| `netto_ouderdomspensioen_echtgenoten` | €2135,40 | Regeling medeoverheden art 3 lid 2 onder a (formule) | SHORTCUT (uitkomst hardcoded) |

**Review per SHORTCUT**:

- ☐ 2026-bedragen kloppen met externe SVB/Pw/CBS-publicatie.
- ☐ SHORTCUT-markering in de YAML-description is voor alle zeven aanwezig.
- ☐ Refactor-kandidaat: de bijstandsnormen zouden via `source:` naar Participatiewet art 21-22 kunnen. Nu niet gedaan omdat Pw-YAML op 2022-waarden staat — discrepantie zou BDD breken.
- ☐ Netto-ouderdomspensioen hardcodet de uitkomst; de formule leeft in Regeling medeoverheden art 3 (bruto − LB − premies VV − Zvw-bijdrage). Refactor-kandidaat: URI source-t naar Regeling medeoverheden art 3 met bruto-AOW als parameter.

---

## Open_term — `kostennorm_percentage`

**Wettekst-excerpt** — implicatie van 90%-formulering gecombineerd met Regeling medeoverheden art 2:

> Regeling medeoverheden art 2: "Bij verordening kan van de in artikel 16
> van de Uitvoeringsregeling genoemde percentages worden afgeweken, mits:
> a. de percentages ten minste 90 en ten hoogste 100 bedragen"

| | |
|---|---|
| **Declaratie** | `open_term` in URI art 16 machine_readable |
| **Default** | 0.9 (URI's 90%) |
| **Delegatie** | waterschap / gemeente / provincie (via WATERSCHAPS_VERORDENING / etc.) |
| **Invulling HHNK** | verordening art 2 `implements`: percentage = 1.0 |
| **Begrenzing** | Regeling medeoverheden art 2 zegt 0.9 ≤ % ≤ 1.0 |

**Review**:

- ☐ Default 0.9 klopt voor rijks-invordering (geen verordenings-afwijking).
- ☐ HHNK-invulling 1.0 via `implements` correct gekoppeld aan deze open_term.
- ☐ Constraint 90-100% is gemodelleerd in Regeling medeoverheden art 2 (`kostennorm_percentage_rechtsgeldig`), niet in URI zelf. Correct: URI vermeldt alleen het percentage, de *grens* komt van de overkoepelende MR.

---

## Open punten voor workshop

1. **SHORTCUT-status** — is het acceptabel om 2026-waarden hardcoded in URI te houden tot Pw-YAML bijgewerkt is, of moeten we de refactor nu al plannen?
2. **Maxima lid 1 onder a** — "ten hoogste 90% nadat verminderd met art 33 lid 2 van die wet" — niet in formule. Review vereist.
3. **Relatie met art 22a Pw** — Pw-hervorming kostendelersnorm werkt door op de basis-bijstandsnorm voor jongere en kostendelende belastingschuldigen. Huidige formule houdt geen rekening met kostendelers.
4. **AOW factor B** — Regeling medeoverheden art 3 lid 2 onderdeel b verwijst naar factor B uit Pw art 22a. Gedekt via is_pensioengerechtigd=true? Of aparte case?
