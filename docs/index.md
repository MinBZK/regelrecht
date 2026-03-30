---
layout: home
hero:
  name: RegelRecht
  text: Machine-Readable Dutch Law
  tagline: Dutch legislation you can run, test, and trace back to the source.
  actions:
    - theme: brand
      text: How It Works
      link: /concepts/how-it-works
    - theme: alt
      text: Get Started
      link: /guide/getting-started
    - theme: alt
      text: GitHub
      link: https://github.com/MinBZK/regelrecht
features:
  - title: Executable Law
    details: Dutch legislation encoded in YAML and executed by a deterministic engine. Every decision traces back to its legal source.
  - title: Cross-Law References
    details: Laws reference each other just as in statute books. The engine resolves dependencies automatically across the entire corpus.
  - title: Delegation and IoC
    details: Higher laws delegate to lower regulations. The engine discovers implementations at runtime, matching the real legal hierarchy.
  - title: Test-Driven Legislation
    details: BDD scenarios derived from the Memorie van Toelichting verify that machine-readable law matches what Parliament intended.
  - title: Multi-Organization Execution
    details: Different government organizations handle different laws. The engine models these boundaries and can exchange signed results between organizations.
  - title: Open Source
    details: Built by the Dutch Ministry of the Interior (MinBZK). All law, all tooling, all decisions are open and auditable.
---

## What does a machine-readable law look like?

```yaml
# Wet op de zorgtoeslag, article 2
- number: '2'
  text: |
    De verzekerde heeft aanspraak op een zorgtoeslag ter hoogte
    van het verschil tussen de standaardpremie en de normpremie...
  machine_readable:
    execution:
      input:
        - name: toetsingsinkomen
          source:
            regulation: algemene_wet_inkomensafhankelijke_regelingen
            output: toetsingsinkomen
            parameters:
              bsn: $bsn
      output:
        - name: hoogte_zorgtoeslag
          type: amount
      actions:
        - output: hoogte_zorgtoeslag
          value:
            operation: MAX
            values:
              - 0
              - operation: SUBTRACT
                values:
                  - $standaardpremie
                  - $normpremie
```

Each article sits alongside the original legal text. The `machine_readable` section defines inputs, outputs, and the calculation logic. The engine executes this directly.

## Who is this for?

**Developers** building government services that implement Dutch law. Use the engine to compute legal outcomes instead of hand-coding rules.

**Legal experts** validating whether machine-readable interpretations match the law. The execution-first methodology uses concrete test cases from parliamentary documents.

**Policy makers** exploring how legislation works in practice. Run "what if" scenarios in the browser to see how changing a parameter affects outcomes across multiple laws.

[Learn how it works →](/concepts/how-it-works)
