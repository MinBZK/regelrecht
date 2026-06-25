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
  # Peildatum 2026-05-11 (vandaag) voor alle scenarios. De corpus
  # modelleert valid_from per wet-versie; elke wet blijft geldig totdat
  # er een opvolger komt. Voor onze 7 wetten geldt vandaag:
  #   - WW: 2024-01-01.yaml (geen opvolger → nog geldend)
  #   - Wtl: 2025-01-01.yaml (opvolger van 2024; LIV per 2025 afgeschaft)
  #   - Ziektewet, Wajong, WIA, Pwet: 2025-01-01.yaml (geen opvolger)
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
  # NRP — Ziektewet artikel 29b
  # Sadee triggert lid 1 (Wajong-status) én lid 2.e (banenafspraak).
  # Lid 2 geeft duur -1 (onbeperkt) en wint van de 5-jaars termijn van lid 1.
  Scenario: Sadee krijgt no-risk polis als Wajonger op banenafspraakregister
    Given the calculation date is "2026-05-11"
    And a citizen with the following data:
      | bsn                              | 999990100 |
      | is_wsw_werknemer                 | false     |
      | is_wia_uitkeringsgerechtigd      | false     |
      | is_wia_min_35_arbeidsongeschikt  | false     |
      | heeft_voortgezet_wia_recht       | false     |
      | heeft_arbeidsbeperking_wia       | false     |
      | is_wajong_gerechtigd             | true      |
      | is_jonggehandicapt_schoolverlater | false    |
      | is_banenafspraak_doelgroep       | true      |
      | is_pwet_loonkostensubsidie       | false     |
      | is_beschut_werk                  | false     |
      | loonwaarde_lager_dan_minimumloon | true      |
    When the law "ziektewet" is executed for outputs "heeft_recht_op_no_risk_polis,duur_no_risk_polis_jaren"
    Then the execution succeeds
    And the output "heeft_recht_op_no_risk_polis" is "true"
    And the output "duur_no_risk_polis_jaren" is "-1"

  # ────────────────────────────────────────────────────────────────────
  # LDP — Wajong artikel 2:20
  # Werkgever heeft loondispensatie aangevraagd voor Sadee; UWV stelt
  # vast dat haar arbeidsprestatie < minimumloon is.
  Scenario: Sadee komt in aanmerking voor loondispensatie via Wajong art. 2:20
    Given the calculation date is "2026-05-11"
    And a citizen with the following data:
      | bsn                                                | 999990100 |
      | is_wsw_werknemer                                   | false     |
      | arbeidsprestatie_duidelijk_minder_dan_minimumloon  | true      |
      | aanvraag_loondispensatie_ingediend                 | true      |
      | heeft_recht_op_arbeidsondersteuning_wajong         | true      |
    When the law "wet_arbeidsongeschiktheidsvoorziening_jonggehandicapten" is executed for outputs "heeft_recht_op_loondispensatie,beding_lagere_beloning_is_nietig"
    Then the execution succeeds
    And the output "heeft_recht_op_loondispensatie" is "true"
    And the output "beding_lagere_beloning_is_nietig" is "true"

  # ────────────────────────────────────────────────────────────────────
  # JC + WPA — Wet WIA artikel 35
  # KEY INSIGHT: lid 4.a sluit Wajong-gerechtigden uit van artikel 35-
  # voorzieningen. Werkgever die JC/WPA voor Sadee verwacht moet weten:
  # die loopt niet via WIA, maar via Wajong-eigen voorzieningen
  # (art. 2:22 e.v. — niet in deze slice gemodelleerd).
  Scenario: Sadee valt buiten WIA artikel 35 voor JC en WPA (lid 4.a Wajong)
    Given the calculation date is "2026-05-11"
    And a citizen with the following data:
      | bsn                                              | 999990100 |
      | heeft_structurele_functionele_beperking          | true      |
      | heeft_arbeidsverhouding_of_voorbereiding         | true      |
      | is_wsw_werknemer                                 | false     |
      | heeft_recht_op_arbeidsondersteuning_wajong       | true      |
      | pwet_college_draagt_zorg_uitsluiting             | false     |
      | aanvraag_jobcoaching_ingediend                   | true      |
      | aanvraag_werkplekaanpassing_ingediend            | true      |
    When the law "wet_werk_en_inkomen_naar_arbeidsvermogen" is executed for outputs "artikel_35_van_toepassing,heeft_recht_op_jobcoaching,heeft_recht_op_werkplekaanpassing"
    Then the execution succeeds
    And the output "artikel_35_van_toepassing" is "false"
    And the output "heeft_recht_op_jobcoaching" is "false"
    And the output "heeft_recht_op_werkplekaanpassing" is "false"

  # ────────────────────────────────────────────────────────────────────
  # LIV — Wtl artikel 3 — AFGESCHAFT per 1 januari 2025 (Wet 36458)
  # In Wtl 2025-01-01.yaml is hoofdstuk 3 verwijderd (artikelen 3.1.1
  # t/m 3.2.2). De engine geeft op peildatum vandaag een "Output not
  # found"-error voor heeft_recht_op_liv. Werkgever-relevantie: de tool
  # moet ondernemers vertellen dat dit voordeel niet meer bestaat.
  Scenario: LIV bestaat niet meer per 2025-01-01 — output is afgeschaft
    Given the calculation date is "2026-05-11"
    And a citizen with the following data:
      | bsn                                          | 999990100 |
      | jaarloon_eurocent                            | 2412800   |
      | verloonde_uren                               | 1664      |
      | heeft_pensioengerechtigde_leeftijd_bereikt   | false     |
    When the law "wet_tegemoetkomingen_loondomein" is executed for outputs "heeft_recht_op_liv"
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
    Given the calculation date is "2026-05-11"
    And a citizen with the following data:
      | bsn                                          | 999990100 |
      | verloonde_uren                               | 1664      |
      | is_oudere_werknemer                          | false     |
      | is_arbeidsgehandicapte_werknemer             | true      |
      | is_herplaatsen_arbeidsgehandicapte           | false     |
      | is_doelgroep_banenafspraak                   | true      |
      | heeft_pensioengerechtigde_leeftijd_bereikt   | false     |
      | heeft_loonaangifte_verzoek_ingediend         | true      |
    When the law "wet_tegemoetkomingen_loondomein" is executed for outputs "heeft_recht_op_lkv,categorie_lkv,tegemoetkoming_arbeidsgehandicapte_eurocent,tegemoetkoming_banenafspraak_eurocent,hoogte_lkv_per_jaar_eurocent"
    Then the execution succeeds
    And the output "heeft_recht_op_lkv" is "true"
    And the output "categorie_lkv" is "arbeidsgehandicapte_werknemer"
    And the output "tegemoetkoming_arbeidsgehandicapte_eurocent" is "507520"
    And the output "tegemoetkoming_banenafspraak_eurocent" is "168064"
    And the output "hoogte_lkv_per_jaar_eurocent" is "507520"

  # ────────────────────────────────────────────────────────────────────
  # PP — WW artikel 76a
  # Sadee heeft geen WW-uitkering (komt uit Wajong), dus PP is niet aan
  # de orde. Werkgever moet weten: proefplaatsing met behoud van uitkering
  # is een WW-instrument, niet beschikbaar voor Wajongers.
  Scenario: Sadee mag geen proefplaatsing aangaan — geen WW-uitkering
    Given the calculation date is "2026-05-11"
    And a citizen with the following data:
      | bsn                                            | 999990100 |
      | heeft_recht_op_ww_uitkering                    | false     |
      | in_staat_tot_werkzaamheden                     | true      |
      | aansprakelijkheidsverzekering_aanwezig         | true      |
      | niet_eerder_proefplaatsing_zelfde_werkgever    | true      |
      | reeel_uitzicht_op_dienstbetrekking_zes_maanden | true      |
    When the law "werkloosheidswet" is executed for outputs "mag_proefplaatsing_aangaan,ww_uitkering_blijft_bestaan"
    Then the execution succeeds
    And the output "mag_proefplaatsing_aangaan" is "false"
    And the output "ww_uitkering_blijft_bestaan" is "false"

  # ────────────────────────────────────────────────────────────────────
  # LKS — Participatiewet artikel 10c + 10d
  # Sadee zit niet in de Pwet-doelgroep (zij heeft Wajong, geen
  # bijstand). LKS is een gemeente-instrument voor Pwet-doelgroep met
  # loonwaarde < 100%, dus niet van toepassing.
  Scenario: Sadee komt niet in aanmerking voor LKS — geen Pwet-doelgroep
    Given the calculation date is "2026-05-11"
    And a citizen with the following data:
      | bsn                                                | 999990100 |
      | behoort_tot_doelgroep_lks                          | false     |
      | kan_minimumloon_niet_verdienen                     | true      |
      | aanvraag_lks_ingediend_binnen_zes_maanden          | true      |
      | voorafgaand_relevante_onderwijsroute_of_doelgroep  | true      |
      | is_wsw_dienstbetrekking                            | false     |
      | loonwaarde_eurocent_per_maand                      | 150850    |
      | minimumloon_plus_vakantiebijslag_eurocent_per_maand | 215500   |
    When the law "participatiewet" is executed for outputs "heeft_recht_op_lks,hoogte_lks_eurocent_per_maand"
    Then the execution succeeds
    And the output "heeft_recht_op_lks" is "false"
    And the output "hoogte_lks_eurocent_per_maand" is "0"

  # ────────────────────────────────────────────────────────────────────
  # OPEN: Samenloop LIV ↔ LKV (Wtl artikel 4.1.3)
  #
  # De engine geeft per wet onafhankelijk "recht = true" voor zowel LIV
  # (€815) als LKV-arbeidsgehandicapt (€5.075). De cumulatieregel
  # Wtl 4.1.3 verbiedt dat beide tegelijk uitgekeerd worden in hetzelfde
  # dienstverband-jaar. Voor Sadee betekent dit: kies LKV (hogere
  # opbrengst, langere looptijd).
  #
  # Deze samenloop is NIET door één van de bovenstaande scenarios gedekt
  # — een aggregator-laag die meerdere wetten orchestreert en cumulatie-
  # regels uitvoert is een open ontwerpvraag. Wanneer die er is, komt
  # hier een Scenario dat asserteert dat het Financieel CV de werkgever
  # vertelt "u krijgt LKV; LIV vervalt omdat ze niet samen mogen".
