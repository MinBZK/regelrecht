# Geplande routine — {naam}

*Een terugkerende, **onbeheerde** taak via `/schedule` (cloud-routine). Draait door als
de terminal dicht is, op een cron-schema, **zonder permission-prompts**. Min. interval
1 uur. Voor sessie-gebonden batchwerk: gebruik `/loop` (zie `loop-prompt.md`).*

## ⚠ Veiligheid: committen mag, maar alleen naar private repos

Routines draaien autonoom zónder bevestiging. Committen + pushen is toegestaan, mits de
push-target **aantoonbaar private** is. Gebruik de fail-closed guard
`scripts/assert-private-repo.sh` als **preflight vóór elke push**:

```bash
# in de routine, vóór publiceren:
bash {pad-naar-skill}/scripts/assert-private-repo.sh && git push
# of expliciet per remote:  assert-private-repo.sh upstream && git push upstream HEAD
```

De guard vraagt de GitHub-zichtbaarheid op en laat alleen `PRIVATE` door (PUBLIC en
INTERNAL → block). Hij faalt **closed**: geen repo / geen remote / geen `gh` / niet te
bepalen → push geweigerd. Lokale commits zijn altijd veilig (geen blootstelling); de
guard bewaakt het *publiceren*.

> **Echte afdwinging.** Een routine volgt deze invariant uit discipline. Voor
> harness-afdwinging (zodat ook een ontspoorde autonome run niet naar een publieke repo
> kan pushen) is een PreToolUse-hook op `git push` nodig die de guard draait — zie de
> noot onder "Beheer". Zonder hook leunt het op de routine-instructies.

## Wanneer een routine i.p.v. een loop

| Situatie | Gebruik |
|---|---|
| Afgebakende batch die zichzelf afmaakt, jij erbij | `/loop` self-paced |
| Wachten op een externe run binnen je sessie | `/loop <interval>` |
| Terugkerend toezicht dat terminal-sluiting overleeft | **routine (`/schedule`)** |
| Eén run op een toekomstig moment | éénmalig: `/schedule tomorrow at 9am <prompt>` |

## Voorbeeld-routines voor desk-review

| Naam | Cron | Doel (report-only) |
|---|---|---|
| Nachtelijke corpus-validatie | `0 2 * * *` | Valideer hele corpus tegen schema + engine; schrijf regressies naar een status-log + meld afwijkingen t.o.v. gisteren |
| Wekelijkse coverage-sweep | `0 6 * * 1` | MR-coverage + untranslatables-coverage per wet; werk `corpus-status` bij en concept-eindrapport |
| Bron-watch | (API/GitHub-trigger via web-UI) | Bij nieuwe publicatie van een wet in scope: signaleer dat een harvest nodig is |

## Routine-prompt

```
{Beschrijf de taak voor de routine. Begin met de scope + het corpus-pad. Sluit af met de
invariant: "Vóór elke push: draai `bash {pad}/scripts/assert-private-repo.sh` en push
alleen als die slaagt; bij block commit je lokaal en rapporteer je dat niet gepusht is.
Push nooit naar een publieke repo."}
```

## Beheer

- `/schedule list` — overzicht · `/schedule update` — wijzigen · `/schedule run` — nu draaien
- Web-UI: claude.ai/code/routines (triggers, omgeving, connectors)

### Harness-afdwinging (lokaal instellen)

Installeer een globale PreToolUse-hook in `~/.claude/settings.json`
(`~/.claude/hooks/git-push-private-guard.sh`) die **elke** `git push` blokkeert tenzij de
target een private GitHub-repo is. Deze hook is **machine-lokaal** en reist niet mee met
de repo — registreer 'm zelf in je eigen `~/.claude/`. Escape-hatch voor een bewuste
publieke push: `ALLOW_PUBLIC_PUSH=1 git push`.

> ⚠ **Lokaal vs cloud.** Deze hook beschermt **lokale** sessies en `/loop`-runs (die op
> jouw machine draaien). `/schedule`-routines draaien in een **aparte cloud-omgeving** en
> gebruiken deze lokale hook niet noodzakelijk. Voor cloud-routines geldt daarom:
> - laat de routine-prompt de guard expliciet aanroepen vóór elke push, **en**
> - zorg dat de guard (of een equivalent) in die omgeving beschikbaar is, **en**
> - houd commit-routines bij voorkeur report-only of push naar een aantoonbaar private
>   remote.
> De fail-closed guard in de routine-prompt is dus de primaire beveiliging voor cloud;
> de hook is de extra net voor lokaal werk.

## Cron-spiekbriefje

`min uur dag-vd-maand maand dag-vd-week` · `*/15 * * * *` elke 15 min ·
`0 9 * * *` dagelijks 09:00 · `0 6 * * 1` maandag 06:00 · `0 0 1 * *` 1e v/d maand.
