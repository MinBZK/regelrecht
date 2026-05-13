# CSRD — projectoverzicht

Branch: `feat/CSRD`. Modellering van de Corporate Sustainability
Reporting Directive (CSRD) in regelrecht.

Status: **kick-off-fase**. Nog geen YAMLs, nog geen BDD-scenarios.
Eerst scope-bepaling met jurist.

## Domein in één oogopslag

| Aspect | Waarde |
|--------|--------|
| Primaire richtlijn | Richtlijn 2013/34/EU (Accounting Directive) |
| Amenderend | Richtlijn 2022/2464/EU (CSRD) + Omnibus 2026 |
| ESRS-detail | Verordening (EU) 2023/2772 (delegated act) |
| NL-omzetting | Boek 2 BW titel 9 + Wet implementatie CSRD |
| Drempelwaarden (post-Omnibus) | >1000 werknemers + >€450M netto-jaaromzet |
| Ingangsdatum | Boekjaren op of na 1 januari 2027 |
| Uitvoering & toezicht | AFM (beursgenoteerd), KVK (deponering), externe accountant (assurance), RJ (interpretatie) |

## Bestanden in deze map

| Bestand | Doel |
|---------|------|
| [stelsel-overview.md](stelsel-overview.md) | **Startpunt** — stelsel-diagram met regulatory_layers + uitvoeringsorganisaties |
| [scope-bepaling.md](scope-bepaling.md) | Werkblad voor de scope-bepalings-sessie met jurist (wegkruis-tabel + artikel-graph) |
| [jurist-input-2026-05-13.md](jurist-input-2026-05-13.md) | Verbatim juristen-mail die het startpunt vormt |

## Eerste sprint: doelgroepbepaling

Doel: `valt_onder_csrd` als eerste output, op basis van Richtlijn
2013/34/EU art. 1.3 + 19 bis + 29 bis en Richtlijn 2022/2464/EU
art. 5.2.

### Voortgangs-status

- [x] Fase 0 — Domein aankleden (juristen-input opslaan, doc-structuur)
- [x] Fase 1 — Stelsel-overzicht + scope-bepaling werkblad opgesteld
- [ ] Fase 2 — Corpus opzetten: 2013/34/EU + 2022/2464/EU als YAML
      (harvester ondersteunt geen EUR-Lex — handmatige YAML voor
      eerste slice, conform principe "machine_readable handmatig
      toevoegen is OK")
- [ ] Fase 3 — Eerste output `valt_onder_csrd` met machine_readable
- [ ] Fase 4 — BDD-personae: 3 archetypen (NovaCorp, MidBV, GroupHolding) + mogelijk 4e (EuroTech Subsidiary — exempt via geconsolideerde rapportage groep, zie open vraag 6 in `scope-bepaling.md`)
- [ ] Fase 5 — Verdere uitwerking: ESRS-standaarden, materialiteit,
      NL-omzetting

## Buiten scope (voor nu)

- **Third-country undertakings** — niet-EU bedrijven met EU-activiteit.
  Bewust uitgesloten door jurist
- **ESRS-detail** — de ~1000 rapportage-datapunten over 12 standaarden.
  Komt na de scope-bepaling-laag
- **Sector-specifieke ESRS** — financiële sector, agri, mijnbouw, etc.
- **NFRD-overgangsrecht** — de oude Non-Financial Reporting Directive
  is vervangen, eventuele transitie-regelingen buiten scope
- **SFDR + Taxonomieverordening** — buurregelingen met conceptuele
  overlap (materialiteit) maar niet de regelhulp-scope

## Hulpbronnen (juristen-input)

- **MVO Wetchecker** — https://www.mvonederland.nl/wetchecker/
- **RVO CSRD-pagina** — https://www.rvo.nl/onderwerpen/csrd
- **SER over Omnibus** — https://www.ser.nl/nl/thema/imvo/csrd-omnibus
- **EU Commission Omnibus** — https://commission.europa.eu/business-economy-euro/doing-business-eu/sustainability-due-diligence-responsible-business/sustainability-related-reporting_en
- **Geconsolideerde Accounting Directive** — https://eur-lex.europa.eu/eli/dir/2013/34/2026-03-18
- **Geconsolideerde CSRD** — https://eur-lex.europa.eu/eli/dir/2022/2464/2026-03-18
