# Corpus

Internal test regulations for validating the regelrecht engine.

## Purpose

These regulations are maintained by us to create isolated, controlled scenarios for testing the engine. They are **not** the public corpus — that lives in a separate repository.

## Structure

```
corpus/
├── regulation/
│   └── nl/
│       ├── wet/                          # Formal laws
│       │   └── <law_id>/
│       │       ├── <valid_from>.yaml     # One file per version
│       │       └── scenarios/            # BDD scenarios validating this law
│       ├── ministeriele_regeling/        # Ministerial regulations
│       └── gemeentelijke_verordening/    # Municipal ordinances
├── annotations/
│   ├── <law_id>/annotations.yaml         # Note sidecars (RFC-018)
│   └── _vocabulary/                      # Shared ambiguity tag vocabulary
└── README.md
```
