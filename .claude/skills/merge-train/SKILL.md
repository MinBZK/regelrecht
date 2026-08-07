---
name: merge-train
description: Mergt één pull request of een reeks achter elkaar op main, met bijwerken zonder handtekeningverlies, wachten op de juiste checks, en verifiëren dat main groen blijft. Gebruik dit bij "merge PR N", "laat de trein rijden" of het wegwerken van een stapel mergebare PR's.
user-invocable: true
allowed-tools: Bash, Read, Grep, Glob
---

# Merge train

Mergt pull requests op `main` binnen de regels die deze repo stelt: trunk-based,
`strict: true`, squash-merge, ondertekende commits.

Twee ingangen, dezelfde lus:

- `merge-train <nummer>`: één pull request.
- `merge-train`: alle mergebare PR's, één voor één, met een verse `main` per ronde.

## Waarom dit een lus is en geen lijstje

`strict: true` betekent dat een PR bij moet zijn met `main` voordat hij mag
mergen. Elke merge zet daarmee elke andere openstaande PR op achterstand. Je kunt
dus niet vooraf bepalen welke PR's mergebaar zijn en die lijst afwerken: na
iedere merge is de vraag opnieuw.

Werk daarom altijd één PR tegelijk af en begin elke ronde bij een verse
`origin/main`. Bundel nooit meerdere PR's in één branch om cycli te sparen. Dat
is eerder geprobeerd (PR 1205): één rode test legde de hele bundel stil en de
attributie per PR verdween in de squash.

## Vaste regels

**Handtekeningen.** `required_signatures` staat aan. Zet nooit `user.name`,
`user.email` of een `GIT_AUTHOR_*`/`GIT_COMMITTER_*`-variabele; git haalt die uit
`~/.gitconfig` en een ander adres maakt de handtekening onherleidbaar, waardoor
de merge blokkeert.

**Bijwerken doe je met de merge-variant, niet met rebase.**

```bash
gh api --method PUT repos/MinBZK/regelrecht/pulls/<nr>/update-branch
```

Die zet er een merge-commit bovenop die GitHub zelf tekent, dus de branch blijft
`verified`. `gh pr update-branch --rebase` schrijft de commits opnieuw en tekent
het resultaat níét; een ondertekende branch wordt dan `unsigned` en strandt op
`required_signatures`. Squash-merge platst die merge-commits alsnog, dus `main`
houdt een vlakke historie.

**Documenten die een mens zelf beoordeelt gaan nooit automatisch mee.** RFC's,
rapporten en substantieel proza onder `docs/src/content/rfcs/` sla je over, ook
als ze groen zijn. Meld ze aan het eind als klaar en wachtend op een oordeel.

**Blijf van andermans werk af.** Fork-PR's en PR's van externe bijdragers merge
je niet; die liggen bij hun auteur.

## Lus

### 1. Verse main en de kandidatenlijst

```bash
git fetch origin main
gh pr list -R MinBZK/regelrecht --state open --limit 200 \
  --json number,title,isDraft,mergeStateStatus,author,labels
```

Kandidaat is een PR die niet in draft staat, van `ehotting` of een teamlid is,
geen RFC of rapport wijzigt, en `mergeStateStatus` heeft van `CLEAN`, `BEHIND` of
`BLOCKED`. `DIRTY` betekent een echt conflict: overslaan en melden.

Kies er één. Bij gelijke geschiktheid: de kleinste diff eerst, want die is het
snelst door de poort en zet de rest het minst op achterstand.

### 2. Bijwerken als hij achterloopt

```bash
gh api --method PUT repos/MinBZK/regelrecht/pulls/<nr>/update-branch
```

Wacht daarna tot de nieuwe checks starten. Werkt de aanroep niet (conflict), dan
is het een `DIRTY`-geval: overslaan en melden.

### 3. Wachten op de juiste checks

Verplicht op `main` zijn:

```
Pre-commit, WASM Build, Protect schema versions, Security Audit,
Test, Validate PR title, Claude review completed
```

`Test` is een verzamelpoort: daarachter hangen `rust-tests`, `frontend-tests`,
`e2e`, `cross-law-integrity`, `provenance-checks`, `docs-a11y`, `rust-image` en
`changes`. Je hoeft die niet apart af te wachten; groen op `Test` dekt ze.

Wacht op precies deze zeven en op niets anders.

- `Build and Deploy` is `skipped` zonder het `preview`-label. Dat is geen fout.
- `CodeQL` en `Analyze (…)` rapporteren niet op een PR die alleen docs raakt.
  Nooit op wachten; ze zijn niet verplicht.
- `Mutation Testing (diff)` draait alleen bij een wijziging in
  `packages/engine/**`.

Reken op ongeveer zes minuten voor CI en tien voor de review; onder belasting
loopt CI op tot een kwartier. Poll rustig, niet elke tien seconden.

### 4. Mergen

```bash
gh pr merge <nr> -R MinBZK/regelrecht --squash --delete-branch
```

Squash is de enige toegestane methode in deze repo. De titel is de commit-titel,
dus die moet aan Conventional Commits voldoen; `Validate PR title` bewaakt dat al.

### 5. Main controleren voor je doorgaat

Na de merge draait CI opnieuw op `main`, en `deploy-production` hangt daaraan
vast. Wacht tot de CI-run van de nieuwe `main`-commit klaar is en groen.

Is hij rood, **stop dan de trein** en meld het. Doorgaan met de volgende PR
bovenop een kapotte `main` maakt de oorzaak alleen moeilijker terug te vinden.

### 6. Terug naar stap 1

Verse `origin/main` ophalen en opnieuw beginnen. Verifieer voor elke volgende
merge dat de gekozen PR `behind_by = 0` heeft; anders eerst stap 2.

## Wanneer je stopt

- `main` is rood na een merge.
- Een PR heeft een echt conflict (`DIRTY`).
- Een required check faalt inhoudelijk. Repareren hoort niet bij deze skill;
  meld welke check op welke PR en wat het log zegt.
- De review meldt iets kritieks. Bevindingen beoordelen is mensenwerk.

## Wat je aan het eind meldt

Per PR één regel: nummer, titel, en wat ermee gebeurd is. Daarachter de
overgeslagen PR's met de reden. Geen samenvatting van de inhoud van wat er
gemerged is; dat staat in de PR's zelf.

## Bekende ruis

Een gefaalde `deploy-preview` met `"status": "superseded"` in de JSON betekent
dat ZAD de taak opzij heeft gezet voor een nieuwere taak op hetzelfde deployment.
Het werk is dan gedaan; alleen die job opnieuw draaien volstaat.

Twee check-runs met dezelfde naam op één commit kan: twee PR's kunnen dezelfde
head-SHA delen. De nieuwste telt.
