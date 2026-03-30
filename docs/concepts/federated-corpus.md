# Federated Corpus

Dutch legislation is produced by many authorities: Parliament, ministers, 342 municipalities, 12 provinces, 21 water boards. Maintaining all regulations in a single repository does not match this reality.

The federated corpus model lets each authority maintain their own regulations in their own Git repository. The engine discovers and loads laws from all sources through a registry.

## The registry

A `corpus-registry.yaml` file lists all regulation sources:

```yaml
schema_version: "1.0"
sources:
  - id: minbzk-central
    name: "MinBZK Central Corpus"
    type: github
    github:
      owner: MinBZK
      repo: regelrecht-corpus
      branch: main
      path: regulation/nl
    scopes: []
    priority: 1

  - id: amsterdam
    name: "Gemeente Amsterdam"
    type: github
    github:
      owner: gemeente-amsterdam
      repo: regelrecht-amsterdam
      branch: main
      path: regulation/nl
    scopes:
      - type: gemeente_code
        value: "GM0363"
    priority: 10
    auth_ref: amsterdam
```

Each source declares:
- **Where** to find the regulations (a GitHub repository or local directory)
- **Scope** (jurisdiction claims, like `gemeente_code: GM0363` for Amsterdam)
- **Priority** (lower number = higher priority, used when the same law appears in multiple sources)

## How it works

The engine merges laws from all registered sources into a single corpus at load time. Scope information is used to filter: when executing for a person in Amsterdam, only Amsterdam's municipal ordinances apply.

The `implements` mechanism from [Inversion of Control](./inversion-of-control) works across repositories. Amsterdam's afstemmingsverordening (in Amsterdam's repo) can implement open terms from the Participatiewet (in the central repo). The engine does not care which repository a file came from.

## Local overrides

Developers can add a `corpus-registry.local.yaml` (gitignored) to add personal sources during development. This file extends the main registry without affecting the shared configuration.

## Authentication

Credentials for private repositories are stored separately from the manifest. The registry uses `auth_ref` identifiers that map to environment variables (`CORPUS_AUTH_{ID}_TOKEN`) or a `corpus-auth.yaml` file. No secrets in the manifest itself.

## Editor integration

Municipalities can edit, validate, and submit their regulations through the web editor without needing a development environment. The editor writes directly to the appropriate GitHub repository via the Contents API, creating branches and pull requests.

## Further reading

- [Inversion of Control](./inversion-of-control) - how `implements` works across repositories
- [Multi-Org Execution](./multi-org-execution) - how organizational boundaries affect execution
- [RFC-010: Federated Corpus](/rfcs/rfc-010) - the full design specification
