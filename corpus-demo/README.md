# corpus-demo

Demo-only corpus-toevoegingen voor de burger-demo (`frontend-demo/`).

**Dit pad is niet de bron voor productie.** YAML hier mag afwijken van de letter van de wet, zolang de afwijkingen expliciet zijn vermeld. Gebruik niets uit dit pad als referentie voor echt wetgevingswerk — daarvoor is `corpus/regulation/`.

Zie [RFC-016](../docs/rfcs/rfc-016.md) voor de achtergrond.

## Wat mag hier wel

- **Persona-data** (`profiles/`). Synthetische BSN's, gefingeerde inkomens, gefingeerde relaties. Zie `profiles/merijn.yaml`.
- **Demo-index** (`demo-index.yaml`). De lijst van wetten die in de demo zichtbaar zijn. Mag naar `corpus/regulation/` wijzen voor wetten die al law-faithful gemodelleerd zijn.
- **Demo-only YAML** (`regulation/`). Wetten die in het hoofdcorpus nog niet bestaan of daar bewust niet thuishoren omdat de modellering vereenvoudigingen bevat. Elke demo-wet krijgt zijn eigen sub-README die de afwijkingen opsomt.

## Wat mag hier niet

- **Kopieën van law-faithful wetten.** Als een wet al in `corpus/regulation/` staat en klopt, hergebruik via `demo-index.yaml`.
- **Stille afwijkingen.** Elke simplification moet in een README bij de wet vermeld staan met verwijzing naar wat in de echte wet anders is.

## Checklist voor een nieuwe demo-wet

- [ ] Directory `regulation/<naam>/` aangemaakt.
- [ ] Sub-README vermeldt welke artikelen zijn gemodelleerd en welke niet.
- [ ] Sub-README somt de demo-simplifications expliciet op (vaste bedragen, weggelaten randgevallen, vereenvoudigde temporale semantiek).
- [ ] YAML valideert tegen het schema (`cargo run -p corpus-cli -- validate corpus-demo/regulation/<naam>/<datum>.yaml`).
- [ ] Minstens één BDD-scenario in `corpus-demo/features/` dat de demo-flow afdekt.
- [ ] Wet toegevoegd aan `corpus-demo/demo-index.yaml`.

## Structuur

```
corpus-demo/
├── README.md              (dit bestand)
├── demo-index.yaml        (welke wetten in de demo)
├── profiles/              (persona-data)
│   └── merijn.yaml
└── regulation/            (demo-only wetten, v1 leeg)
```

## CI

`just validate` slaat `corpus-demo/` over om te voorkomen dat demo-simplifications als schending van law-faithfulness worden opgevat. YAML-lint en pre-commit hooks draaien wel.
