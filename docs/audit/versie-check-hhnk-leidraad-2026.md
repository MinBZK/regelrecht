# Versie-check: HHNK-leidraad invordering waterschapsbelastingen 2023 → 2026

**Bestaande YAML**: `corpus/regulation/nl/waterschaps_verordening/hhnk/
  leidraad_invordering_waterschapsbelastingen/2023-01-01.yaml` (CVDR691525, valid_from 2023-09-08)

**Nieuwe officiële versie**: CVDR756485, *Leidraad invordering waterschapsbelastingen HHNK 2026*,
valid_from 2026-02-07. Ingetrokken voorganger: CVDR-id 22.1032975 (=CVDR691525).

**Aanpak**: parallel-versie — nieuwe YAML `2026-02-07.yaml` naast de bestaande
`2023-01-01.yaml`. Geen vervanging; de engine dispatcht op `calculation_date`.

**Checker**: — (plaatsnaam invullen)
**Datum check**: — (datum invullen)

---

## Check-werkwijze

1. CVDR756485 volledig gedownload via `curl` (625KB HTML, 6505 regels).
2. Artikelen 25, 26, 48, 80 geëxtraheerd (de artikelen waar HHNK-leidraad
   machine_readable heeft).
3. Normalised diff tegen equivalente tekst in de 2023-YAML.
4. Per verschil beoordeeld: tekstueel (alleen `text:` veld bijwerken),
   materieel-bedragwijziging (bedrag-definition bijwerken), of logica-wijziging
   (machine_readable aanpassen + BDD).

---

## Bevindingen

### Art 26 — Kwijtschelding

**Koptekst**:
- 2023: `**Kwijtschelding van belastingen**` (markdown-bold)
- 2026: `Artikel 26 – Kwijtschelding van belastingen` (expliciet artikel-nummer in titel)

**26.1.1 Ambtshalve terugbetaling** — slotzin gewijzigd:
- 2023: *"… betaalt hij de belastingschuldige **de bedragen terug die tot maximaal drie maanden voorafgaand aan de datum van indiening van het verzoek zijn verricht**."*
- 2026: *"… betaalt hij de belastingschuldige **het bedrag terug waarvoor kwijtschelding is verleend**."*
- **Materiële wijziging**: in 2023 was het terugbetaalde bedrag beperkt tot max 3 maanden voor indiening; in 2026 wordt het hele kwijtgescholden bedrag terugbetaald.
- **Impact op machine_readable**: geen — 26.1.1 staat als `untranslatables: accepted: true` (tijdsvenster-check is niet generiek codeerbaar). De `legal_text_excerpt` in de untranslatable verdient wel bijwerking naar de 2026-formulering.

**26.1.2 t/m 26.1.8**: layout-wijziging (bullet-punten inline i.p.v. bullet op aparte regel). Inhoudelijk identiek.

**26.1.9 Uitsluitingsgronden**: tekst identiek. De 7 weigergronden die onze `uitgesloten_van_kwijtschelding` voeden zijn ongewijzigd.

**26.2.2 Inboedel**: tekst-marker *"[Vervallen per 01-01-2023]"* blijft aanwezig in beide versies. Inhoud uit Leidraad 2008 26.2.2.

**26.2.3 Motorvoertuigen**: auto-drempel **€3.350 ongewijzigd**. Onze `overrides`-declaratie in Leidraad 2008 26.2.3 blijft correct voor 2026.

**26.2.12 Studiefinanciering en kwijtschelding**: bedrag-wijzigingen:
- Boeken/leermiddelen forfait algemeen: €67 → **€80**
- Boeken/leermiddelen forfait studiefinanciering: €60 → **€70**
- **Impact op machine_readable**: geen — 26.2.12 heeft geen machine_readable.
  Tekst in `text:` moet wel bijgewerkt.

**26.2.19 Normpremie zorgverzekering** — substantiële bedrag-wijzigingen:
- Alleenstaande/alleenstaande ouder: €3/mnd → **€47/mnd**
- Echtgenoten: €50/mnd → **€106/mnd**
- **Impact op machine_readable**: geen — 26.2.19 heeft geen machine_readable.
  Wel signaal: normpremies zijn drastisch verhoogd (factor 2-16×) wat in de
  Zvw-tarieven-tracking belangrijk is om door te trekken. Zvw-YAML zelf moet
  nog een 2026-versie krijgen.

**26.2.20 Onderhoud gezinsleden buitenland**: tekst identiek; blijft `untranslatable`.

**Overige subsecties 26.3.x** (ondernemers): grotendeels identiek; eventuele
wijzigingen beïnvloeden geen machine_readable.

### Art 25 — Uitstel van betaling (alleen 25.5.3 is machine_readable)

**25.5.3 Kort uitstel particulier** — drempel **€500 ongewijzigd**. Andere
voorwaarden (dwangbevel, auto-incasso, goed-betalingsgedrag, ander uitstel)
zijn tekstueel identiek. Machine_readable blijft correct.

### Art 48 — Beperking aansprakelijkheid erfgenamen

**48.2** — drempel **€23 ongewijzigd**. Voorwaarden (hoofdsom-uitzondering +
"niet meer tegen gezamenlijke erfgenamen") identiek. Machine_readable
blijft correct.

### Art 80 — Incassoreglement

**Minimumbedrag €10/termijn ongewijzigd**. Drempel **€100 aanslag** ongewijzigd.
Machine_readable blijft correct.

---

## Conclusies

1. **Alle machine_readable-logica blijft geldig** voor 2026. Geen formule-
   aanpassingen nodig.
2. **Tekst-updates zijn klein**: één materiële slotzin-wijziging (26.1.1) en
   drie bedrag-wijzigingen (26.2.12 × 2, 26.2.19 × 2) — allemaal in artikelen
   zonder machine_readable.
3. **Metadata-update**: titel, `publication_date`, `valid_from`, `url`, alle
   artikel-URL's van CVDR691525 → CVDR756485.
4. **Normpremie zorgverzekering** (26.2.19): wijziging signaal voor
   Zorgverzekeringswet YAML — 2026-versie zou mee-evolueren.

## Actie

- Nieuwe YAML `2026-02-07.yaml` als **kopie van 2023-01-01.yaml**, met:
  - Metadata bijgewerkt (CVDR756485, 2026-datums, titel/name)
  - Artikel-URL's CVDR691525 → CVDR756485
  - Art 26 `text:` vervangen door 2026-tekst (behoudt alle wijzigingen
    hierboven genoemd)
  - Art 25, 48, 80 `text:` vervangen door 2026-tekst (geen materiële wijziging,
    wel kleine layout-verbetering zoals bullet-inlining)
  - Machine_readable: identiek overgenomen uit 2023-versie
  - `untranslatables[26.1.1].legal_text_excerpt` bijwerken naar 2026-formulering
- Oude YAML `2023-01-01.yaml` ongewijzigd laten — pre-2026 scenarios blijven
  valid.

## Review-checkboxes

- ☐ Tekst-diff volledig doorlopen (660-regels diff `/tmp/art26-diff.txt`)
- ☐ Bedrag-wijzigingen gecontroleerd tegen officiële publicatie
- ☐ Geen materiële wijziging in 26.1.9 uitsluitingsgronden (7 gronden identiek)
- ☐ Auto-drempel €3.350 in 26.2.3 ongewijzigd
- ☐ Normpremie-wijziging 26.2.19 doortrekken naar Zvw 2026-YAML (separate check)
- ☐ Slotzin-wijziging 26.1.1: `untranslatable.legal_text_excerpt` bijwerken
