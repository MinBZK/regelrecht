# Kick-off flow — regelhulp Financieel CV-team

Logische volgorde om in ~75 minuten alles door te lopen. Aanrader: open
deze pagina links en het genoemde bestand rechts. Linkjes zijn relatief
zodat ze in elke editor (VSCode, Obsidian, GitHub) clickable zijn.

---

## Voorbereiding (15 min, vóór de workshop)

Stack laten draaien zodat je in deel 4 niets meer hoeft te starten.

```bash
# Token uit keychain + dev stack omhoog (postgres + admin + editor)
export GITHUB_TOKEN=$(security find-generic-password -a "$USER" -s github-packages-read -w)
just dev

# Editor-api ipv admin op :8000 (admin heeft niet alle endpoints)
pkill -f regelrecht-admin
set -a && source .env && set +a
cd packages && nohup target/debug/regelrecht-editor-api > /tmp/editor-api.log 2>&1 &
cd ..
```

Open in browser: <http://localhost:3000> — hard reload als je nog
gecached materiaal ziet.

Open in editor (1 keer per tabblad):

- [README.md](README.md) — projectoverzicht
- [stelsel-overview.md](stelsel-overview.md) — deel 1
- [financieel-cv-stelsel.png](financieel-cv-stelsel.png) — deel 1
- [financieel-cv-graph-detail-alle-7.png](financieel-cv-graph-detail-alle-7.png) — deel 2
- [financieel-cv-graph-detail.png](financieel-cv-graph-detail.png) — deel 2 (zoom)
- [output-walkthrough.md](output-walkthrough.md) — deel 3
- [mvt-referenties.md](mvt-referenties.md) — deel 5

---

## Deel 1 — Het stelsel (15 min)

**Doel:** gedeeld beeld krijgen over welke 6 wetten + AWB de regelhulp
raken en welke 3 uitvoeringsorganisaties die uitvoeren.

1. Open [stelsel-overview.md](stelsel-overview.md) en scroll naar het
   diagram.
2. Toon [financieel-cv-stelsel.png](financieel-cv-stelsel.png) op
   groot scherm.
3. Loop kort de drie kolommen langs: **WET → lagere regelgeving →
   uitvoering**.
4. Wijs op AWB als overkoepelende procesrechtelijke schil.
5. Lees voor: het tabelletje "Welke regeling op welk niveau" in
   `stelsel-overview.md`.

**Discussie-vraag:** "Klopt deze indeling met hoe jullie in het
ontwerp van de regelhulp omgaan met deze wetten?"

---

## Deel 2 — De grafiek (10 min)

**Doel:** laten zien dat de cross-law structuur van de 8 regelingen
machine-bekende informatie is, niet alleen documentatie.

1. Toon [financieel-cv-graph-detail-alle-7.png](financieel-cv-graph-detail-alle-7.png).
2. Wijs op kleurcodering: blauw=parameters, blauw=conditions,
   rood=outputs, geel=untranslatables.
3. Loop NRP, LIV, LKV, LKS, LDP, JC/WPA, PP linksboven naar rechtsonder
   langs — telkens wijzen op:
   - de *parameters* (wat de aanvrager invult)
   - de *conditions* / *gates* (lid-niveau OR/AND)
   - de *outputs* (wat de werkgever terugkrijgt)
4. Wijs op de **untranslatables** per regeling — dat zijn de plekken
   waar jurist-input nodig is.

**Optioneel (zoom-in):** open
[financieel-cv-graph-detail.png](financieel-cv-graph-detail.png) voor
NRP alleen — de 8 cross-law inputs en lid-1/2/4 OR-gates.

---

## Deel 3 — Eén regeling grondig (20 min)

**Doel:** laten zien hoe de regelhulp inhoud van de wet vertaalt naar
toetsbare formules — gebruik NRP als blauwdruk.

1. Open [output-walkthrough.md](output-walkthrough.md) en scroll naar
   sectie **1. NRP**.
2. Loop met de groep door **Output 1 — `voldoet_aan_lid_1`**:
   - Lees de wettekst-citaat hardop.
   - Toon de formule.
   - Vraag de jurist: "Klopt deze formule met deze wettekst?"
   - Vink de checkbox af zodra het ja is.
3. Doe **Output 4 — `heeft_recht_op_no_risk_polis`** als showcase van
   de OR-orchestrator over de drie leden.
4. Doe **Output 5 — `duur_no_risk_polis_jaren`** als showcase van een
   *untranslatable*: lid 2 als `0` jaar, vijfjaarstermijn-onderbreking
   als open punt.

**Discussie-vraag:** "Welke van deze checkboxes durven jullie nu te
bekrachtigen, welke moeten naar een vervolgsessie?"

---

## Deel 4 — Live in de editor (15 min)

**Doel:** laten voelen dat dit niet alleen documentatie is maar
draaiende code die per persoon doorrekent.

1. Open <http://localhost:3000> in de browser.
2. Klik in de library op **Ziektewet**, peildatum 2025-01-01.
3. Open de **Wettengraaf**-pane (rechts of via menu) — ze zien dezelfde
   structuur als in stap 2, maar nu interactief.
4. Toon een **persona-trace** door deze in de editor terminal te tonen:

   ```bash
   cat docs/financieel-cv/persona-traces/no-riskpolis-persona-1-wia-uitkering.txt
   ```

   Dit is een werknemer met WIA-uitkering die wordt aangenomen — laat
   zien hoe `voldoet_aan_lid_1` resulteert in `True` en
   `heeft_recht_op_no_risk_polis = True` met `duur = 5 jaar`.

5. Run live de BDD-suite:

   ```bash
   just bdd
   ```

   88/88 scenarios groen, ~5 seconden. Niet papier, runnable.

---

## Deel 5 — Open vragen voor de jurist (10 min)

**Doel:** vooraf inkaderen wat in vervolgsessies juridisch besproken
moet worden.

1. Open [mvt-referenties.md](mvt-referenties.md).
2. Loop per regeling de **Open vragen voor jurist** sectie langs.
3. Vraag de groep: "Welke van deze vragen moeten in vervolgsessie 1,
   welke in sessie 2?" Maak met ze een ruwe planning.
4. Belangrijkste vroeg-te-beantwoorden vragen:
   - **NRP** — Beschut werk (Pwet 10b) was uitgesloten in MvT 34194,
     nu via lid 2.f ingesloten. Welke MvT-passage geldt nog?
   - **LKS** — `loonwaarde_eurocent_per_maand` mét of zonder
     vakantiebijslag? Zie code-review-actiepunt.
   - **JC/WPA** — Reïntegratiebesluit als implementing regulation?

---

## Deel 6 — Wat is er nog niet (5 min)

**Doel:** transparant zijn over de scope-grenzen voordat de groep
verwachtingen vormt.

Open [README.md](README.md) en scroll naar de sectie
**"Niet-gedaan / volgende iteratie"**:

- Reïntegratiebesluit (BWBR0018394) niet als `implements` geharvest
- Wet sociale werkvoorziening (BWBR0008903) niet geharvest
- Werkgever-WPA (Wet WIA art. 36) niet uitgewerkt
- Uitvoeringsbeleid-laag (RVO regelhulp als orchestrator) niet
  gebouwd — *dat is wat we samen zouden doen*

Ook op tafel:

- 3 follow-up actiepunten uit de code-review (zie eind van README:
  "Lokale-only state" en zoek "follow-up"). Niet blokkerend voor
  vandaag, wel voor productie.

---

## Wrap-up (5 min)

**Volgende stappen — voorstel om af te stemmen:**

1. **Vervolgsessie 1 (1 week na kick-off)**: per regeling de overige
   outputs in `output-walkthrough.md` invullen mét jurist en
   regelhulp-makers.
2. **Vervolgsessie 2 (2-3 weken na kick-off)**: bouwen van het
   uitvoeringsbeleid-niveau — `regelhulp_financieel_cv` als
   `regulatory_layer: UITVOERINGSBELEID` die de 8 outputs
   orchestreert.
3. **Tussendoor schriftelijk**: open vragen uit `mvt-referenties.md`
   en de 3 code-review-actiepunten.

**Stack afsluiten na de workshop:**

```bash
just dev-down
git checkout packages/editor-api/src/feature_flags.rs   # demo-only patch revert
```

---

## Cheatsheet — bestanden in volgorde van gebruik

| Stap | Bestand | Wat |
|------|---------|-----|
| Voorber. | [README.md](README.md) | Projectoverzicht |
| Deel 1 | [stelsel-overview.md](stelsel-overview.md) | Tekst + uitleg stelsel |
| Deel 1 | [financieel-cv-stelsel.png](financieel-cv-stelsel.png) | Stelsel-diagram |
| Deel 2 | [financieel-cv-graph-detail-alle-7.png](financieel-cv-graph-detail-alle-7.png) | Detail-graph alle regelingen |
| Deel 2 | [financieel-cv-graph.png](financieel-cv-graph.png) | Compacte overview-graph |
| Deel 2 | [financieel-cv-graph-detail.png](financieel-cv-graph-detail.png) | NRP zoom-in |
| Deel 3 | [output-walkthrough.md](output-walkthrough.md) | Wettekst+formule per output |
| Deel 4 | <http://localhost:3000> | Live editor + Wettengraaf |
| Deel 4 | [persona-traces/](persona-traces/) | Trace-output van BDD-runs |
| Deel 5 | [mvt-referenties.md](mvt-referenties.md) | MvT + open vragen |
| Deel 6 | [README.md](README.md) | "Niet-gedaan"-sectie |

## Cheatsheet — commando's

```bash
# Validatie van alle YAMLs
just validate

# Volledige BDD-suite (88/88 scenarios)
just bdd

# BDD met traces (nuttig om persona-uitkomst toe te lichten)
just bdd-trace
ls trace_output/ | grep ziektewet

# Editor lokaal starten
export GITHUB_TOKEN=$(security find-generic-password -a "$USER" -s github-packages-read -w)
just dev

# Editor stoppen
just dev-down
```
