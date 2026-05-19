# Contributing

RegelRecht is open source and welcomes contributions. This page covers the workflow.

## Branching model

The project uses GitFlow:

- `main` - production, always deployable
- `feature/*` - new features
- `fix/*` - bug fixes
- `docs/*` - documentation changes

Create your branch from `main`, open a PR back to `main`.

## Commit conventions

Commits follow [Conventional Commits](https://www.conventionalcommits.org/):

```
type(scope): description
```

Types: `feat`, `fix`, `docs`, `style`, `test`, `chore`, `refactor`

Subject line should be 72 characters or less and explain *why*, not *what*.

## Pre-commit hooks

The repository uses pre-commit hooks that run automatically on each commit:

- Trailing whitespace removal
- End-of-file fixer
- YAML linting
- Rust formatting (`just format`)
- Rust linting (`just lint`)
- Schema validation on corpus files (`just validate`)

Install them after cloning:

```bash
pre-commit install
```

Do not bypass hooks with `--no-verify`. If a hook fails, fix the underlying problem.

## Pull request process

1. Create a feature branch and push your changes
2. Open a PR - CI runs all relevant checks automatically
3. A preview deployment is created (see [Deployment](./deployment))
4. Get a code review
5. Merge to main - production deploys automatically

## Code review

Reviewers check for:
- Legal faithfulness - does the `machine_readable` section match the law text?
- Cross-law reference correctness - do `source` blocks point to the right regulations and outputs?
- Test coverage - are there BDD scenarios, especially for edge cases?
- Schema compliance - does `just validate` pass?

## Design decisions (RFCs)

Changes to the law format, engine architecture, or cross-cutting patterns require an RFC. See the [RFC process](/rfcs/rfc-000) for details.

Use the template at `docs/rfcs/template.md` to draft your RFC, then open a PR for discussion.

## Further reading

- [Getting Started](/guide/getting-started) - set up your development environment
- [Testing](/guide/testing) - how to write and run tests
- [Adding a Law](./adding-a-law) - step-by-step guide for new laws
