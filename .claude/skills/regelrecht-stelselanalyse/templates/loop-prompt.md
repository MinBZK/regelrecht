# Completion-loop — {taak}

*Een self-driving prompt voor een afgebakende, repetitieve batch (bv. een werklijst
wet-voor-wet harvesten/migreren/reviewen tot alles klaar is). Alleen voor repetitief,
goed-afgebakend werk — niet voor verkennend/oordeels-werk.*

## Hoe starten

Kies de modus (zie `references/cycle-workflow.md` → loop vs schedule):

- **Batch tot klaar (aanbevolen)** — self-paced `/loop`, Claude bepaalt zelf de cadans
  en stopt zodra de stop-conditie is bereikt. Plak alles onder "PROMPT" achter:
  ```
  /loop <PROMPT>
  ```
- **Vaste polling** — als je op een externe run wacht (validatie/CI): `/loop 10m <PROMPT>`.
- **Onbeheerd / terugkerend** — overleeft terminal-sluiting: gebruik géén `/loop` maar
  een routine, zie `templates/scheduled-routine.md`.

Self-paced loops zijn sessie-gebonden (stoppen als de terminal sluit) en lopen max 7
dagen. Druk `Esc` om te stoppen; `--resume`/`--continue` herstelt een niet-verlopen loop.

---

## PROMPT (alles hieronder is de loop-prompt)

Je werkt aan **{taak}** voor het corpus `{pad}`, cyclus-scope **{thema}**. Werk één
item per iteratie af tot de stop-conditie is bereikt.

**Stop-conditie**: {expliciet en controleerbaar — bv. alle items in de werklijst staan
op `done`}. Als dit waar is: rapporteer de eindstand en **plan geen volgende wake** (de
loop is klaar).

**Per iteratie**:
1. Lees het voortgangslog (onderaan dit document `{pad}`) + de werklijst; bepaal het
   eerstvolgende niet-`done`-item.
2. Doe het werk voor dat item ({concreet: harvest / migreer / breid MR uit / review-as}).
   Houd je aan de regelrecht-methode en classificeer elke bevinding **4-weg**
   (modellering-fout / wetgevings-fout / engine-limitatie / acceptabele untranslatable).
3. Valideer: `{validatie-commando}` groen + `{scenario/BDD-commando}` groen. Rood? →
   herstel binnen deze iteratie of markeer het item `geblokkeerd` met reden.
4. Commit met een beschrijvende message (geen branding, nooit `--no-verify`).
5. Werk de werklijst bij (`done`/`geblokkeerd`) en schrijf één regel in het voortgangslog:
   item + uitkomst + eventuele discovery + classificatie.
6. Bepaal de wake-cadans:
   - werk-iteratie die echt werk deed → ga direct door (korte/geen wachttijd);
   - wachtend op een externe run → poll rond ~270s (cache blijft warm);
   - niets te doen maar nog niet klaar → fallback ~1200s.

**Invarianten (nooit schenden)**:
- Schema blijft valide; nooit rode tests pushen.
- Blijf binnen scope **{thema}**; nieuwe scope → noteren in het log, niet zelf oppakken.
- **Niets pushen zonder toestemming** (de loop draait lokaal met sessie-permissies).
- Bij twijfel over een interpretatie → markeer `NEEDS_HUMAN_REVIEW` en ga door; forceer
  geen oordeel.

---

## Werklijst

| # | Item | Status |
|---|---|---|
| 1 | {item} | open |

## Voortgangslog

*(de loop vult dit per iteratie aan)*

- **Iteratie 1** — {item}: {uitkomst}. Classificatie: {—}. Discovery: {—}.
