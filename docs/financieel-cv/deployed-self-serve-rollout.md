# Rollout — deployed self-serve editor voor jurist/beleidsmedewerkers

Doel: jurist + beleidsmedewerkers loggen met hun **eigen SSO-account** in op
een bereikbare editor en werken zelf aan de **Financieel-CV-corpus**, met
commits onder hun eigen naam. Gekozen route: corpus op een **branch van de
centrale `MinBZK/regelrecht-corpus`**, eerst op een **PR-preview** (`prN`),
later productie.

> Achtergrond en volledige uitleg van het traject-model:
> `docs/src/content/docs/operations/private-repo-trajects.md` en
> `docs/src/content/docs/auth-and-roles.md`. Dit is de dossier-specifieke
> invulling.

## Het kernmechanisme

Een **traject** legt de editor bovenop een GitHub-repo/branch. Deelnemers
worden per e-mail uitgenodigd, loggen in via SSO, en hun saves committen
onder hun eigen naam op de traject-branch (afgeleid van de base-branch).
Tokens leven alleen in de runtime van het deployment.

## Belangrijk: welke auth-ref / env-var

De centrale repo kent twee paden bij traject-aanmaak, met **verschillende**
env-vars:

| Pad | Base-branch | Auth-ref | Env var |
|---|---|---|---|
| Toggle **UIT** (central default) | vast (`development`) | `minbzk-central` | `CORPUS_AUTH_MINBZK_CENTRAL_TOKEN` |
| Toggle **AAN**, coords = `MinBZK/regelrecht-corpus` | **eigen branch** (FCV) | `minbzk-regelrecht-corpus` (afgeleid) | `CORPUS_AUTH_MINBZK_REGELRECHT_CORPUS_TOKEN` |

Omdat we de FCV-corpus op een **eigen branch** willen (niet in
`development` mergen), gebruiken we de **toggle-AAN** variant → env var
**`CORPUS_AUTH_MINBZK_REGELRECHT_CORPUS_TOKEN`**. Deze is *anders* dan de
bestaande centrale token en moet apart worden gezet.

## Stappen (met wie)

### 1. Corpus op een branch van MinBZK/regelrecht-corpus — jij
De FCV-laws staan nu in dit app-repo onder `corpus/regulation/nl/…`. In de
corpus-repo leven ze onder `regulation/nl/…`. Zet ze op een nieuwe branch
(bv. `financieel-cv`) afgeleid van `development`, zodat `development`
onaangeroerd blijft (lage blast radius).

> Dit is de enige naar-buiten-gerichte, onomkeerbare stap. Vereist push-
> recht op de corpus-repo. Gebeurt pas na expliciete go-ahead.

### 2. Fine-grained PAT op MinBZK/regelrecht-corpus — repo-admin
- Repository access: **Only select repositories** → `MinBZK/regelrecht-corpus`
- Permissions: `Contents` **RW**, `Pull requests` **RW**, `Metadata` **R**
- Expiration: conform security-beleid (bij verloop weigert de editor saves)

### 3. Env var + herstart op de PR-preview — operator
- Zet **`CORPUS_AUTH_MINBZK_REGELRECHT_CORPUS_TOKEN`** = de PAT-waarde op het
  `prN`-editor-deployment.
- Controleer dat **`CORPUS_AUTH_MINBZK_CENTRAL_TOKEN`** ook aanwezig is (het
  default leespad van de editor gebruikt die). Bij `clone-from: regelrecht`
  komt die automatisch mee van productie.
- Herstart de editor-pod zodat de nieuwe var wordt opgepikt.

### 4. PR-preview deploy — jij (via CI)
- Push branch `feat/financieel_cv_RVO` en open/refresh de PR → CI bouwt de
  `prN`-editor en deployt naar ZAD. Noteer de preview-URL.

### 5. Rollen in Keycloak — Keycloak-admin
- Ken elke jurist/beleidsmedewerker **`editor-writer`** toe (of `editor-reader`
  voor alleen bekijken + notities). Werkt pas **bij de volgende login**.
- Zorg dat hun **`email_verified`** claim `true` is — anders weigert de editor
  saves (403 → opnieuw inloggen).

### 6. Traject aanmaken + uitnodigen — jij (traject-eigenaar)
In de `prN`-editor → **Nieuw traject**:
- Naam / beschrijving / scope: bv. "Financieel CV — juristvalidatie"
- **"Eigen GitHub-repo gebruiken"** = AAN
- `repo_owner` = `MinBZK`
- `repo_name` = `regelrecht-corpus`
- `base_branch` = `financieel-cv` (de branch uit stap 1)
- `repo_path` = `regulation/nl`
- Aanmaken → de pre-flight check valideert repo, push-recht en base-branch.
- Nodig de deelnemers uit **per e-mail** (hun SSO-e-mail).

### 7. Verifiëren op preview — jij + 1 deelnemer
Vóór je iedereen loslaat: laat één deelnemer inloggen, een wet openen, een
triviale save doen, en controleer dat de commit op de traject-branch landt
onder hun eigen naam. Pas daarna opschalen (en later herhalen op productie).

## Aandachtspunten

- **PR-preview is tijdelijk** — verdwijnt als de PR sluit. Voor duurzaam
  zelfstandig doorwerken herhaal je stap 3/5/6 op **productie**.
- **Tijdlijn.** Dit spant jou + operator + repo-admin + Keycloak-admin samen;
  realistisch niet volledig live donderdag. Overweeg de begeleid-lokale opzet
  (`kickoff-flow.md`, corpus-registry naar de echte corpus) als vangnet voor
  de sessie zelf.
- **Attributie.** Alle deelnemers committen via dezelfde repo-PAT, maar onder
  hun eigen naam (Author + `Co-authored-by`). Voor harde per-user audit: de
  editor-API audit-logs of de PR-commitgraaf.
