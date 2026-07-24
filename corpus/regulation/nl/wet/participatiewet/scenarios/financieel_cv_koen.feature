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
  # LKS — Participatiewet artikel 10c + 10d
  # Koen heeft loonwaarde 60% van WML+VB (215500 ec/mnd) = 129300 ec.
  # Bruto subsidie = 215500 - 129300 = 86200 ec.
  # Max 70% van WML+VB = 150850 ec.
  # Hoogte = MIN(86200, 150850) = 86200 eurocent (€862 per maand).
  # Werknemer-relevantie: Koen weet dat gemeente per maand €862 aan zijn
  # werkgever betaalt om hem het WML-loon te kunnen geven. Vertrouwens-
  # vergroting: hij weet waar het loon vandaan komt.
  #
  # NB: WML+VB-waarde 215500 is 2025-cijfer; voor exacte 2026-cijfers
  # moet deze input worden geüpdatet uit officiële bron.
  Scenario: Gemeente betaalt €862 per maand LKS aan werkgever van Koen
    Given the calculation date is "2026-06-01"
    And the following parameters:
      | bsn                                                | 999990101 |
      | behoort_tot_doelgroep_lks                          | true      |
      | kan_minimumloon_niet_verdienen                     | true      |
      | aanvraag_lks_ingediend_binnen_zes_maanden          | true      |
      | voorafgaand_relevante_onderwijsroute_of_doelgroep  | true      |
      | is_wsw_dienstbetrekking                            | false     |
      | loonwaarde_eurocent_per_maand                      | 129300    |
      | minimumloon_plus_vakantiebijslag_eurocent_per_maand | 215500   |
    When I evaluate "heeft_recht_op_lks" of "participatiewet"
    Then the execution succeeds
    And output "heeft_recht_op_lks" is true
    And output "bruto_subsidie_eurocent_per_maand" equals 86200
    And output "maximum_subsidie_eurocent_per_maand" equals 150850
    And output "hoogte_lks_eurocent_per_maand" equals 86200

  # ────────────────────────────────────────────────────────────────────
  # OPEN: Samenloop LKS ↔ LKV (Pwet 10d lid 9)
  #
  # Engine geeft onafhankelijk "recht = true" voor zowel LKS (gemeente,
  # €862/mnd) als LKV-banenafspraak (Belastingdienst, €1.680/jaar).
  # Pwet 10d lid 9 bevat een samenloopverbod tussen LKS en bepaalde
  # andere subsidies — moet door aggregator-laag worden uitgerekend.
  #
  # Wanneer de aggregator er is, komt hier een Scenario dat asserteert
  # of LKS en LKV-banenafspraak tegelijk uitgekeerd mogen worden.
