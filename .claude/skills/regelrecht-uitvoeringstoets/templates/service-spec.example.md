# Service-spec — neutraal voorbeeld

Illustratief voorbeeld met **placeholders** (geen echte casus-waarden). De skill genereert
een ingevulde versie hiervan uit het corpus; die ingevulde versie leeft uitsluitend bij de
(privé) PoC, nooit in deze skill of de template. Voorbeelden/demo's draaien tegen een
dummy-corpus. Zie `references/service-spec.md` voor het contract en de afleiding.

```yaml
casus: <voorbeeld-casus>
orchestrator_law: <hoofdregeling-id>

lenzen: [proef, burger, behandelaar, meekijken]   # of [headless] bij ambtshalve/plugin

levenscyclus:                                       # uit hooks
  - { fase: AANVRAAG }
  - { fase: BESLUIT, produceert_beschikking: true } # uit legal_character
  - { fase: BEZWAAR }

formulier:                                          # uit caller-params + type_spec
  outputs: [<uitkomst-a>, <bedrag-b>]
  groepen:
    - kort: <groep>
      velden:
        - key: <leaf_param>
          type: euro                                # of: getal | keuze | ja_nee
          herkomst: burger                          # burger | ander_systeem | oordeel
          label: "<B1-label — redactie, te reviewen>"
  gates: []                                         # poortvragen (progressive disclosure)
  escalaties: []                                    # toon_als-regels (regel-escalatie)

keten:                                              # uit source/implements
  - { knoop: <tussenresultaat>, artikel: <regeling>_<art> }

termijnen:                                          # uit type_spec days/weeks
  herstel: { output: <termijn-output>, unit: weeks }

mens_taken_bron: { law: <regeling>, article: "<nr>" }   # uit untranslatables

# validatie-instrumentatie
wat_als:                                            # uit het open-vragen-register
  - naam: "<open beleidskeuze>"
    varianten: [{ label: "Variant A (corpus)" }, { label: "Variant B (praktijk)" }]
open_vragen: []
herkomst_annotaties: true
```
