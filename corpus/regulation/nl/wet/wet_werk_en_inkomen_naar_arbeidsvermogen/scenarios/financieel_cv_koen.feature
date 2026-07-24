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
  # JC + WPA — Wet WIA artikel 35
  # Koen heeft geen Wajong-status (lid 4.a niet van toepassing) en valt
  # niet onder college-zorg-uitsluiting (Pwet 7.1.a, lid 4.b). UWV kan
  # dus zelf voorzieningen toekennen. Werknemer-relevantie: Koen kan
  # jobcoaching en werkplekaanpassing zelf aanvragen bij UWV.
  Scenario: Koen kan jobcoaching en werkplekaanpassing aanvragen via UWV
    Given the calculation date is "2026-06-01"
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
