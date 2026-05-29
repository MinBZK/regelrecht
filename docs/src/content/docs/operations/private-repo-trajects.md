---
title: "Private-repo trajects"
---

Vanaf [PR #704](https://github.com/MinBZK/regelrecht/pull/704) kan een traject in de editor gekoppeld worden aan een **eigen GitHub-repo** in plaats van de centrale `MinBZK/regelrecht-corpus`. Handig voor organisaties of teams die hun regelgeving in een private repo willen beheren, met behoud van RegelRecht's editor- en attributie-eigenschappen.

Deze pagina beschrijft de end-to-end flow: wat je als eindgebruiker, traject-eigenaar en operator moet doen.

## Hoe het werkt op hoofdlijnen

- Bij het aanmaken van een traject kies je **of** de centrale MinBZK-repo (default), **of** een eigen `owner/repo` + base branch.
- Authenticatie naar de eigen repo gebeurt met een **GitHub Personal Access Token** (fine-grained) die door de **operator van het RegelRecht-deployment** als environment variable wordt geconfigureerd. Tokens leven dus niet in de database en niet in de browser — alleen in de runtime-omgeving van de editor.
- Commits in de repo verschijnen onder **de naam en het email-adres van de echte gebruiker** (afkomstig uit de OIDC-sessie, mits geverifieerd door de IdP). Niet onder een service-account.

## Wie doet wat

| Rol | Actie |
|---|---|
| Repo-eigenaar (of admin) | Maakt een GitHub-PAT met schrijfrechten op de target-repo |
| Operator | Configureert de PAT als env var op het editor-deployment (eenmalig per repo) |
| Traject-eigenaar | Maakt het traject aan in de editor met de eigen repo-velden |
| Deelnemers | Loggen in via SSO; saves verschijnen automatisch onder hun eigen naam |

## Stap 1 — Bereid de GitHub-repo voor

De target-repo moet aan twee voorwaarden voldoen vóór je 'm kunt koppelen:

1. **De `base branch` bestaat al** (met minstens één commit). De editor maakt een eigen feature-branch áf van deze base — als de base nog niet bestaat (bv. een leeg gestelde `main`), faalt de validatie. Maak desnoods een lege README-commit op `main` om de branch te initialiseren.
2. **Het PAT-account heeft push-rechten op de repo**. Voor private repo's: de PAT moet voor die specifieke repo aangevraagd zijn met de juiste scopes (zie stap 2).

> De repo mag publiek of private zijn. Voor publieke repos gelden GitHub's rate-limits van het tokenless API-endpoint zonder PAT — voor private repo's is een PAT verplicht voor zowel reads als writes.

## Stap 2 — Maak een GitHub PAT aan

Genereer een **fine-grained PAT** op [github.com/settings/personal-access-tokens/new](https://github.com/settings/personal-access-tokens/new):

- **Resource owner**: de owner van de target-repo (persoonlijk account of organisatie)
- **Repository access**: "Only select repositories" → kies precies de target-repo (níét "all repositories", houd de blast radius klein)
- **Repository permissions**:
  - `Contents`: **Read and write** — nodig voor commits + branch-creatie
  - `Pull requests`: **Read and write** — nodig om de session-PR te openen en bij te werken
  - `Metadata`: **Read** (verplicht en auto-geselecteerd)
- **Expiration**: kies een termijn die past bij je security-beleid. PAT's verlopen — bij verloop weigert de editor nieuwe saves en moet de operator de env var verversen.

Sla de waarde van de PAT direct op een veilige plek op (GitHub toont 'm maar één keer).

> Classic PATs werken ook, maar geven veel te brede toegang (alle repo's waar je toegang toe hebt). Fine-grained is sterk aanbevolen.

## Stap 3 — Operator configureert de env var

Geef je operator deze drie dingen:

1. De `owner/repo` van de target-repo (bv. `acme/regelrecht-lokaal`)
2. De gegenereerde PAT-waarde
3. Een korte beschrijving van het doel (voor audit-doeleinden)

De operator zet de PAT als environment variable op het editor-deployment met de naam:

```
CORPUS_AUTH_<OWNER>_<REPO>_TOKEN
```

waarbij `<OWNER>_<REPO>` een **deterministische slug** is van de coordinates: lowercase, alle niet-alfanumerieke tekens vervangen door `-`, vervolgens hoofdletters en dashes naar underscores. Voorbeelden:

| owner/repo | env var |
|---|---|
| `MinBZK/regelrecht-corpus` | `CORPUS_AUTH_MINBZK_REGELRECHT_CORPUS_TOKEN` |
| `acme/regels` | `CORPUS_AUTH_ACME_REGELS_TOKEN` |
| `tdjager/regelrecht-private-test` | `CORPUS_AUTH_TDJAGER_REGELRECHT_PRIVATE_TEST_TOKEN` |

De operator weet hoe ze env vars op het cluster moeten zetten — dat is omgevings-specifiek (ZAD, Kubernetes, docker-compose). Na de wijziging moet de editor-pod herstart worden zodat de nieuwe var wordt opgepikt. **Eén env var per repo** — als meerdere trajects naar dezelfde repo wijzen, is één configuratie genoeg.

> Wanneer je het traject probeert aan te maken vóórdat de env var bestaat, krijg je een nette foutmelding die exact de verwachte env-var-naam toont. Geef die naam letterlijk door aan je operator.

## Stap 4 — Maak het traject aan

In de editor:

1. Klik **"Nieuw traject"** in het traject-menu.
2. Vul **naam**, **beschrijving** en **scope** in.
3. Zet de schakelaar **"Eigen GitHub-repo gebruiken"** aan.
4. Vul `repo_owner`, `repo_name` en `base_branch` in. Eventueel `repo_path` als je YAML-bestanden in een subdirectory zitten (default: repo root).
5. Klik **"Aanmaken"**.

De editor doet vóór het aanmaken een **pre-flight check** tegen de GitHub-API:
- Bestaat de repo en is 'ie zichtbaar voor de geconfigureerde token?
- Heeft de token push-rechten?
- Bestaat de opgegeven base-branch?

Faalt één van deze checks, dan zie je een gerichte foutmelding (zie [foutmeldingen](#foutmeldingen-en-wat-ze-betekenen) hieronder). Geen rij in de database — je kunt direct opnieuw proberen na correctie.

## Hoe commit-attributie werkt

Elke save in een traject produceert een commit op de session-branch met:

- **Author**: `<jouw OIDC-naam> <jouw OIDC email>` (uit Keycloak)
- **Committer**: het service-account dat het deployment runt (dit is wat GitHub als "pushed by" registreert in de Push Events)
- **`Co-authored-by:` trailer**: identiek aan de author — dubbele credit zodat GitHub de commit ook in jouw Contributions-graph laat zien

Voor de UI-weergave op GitHub geldt: als jouw OIDC-email een verified email is op je GitHub-account, dan toont GitHub jouw avatar en naam bij elke commit, en linkt 'ie naar je profile. Geen mapping verified? Dan wordt de email als string getoond, zonder profile-link, maar nog steeds met de juiste naam.

> De editor weigert saves wanneer Keycloak's `email_verified` claim niet `true` is. Dit voorkomt dat een gebruiker met een ongeverifieerd email-claim onder die naam zou kunnen committen. Krijg je hiervan een 403, log dan opnieuw in (refresht de claim) of vraag je beheerder om je account-instellingen te controleren.

## Foutmeldingen en wat ze betekenen

Wanneer je traject-create faalt, toont de UI een specifieke melding. De belangrijkste:

| Statuscode | Melding | Wat je moet doen |
|---|---|---|
| 503 | "deze repo is nog niet door je beheerder geconfigureerd (verwacht env var X)" | Vraag de operator de getoonde env-var-naam te configureren |
| 502 | "het token van je beheerder wordt door GitHub geweigerd" | PAT is verlopen of ongeldig — vraag operator om verversing |
| 403 | "het geconfigureerde token heeft geen schrijftoegang tot deze repo" | PAT mist `Contents: write` of de PAT-account is geen collaborator op de repo |
| 404 | "repo X bestaat niet of het token kan 'm niet zien" | Tikfout in owner/repo, of de fine-grained PAT is niet aan deze repo gekoppeld |
| 404 | "branch 'X' bestaat niet op owner/repo" | De `base_branch` bestaat nog niet op de repo — initialiseer 'm met minstens één commit |
| 400 | "alleen letters, cijfers, en '-', '_', '.'" | owner, repo of subpath bevat ongeldige karakters |
| 400 | "moeten alle drie worden meegegeven" | Eigen-repo-toggle staat aan maar één van owner/repo/base_branch is leeg |
| 502 | "onverwacht antwoord van GitHub bij repo-validatie" | Tijdelijke GitHub-issue; probeer opnieuw |

Tijdens het bewerken kunnen ook saves falen. De meeste meldingen zijn vergelijkbaar; bij een 403 over "geverifieerd e-mailadres" is opnieuw inloggen meestal de oplossing.

## Beperkingen en aandachtspunten

- **Per-repo PAT, niet per-user**. Alle deelnemers aan het traject committen via dezelfde token, maar onder hun eigen naam (via Author/Co-authored-by). GitHub Push Events tonen altijd het service-account als "pushed by" — voor harde per-user audit is dat niet voldoende; gebruik daarvoor de editor-API's audit-logs of de PR's commit-graaf.
- **PAT-verloop is operator-werk**. De editor weigert reads/writes zodra de PAT door GitHub geweigerd wordt; de operator moet 'm dan verversen. Plan dit in.
- **Geen self-service**. Voor elke nieuwe repo moet een operator een env var configureren. Voor occasioneel gebruik is dat prima; voor schaalbare zelfbediening is een **GitHub App** een betere richting (geparkeerd voor latere fase).
- **Geen tokens in DB of browser**. Bewust ontwerp: een bug, breach of insider met DB-toegang kan geen tokens exfiltreren. Wel betekent dit dat tokens niet "even snel" zelf zijn in te stellen.

## Rollout-aandachtspunten (alleen relevant bij de eerste deploy)

De eerste deploy met deze feature scherpt twee oude paden aan; controleer onderstaande punten op je deployment vóór je de release uitrolt.

- **De writable-own source gebruikt voortaan strikte token-resolutie** (geen `CORPUS_GIT_TOKEN`-fallback). Bestaande trajects die via de central MinBZK-repo committen, hebben dus `CORPUS_AUTH_MINBZK_CENTRAL_TOKEN` nodig als losse env var. Deployments die tot nu toe leunden op alleen `CORPUS_GIT_TOKEN` voor het centrale schrijfpad, zien stille push-failures na de release als die env var ontbreekt. Zet 'm vóór deploy en check de editor-logs op de eerste run; de diagnostic-log toont expliciet de verwachte env-var-naam wanneer de resolver `None` returnt voor de writable-own source.
- **Bestaande SSO-sessies missen de nieuwe `email_verified`-claim**. Eerste save na deploy levert dan een 403 op met de melding "log opnieuw in". Geen onderhoud, geen migratie — gewoon eenmalig opnieuw inloggen lost het op.
