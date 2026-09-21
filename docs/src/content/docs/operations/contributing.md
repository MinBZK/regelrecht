---
title: "Contributing"
description: "The branching model, quality checks, and workflow for contributing to RegelRecht."
---

RegelRecht is open source and welcomes contributions. The workflow is below.

## Branching model

Short-lived branches off `main`, merged back into `main`. There is no `develop`, `release/*` or `hotfix/*` branch:

- `main` - production, always deployable
- `feat/*` - new features
- `fix/*` - bug fixes
- `docs/*` - documentation changes

Name the branch after the Conventional Commits type its work carries, so `feat/` rather than `feature/`.

Create your branch from `main`, open a PR back to `main`.

## Commit conventions

Commits follow [Conventional Commits](https://www.conventionalcommits.org/):

```
type(scope): description
```

Types: `feat`, `fix`, `docs`, `style`, `test`, `chore`, `refactor`, `perf`, `build`, `ci`

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
3. End the PR body with a `Werkpakket:` line (see below). A required check blocks the merge without it
4. Add the `deploy:preview` label if reviewers need a running preview (see [Deployment](./deployment))
5. Get a code review, and clear any finding the automated review marks Critical
6. Merge to main - production deploys automatically

The PR title follows the same Conventional Commits shape as a commit, and it is linted. The scope, when present, comes from a fixed list (`engine`, `corpus`, `editor`, `docs`, `ci` and a handful more). The subject must start with a lowercase letter, which is the rule most titles trip on: `docs: RFC-016 toelichten` fails on the capital R, `docs: verwijzing naar RFC-016 toelichten` passes. Editing the title re-runs the check, with no new commit needed.

### The `Werkpakket:` line

Every PR names the werkpakket from the [roadmap](/roadmap) that the work contributes to, as a trailer on its own line at the end of the body:

```
Werkpakket: referentie-casus-i
```

The slug is the werkpakket's `id`, which is also its filename and its URL. Several are comma-separated. Work that genuinely belongs to no werkpakket says so with a reason, because a box that fills itself measures nothing:

```
Werkpakket: geen - losse typefout in de docs
```

Write the bare slug. A bot rewrites the line into a link to the roadmap after the check passes, so the reference is clickable where people read it. Dependabot and fork PRs are exempt.

When the PR touches a law from the corpus, add a `Wet:` line naming the law's `$id`. It is optional, because most PRs touch no law, but if present it has to resolve against the corpus.

```
Werkpakket: referentie-casus-i
Wet: wet_op_de_zorgtoeslag
```

## Code review

Reviewers check for:
- Legal faithfulness - does the `machine_readable` section match the law text?
- Cross-law reference correctness - do `source` blocks point to the right regulations and outputs?
- Test coverage - are there BDD scenarios, especially for edge cases?
- Schema compliance - does `just validate` pass?

## Documentation URLs

A published address stays reachable. A docs page gets cited in an RFC, pasted
into an issue, bookmarked, and mailed to people outside this repository, so
renaming it is fine but letting the old address disappear is not. The cost of a
dead link lands on a reader who cannot know where the page went.

Renaming a page is therefore two steps. Move the file, then add the old path to
`redirects` in `docs/astro.config.mjs`:

```js
redirects: {
  '/concepts/untranslatables': '/concepts/markings',
},
```

CI compares the routes this branch builds against the routes the base branch
builds, so a page that moves without a redirect fails the docs gate and the
failure names the line that fixes it. The same check rejects a redirect whose
target does not exist.

If a page is removed rather than moved, redirect it to whatever now covers the
subject.

## Design decisions (RFCs)

Changes to the law format, engine architecture, or cross-cutting patterns require an RFC. See the [RFC process](/rfcs/rfc-000) for details.

Use the template at `docs/src/content/rfcs/template.md` to draft your RFC, then open a PR for discussion.

## Further reading

- [Getting Started](/guide/getting-started) - set up your development environment
- [Testing](/guide/testing) - how to write and run tests
- [Adding a Law](./adding-a-law) - step-by-step guide for new laws
