---
node: crate:github
fingerprint: 01c47f31809b4e26
---
**Wat.** Een gedeelde GitHub REST-client: één `GithubClient`-service voor alle
regelrecht-applicaties.

**Waarom.** Bewust standalone gehouden (hangt van geen andere workspace-crate
af) zodat de crates die erop consolideerden — corpus en editor-api — hem kunnen
gebruiken zonder een afhankelijkheidscycle te introduceren. Eén client betekent
één plek voor rate-limiting, auth en foutafhandeling tegen de GitHub-API.
