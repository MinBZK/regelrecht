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
  # Peildatum 2026-05-11 (vandaag). De corpus pakt per wet de laatst-
  # geldende versie (valid_from <= peildatum): Wtl 2025-01-01.yaml
  # (LIV per 2025 afgeschaft), WW 2024-01-01.yaml (geen opvolger),
  # Ziektewet/Wajong/WIA/Pwet 2025-01-01.yaml. Zie
  # financieel_cv_sadee.feature voor uitleg over de LIV-afschaffing.

  # ────────────────────────────────────────────────────────────────────
  # NRP — Ziektewet artikel 29b
  # Koen triggert lid 2.e (banenafspraak via Pwet + LKS) — onbeperkte duur.
  # Werknemer-relevantie: openhartig kunnen zijn over gezondheid; zieke-
  # dagen belasten werkgever niet.
  Scenario: Koen krijgt no-risk polis als banenafspraak-werknemer met LKS
    Given the calculation date is "2026-05-11"
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

  # ────────────────────────────────────────────────────────────────────
  # LDP — Wajong artikel 2:20
  # Koen heeft geen Wajong-status (komt uit Pwet) → loondispensatie via
  # Wajong is niet aan de orde. Werknemer-relevantie: zekerheid dat werk-
  # gever niet via deze route minder dan WML mag betalen.
  Scenario: Koen heeft geen recht op loondispensatie — geen Wajong-status
    Given the calculation date is "2026-05-11"
    And the following parameters:
      | bsn                                                | 999990101 |
      | is_wsw_werknemer                                   | false     |
      | arbeidsprestatie_duidelijk_minder_dan_minimumloon  | true      |
      | aanvraag_loondispensatie_ingediend                 | false     |
      | heeft_recht_op_arbeidsondersteuning_wajong         | false     |
    When I evaluate "heeft_recht_op_loondispensatie" of "wet_arbeidsongeschiktheidsvoorziening_jonggehandicapten"
    Then the execution succeeds
    And output "heeft_recht_op_loondispensatie" is false

  # ────────────────────────────────────────────────────────────────────
  # JC + WPA — Wet WIA artikel 35
  # Koen heeft geen Wajong-status (lid 4.a niet van toepassing) en valt
  # niet onder college-zorg-uitsluiting (Pwet 7.1.a, lid 4.b). UWV kan
  # dus zelf voorzieningen toekennen. Werknemer-relevantie: Koen kan
  # jobcoaching en werkplekaanpassing zelf aanvragen bij UWV.
  Scenario: Koen kan jobcoaching en werkplekaanpassing aanvragen via UWV
    Given the calculation date is "2026-05-11"
    And the following parameters:
      | bsn                                              | 999990101 |
      | heeft_structurele_functionele_beperking          | true      |
      | heeft_arbeidsverhouding_of_voorbereiding         | true      |
      | is_wsw_werknemer                                 | false     |
      | heeft_recht_op_arbeidsondersteuning_wajong       | false     |
      | pwet_college_draagt_zorg_uitsluiting             | false     |
      | aanvraag_jobcoaching_ingediend                   | true      |
      | aanvraag_werkplekaanpassing_ingediend            | true      |
    When I evaluate "artikel_35_van_toepassing" of "wet_werk_en_inkomen_naar_arbeidsvermogen"
    Then the execution succeeds
    And output "artikel_35_van_toepassing" is true
    And output "heeft_recht_op_jobcoaching" is true
    And output "heeft_recht_op_werkplekaanpassing" is true

  # ────────────────────────────────────────────────────────────────────
  # LIV — Wtl artikel 3 — AFGESCHAFT per 1 januari 2025 (Wet 36458)
  # In Wtl 2025-01-01.yaml is hoofdstuk 3 verwijderd. De engine geeft
  # op peildatum vandaag een "Output not found"-error voor
  # heeft_recht_op_liv. Werknemer-relevantie: transparantie — Koen
  # weet dat dit voordeel niet (meer) bestaat.
  Scenario: LIV bestaat niet meer per 2025-01-01 — geen LIV voor werkgever Koen
    Given the calculation date is "2026-05-11"
    And the following parameters:
      | bsn                                          | 999990101 |
      | jaarloon_eurocent                            | 1996800   |
      | verloonde_uren                               | 1664      |
      | heeft_pensioengerechtigde_leeftijd_bereikt   | false     |
    When I evaluate "heeft_recht_op_liv" of "wet_tegemoetkomingen_loondomein"
    Then the execution fails with "Output 'heeft_recht_op_liv' not found in law 'wet_tegemoetkomingen_loondomein'"

  # ────────────────────────────────────────────────────────────────────
  # LKV — Wtl 2025: art. 2.1 (recht) + art. 2.13 (bedrag banenafspraak)
  # + art. 4.1.3 (anti-cumulatie: hoogte = hoogste categorie).
  # Koen valt in categorie c (banenafspraak), niet in b (arbeids-
  # gehandicapt — hij heeft geen WIA/Wajong). Hoogte = MIN(101 × 1664,
  # 200000) = MIN(168064, 200000) = 168064 eurocent (€1.680,64 per jaar).
  # Werknemer-relevantie: transparantie — Koen weet welk bedrag werk-
  # gever ontvangt en gedurende hoe lang.
  Scenario: Werkgever ontvangt LKV-banenafspraak voor Koen — €1.680,64 per jaar
    Given the calculation date is "2026-05-11"
    And the following parameters:
      | bsn                                          | 999990101 |
      | verloonde_uren                               | 1664      |
      | is_oudere_werknemer                          | false     |
      | is_arbeidsgehandicapte_werknemer             | false     |
      | is_herplaatsen_arbeidsgehandicapte           | false     |
      | is_doelgroep_banenafspraak                   | true      |
      | heeft_pensioengerechtigde_leeftijd_bereikt   | false     |
      | heeft_loonaangifte_verzoek_ingediend         | true      |
    When I evaluate "heeft_recht_op_lkv" of "wet_tegemoetkomingen_loondomein"
    Then the execution succeeds
    And output "heeft_recht_op_lkv" is true
    And output "categorie_lkv" equals "banenafspraak"
    And output "tegemoetkoming_banenafspraak_eurocent" equals 168064
    And output "hoogte_lkv_per_jaar_eurocent" equals 168064

  # ────────────────────────────────────────────────────────────────────
  # PP — WW artikel 76a
  # Koen heeft geen WW-uitkering (komt uit Pwet). PP met behoud van
  # uitkering is een WW-instrument en niet van toepassing. Werknemer-
  # relevantie: Koen kan geen onbetaalde proefplaatsing aangaan met
  # behoud van Pwet-uitkering — voor Pwet geldt een ander
  # re-integratie-traject via de gemeente.
  Scenario: Koen kan geen proefplaatsing met behoud van uitkering — geen WW
    Given the calculation date is "2026-05-11"
    And the following parameters:
      | bsn                                            | 999990101 |
      | heeft_recht_op_ww_uitkering                    | false     |
      | in_staat_tot_werkzaamheden                     | true      |
      | aansprakelijkheidsverzekering_aanwezig         | true      |
      | niet_eerder_proefplaatsing_zelfde_werkgever    | true      |
      | reeel_uitzicht_op_dienstbetrekking_zes_maanden | true      |
    When I evaluate "mag_proefplaatsing_aangaan" of "werkloosheidswet"
    Then the execution succeeds
    And output "mag_proefplaatsing_aangaan" is false
    And output "ww_uitkering_blijft_bestaan" is false

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
    Given the calculation date is "2026-05-11"
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
