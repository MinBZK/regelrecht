# Corpus

This directory contains regulation sources for the regelrecht engine.

## Structure

```
corpus/
├── central/           # Central corpus (MinBZK)
│   └── regulation/
│       └── nl/
│           ├── wet/                          # Formal laws
│           ├── ministeriele_regeling/        # Ministerial regulations
│           └── gemeentelijke_verordening/    # Municipal ordinances
└── README.md
```

## Sources

The central corpus (`corpus/central/`) contains the primary set of regulations maintained by MinBZK. Additional sources (municipalities, provinces, etc.) can be registered via `corpus-registry.yaml` and loaded from local directories or remote GitHub repositories.

## Priority

Sources are assigned a priority value where **lower value = higher priority**. The central corpus has priority 1 (highest). When multiple sources provide a regulation with the same `$id`, the source with the lowest priority value wins.
