Feature: Financieel CV — werkgever-perspectief, casus Sadee
  Als werkgever (MKB)
  Wil ik weten welke financiële voordelen ik krijg als ik Sadee aanneem
  Zodat ik een onderbouwd besluit kan nemen over het dienstverband

  # Verticale slice door de zeven outputs van het Financieel CV voor één
  # werknemerprofiel. Doel: aan jurist + UWV-uitvoerder + ondernemer
  # concreet tonen wat regelrecht uitrekent én wat het laat zien als
  # "niet van toepassing" of "via andere regelgeving".
  #
  # Werknemerprofiel — Sadee
  #   - 28 jaar (geboren 1996-05-15)
  #   - In dienst per 1 januari 2024
  #   - Wajong-uitkering, doelgroep banenafspraak
  #   - Geschatte loonwaarde 70% van WML (arbeidsprestatie < minimumloon)
  #   - €14,50 per uur × 32 uur per week × 52 weken
  #     = 1664 verloonde uren, jaarloon €24.128 (2.412.800 eurocent)
  #
  # Peildatum 2025-01-15 — BEWUST GEKOZEN, zie kanttekening.
  #
  # De engine pakt per wet de laatst-geldende versie (valid_from <=
  # peildatum). In de CENTRALE corpus bestaan nieuwere, geharvestte
  # versies van alle zeven wetten (Ziektewet/Wajong/WIA/Wtl/WW/Wfsv
  # 2026-01-01, Pwet 2026-04-03) die nog GEEN machine_readable dragen.
  # Bij een peildatum in 2026 zou de engine dus die lege versies laden
  # en niets kunnen doorrekenen.
  #
  # 2025-01-15 ligt in het venster waarin onze gemodelleerde versies
  # overal de laatst-geldende zijn (de eerstvolgende versie is Pwet
  # 2025-02-04). Daarmee draaien de scenario's zowel lokaal als in het
  # traject tegen de corpus die wij daadwerkelijk hebben gemodelleerd.
  #
  # KANTTEKENING voor de jurist: wij tonen dus de wet zoals die medio
  # januari 2025 gold. Sindsdien is er drift — o.a. is LKV-categorie d
  # (herplaatsen arbeidsgehandicapte) per 2026 geschrapt en is de
  # LKS-doelgroep uitgebreid met Pwet 10d.2.c. Dat is een open punt,
  # geen modelleerfout.
  #
  # LIV-afschaffing: Wtl 2025-01-01.yaml is via de harvester opgehaald
  # en aan de corpus toegevoegd; het bestand bevat geen hoofdstuk 3 meer
  # (LIV-artikelen 3.1.1 t/m 3.2.2 vervallen per Wet 36458). De engine
  # geeft op peildatum 2025+ een "Output not found"-error voor
  # heeft_recht_op_liv — wat we hier expliciet asserten.
  #
  # Niet in deze slice:
  #   - Wajong-eigen voorzieningen (art. 2:22 e.v.) voor JC/WPA — Sadee
  #     wordt uitgesloten van WIA art. 35 (lid 4.a) maar zou via Wajong
  #     wel persoonlijke ondersteuning kunnen krijgen. Niet gemodelleerd.
  #   - Cumulatieregel LIV ↔ LKV (Wtl 4.1.3). Beide engines geven
  #     onafhankelijk "recht = true"; de samenloop moet in een
  #     aggregator-laag worden uitgerekend (zie Scenario "Samenloop").

  # ────────────────────────────────────────────────────────────────────
  # LIV — Wtl artikel 3 — AFGESCHAFT per 1 januari 2025 (Wet 36458)
  # In Wtl 2025-01-01.yaml is hoofdstuk 3 verwijderd (artikelen 3.1.1
  # t/m 3.2.2). De engine geeft op peildatum vandaag een "Output not
  # found"-error voor heeft_recht_op_liv. Werkgever-relevantie: de tool
  # moet ondernemers vertellen dat dit voordeel niet meer bestaat.
  Scenario: LIV bestaat niet meer per 2025-01-01 — output is afgeschaft
    Given the calculation date is "2026-06-01"
    And the following parameters:
      | bsn                                          | 999990100 |
      | jaarloon_eurocent                            | 2412800   |
      | verloonde_uren                               | 1664      |
      | heeft_pensioengerechtigde_leeftijd_bereikt   | false     |
    When I evaluate "heeft_recht_op_liv" of "wet_tegemoetkomingen_loondomein"
    Then the execution fails with "Output 'heeft_recht_op_liv' not found in law 'wet_tegemoetkomingen_loondomein'"

  # ────────────────────────────────────────────────────────────────────
  # LKV — Wtl artikel 2.1 + anti-cumulatie art. 4.1 lid 3
  # Sadee voldoet aan twee categorieën: b (arbeidsgehandicapt — Wajong)
  # en c (banenafspraak). Beide tegemoetkomingen worden berekend; het
  # hoogste bedrag wordt verstrekt (art. 4.1 lid 3):
  #   b = MIN(305 × 1664, 600000) = 507520 eurocent (€5.075,20)
  #   c = MIN(101 × 1664, 200000) = 168064 eurocent (€1.680,64)
  # → b wint omdat het de hoogste berekende tegemoetkoming is, niet door
  # IF-volgorde maar door de hoogte-vergelijking.
  Scenario: Sadee krijgt LKV-arbeidsgehandicapt — hoogste tegemoetkoming wint (art. 4.1.3)
    Given the calculation date is "2026-06-01"
    And the following parameters:
      | bsn                                          | 999990100 |
      | verloonde_uren                               | 1664      |
      | is_oudere_werknemer                          | false     |
      | is_arbeidsgehandicapte_werknemer             | true      |
      | is_herplaatsen_arbeidsgehandicapte           | false     |
      | is_doelgroep_banenafspraak                   | true      |
      | heeft_pensioengerechtigde_leeftijd_bereikt   | false     |
      | heeft_loonaangifte_verzoek_ingediend         | true      |
    When I evaluate "heeft_recht_op_lkv" of "wet_tegemoetkomingen_loondomein"
    Then the execution succeeds
    And output "heeft_recht_op_lkv" is true
    And output "categorie_lkv" equals "arbeidsgehandicapte_werknemer"
    And output "tegemoetkoming_arbeidsgehandicapte_eurocent" equals 507520
    And output "tegemoetkoming_banenafspraak_eurocent" equals 168064
    And output "hoogte_lkv_per_jaar_eurocent" equals 507520
