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
  # LIV — Wtl artikel 3 — AFGESCHAFT per 1 januari 2025 (Wet 36458)
  # In Wtl 2025-01-01.yaml is hoofdstuk 3 verwijderd. De engine geeft
  # op peildatum vandaag een "Output not found"-error voor
  # heeft_recht_op_liv. Werknemer-relevantie: transparantie — Koen
  # weet dat dit voordeel niet (meer) bestaat.
  Scenario: LIV bestaat niet meer per 2025-01-01 — geen LIV voor werkgever Koen
    Given the calculation date is "2026-06-01"
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
    Given the calculation date is "2026-06-01"
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
