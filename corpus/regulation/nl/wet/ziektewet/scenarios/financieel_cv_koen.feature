Feature: Financieel CV — werknemer-perspectief, casus Koen
  Als werknemer (Pwet-doelgroep, banenafspraak)
  Wil ik weten welke voorzieningen en voordelen mijn nieuwe dienstverband
  meebrengt
  Zodat ik weet wat ik zelf kan aanvragen en welk financieel kader er
  voor mijn werkgever achter mijn dienstverband zit

  # Verticale slice door de Financieel CV vanuit werknemer-perspectief.
  # Sommige outputs zijn werkgever-voordelen (LKS, LKV) — die zijn voor
  # transparantie meegenomen: een werknemer mag weten wat de
  # gemeente/Belastingdienst aan zijn werkgever uitkeert om hem een
  # fatsoenlijk loon te kunnen geven.
  #
  # De engine-assertions zijn dezelfde als bij werkgever-perspectief.
  # Het persona-verschil zit in de presentatie-laag (brief-template),
  # die buiten scope is voor deze slice.
  #
  # Werknemerprofiel — Koen
  #   - 42 jaar (geboren 1984-03-15)
  #   - Pwet-uitkering, doelgroep banenafspraak
  #   - Geschatte loonwaarde 60% van WML
  #   - In dienst per 1 januari 2026 bij logistiek MKB via gemeente
  #   - €12 per uur × 32 uur per week × 52 weken
  #     = 1664 verloonde uren, jaarloon €19.968 (1.996.800 eurocent)
  #
  # Peildatum 2025-01-15 — BEWUST GEKOZEN. De engine pakt per wet de
  # laatst-geldende versie (valid_from <= peildatum). In de centrale
  # corpus bestaan nieuwere geharvestte versies (2026) zonder
  # machine_readable; bij een peildatum in 2026 laadt de engine die en
  # kan hij niets doorrekenen. 2025-01-15 ligt in het venster waarin
  # onze gemodelleerde versies overal de laatst-geldende zijn.
  # Kanttekening + drift-uitleg: zie financieel_cv_sadee.feature.

  # ────────────────────────────────────────────────────────────────────
  # NRP — Ziektewet artikel 29b
  # Koen triggert lid 2.e (banenafspraak via Pwet + LKS) — onbeperkte duur.
  # Werknemer-relevantie: openhartig kunnen zijn over gezondheid; zieke-
  # dagen belasten werkgever niet.
  Scenario: Koen krijgt no-risk polis als banenafspraak-werknemer met LKS
    Given the calculation date is "2026-06-01"
    And the following parameters:
      | bsn                              | 999990101 |
      | is_wsw_werknemer                 | false     |
      | is_wia_uitkeringsgerechtigd      | false     |
      | is_wia_min_35_arbeidsongeschikt  | false     |
      | heeft_voortgezet_wia_recht       | false     |
      | heeft_arbeidsbeperking_wia       | false     |
      | is_wajong_gerechtigd             | false     |
      | is_jonggehandicapt_schoolverlater | false    |
      | is_banenafspraak_doelgroep       | true      |
      | is_pwet_loonkostensubsidie       | true      |
      | is_beschut_werk                  | false     |
      | loonwaarde_lager_dan_minimumloon | true      |
    When I evaluate "heeft_recht_op_no_risk_polis" of "ziektewet"
    Then the execution succeeds
    And output "heeft_recht_op_no_risk_polis" is true
    And output "duur_no_risk_polis_jaren" equals -1
