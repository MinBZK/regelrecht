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
  # JC + WPA — Wet WIA artikel 35
  # KEY INSIGHT: lid 4.a sluit Wajong-gerechtigden uit van artikel 35-
  # voorzieningen. Werkgever die JC/WPA voor Sadee verwacht moet weten:
  # die loopt niet via WIA, maar via Wajong-eigen voorzieningen
  # (art. 2:22 e.v. — niet in deze slice gemodelleerd).
  Scenario: Sadee valt buiten WIA artikel 35 voor JC en WPA (lid 4.a Wajong)
    Given the calculation date is "2026-06-01"
    And the following parameters:
      | bsn                                              | 999990100 |
      | heeft_structurele_functionele_beperking          | true      |
      | heeft_arbeidsverhouding_of_voorbereiding         | true      |
      | is_wsw_werknemer                                 | false     |
      | heeft_recht_op_arbeidsondersteuning_wajong       | true      |
      | pwet_college_draagt_zorg_uitsluiting             | false     |
      | aanvraag_jobcoaching_ingediend                   | true      |
      | aanvraag_werkplekaanpassing_ingediend            | true      |
    When I evaluate "artikel_35_van_toepassing" of "wet_werk_en_inkomen_naar_arbeidsvermogen"
    Then the execution succeeds
    And output "artikel_35_van_toepassing" is false
    And output "heeft_recht_op_jobcoaching" is false
    And output "heeft_recht_op_werkplekaanpassing" is false
