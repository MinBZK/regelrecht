---
name: regelrecht-bdd-lokaal
description: >
  Draait de regelrecht BDD-suite lokaal wanneer de standaardroute uit CLAUDE.md niet
  werkt — geen mold, geen cc/gcc, of een corpus dat te groot is voor de engine. Gebruik
  dit bij "linker `cc` not found", "linker not found", `Maximum law count exceeded`,
  `Law not found: <wet>` terwijl de YAML er wél staat, `Regulation directory not found:
  <pad>/nl`, "No feature files found", of wanneer je een cross-law dossier tegen een
  corpus uit een andere repo wilt draaien. Beschrijft de musl/rust-lld-uitwijkroute, het
  bouwen van een mini-corpus onder de wettencap, en hoe je de exitcode betrouwbaar
  afleest zodat een gefaalde build niet als groen wordt gemeld. Dossier-agnostisch:
  de wetten in de voorbeelden zijn plaatshouders, de rookproef draait op het
  testcorpus van de repo zelf.
allowed-tools: Read, Grep, Glob, Bash, Edit
---

# BDD lokaal draaien zonder mold en zonder systeem-cc

`CLAUDE.md` beschrijft de standaardroute: `just bdd` met de mold-linker. In een
omgeving zonder mold en zonder `cc`/`gcc` werkt die route niet, en de foutmeldingen
wijzen alle vier de verkeerde kant op — ze lijken op modelleerfouten in het corpus
terwijl het harnasfouten zijn.

**Kernregel: bij een rode BDD-run eerst het harnas verdenken, dan pas het corpus.**
Vier van de vijf valkuilen hieronder produceren tientallen rode scenario's die er
uitzien als een fout in de YAML.

> **Vier secties in deze skill beschrijven een fixbaar defect, geen kennis.** Ze
> zijn gemarkeerd met ⚠︎ en staan opgesomd in
> [Wat hiervan hoort te verdwijnen](#wat-hiervan-hoort-te-verdwijnen). Kom je er een
> tegen, overweeg dan de fix in plaats van de omweg — zolang de omweg beschreven
> staat, is de pijn weg die anders tot de fix had geleid.

## Snelle diagnose

| Melding | Oorzaak | Zie |
|---|---|---|
| `linker 'cc' not found` | Geen systeem-C-toolchain | [Toolchain](#toolchain) |
| `linker not found` (met musl-target) | `rust-lld` uit de **target**-rustlib gepakt | [Het linkerpad](#het-linkerpad-is-de-valkuil) |
| `Maximum law count exceeded` | Meer dan 100 wetsversies geladen | [De wettencap](#de-wettencap) |
| `Law not found: <wet>` terwijl de YAML bestaat | Idem — de wet viel buiten de cap | [De wettencap](#de-wettencap) |
| `Regulation directory not found: <pad>/nl` | `REGULATION_PATH` mist de `nl/`-laag | [De nl-laag](#de-nl-laag-is-verplicht) |
| `No feature files found under ...` | Features komen uit `corpus/regulation`, niet uit `REGULATION_PATH` | [Twee bronnen](#twee-bronnen-wetten-en-features) |
| `Law not found: test_date_operations` (en broertjes) | Bucket B draait mee tegen jouw corpus | [Twee bronnen](#twee-bronnen-wetten-en-features) |
| Eén scenario faalt op een ontbrekende hook-output | Een nieuwere, lege wetsversie overschaduwt een oudere | [Versieschaduw](#versieschaduw) |

## Rookproef — eerst zonder mini-corpus

Voordat je aan een groot corpus begint: draai bucket A tegen het eigen testcorpus
van de repo. Dat is klein genoeg voor de cap, staat al op de plek waar de runner
zijn features zoekt, en vraagt dus geen omsymlinken. Zo scheid je een
toolchain-probleem van een corpus-probleem:

```bash
cd packages/engine
REGULATION_PATH=../../corpus/regulation BDD_BUCKET=corpus \
  cargo test --target aarch64-unknown-linux-musl --test bdd
```

```
6 features
45 scenarios (45 passed)
462 steps (462 passed)
```

Groen? Dan staat je toolchain goed en zit een volgend probleem in het corpus of in
de selectie. Rood? Werk dan eerst [Toolchain](#toolchain) af — aan een mini-corpus
bouwen heeft nog geen zin.

## Toolchain

Bouwen lukt via het target `aarch64-unknown-linux-musl` met `rust-lld`. De
C-wrappers staan in `~/.local/bin/` (`cc`, `ar`, `aarch64-linux-musl-gcc`,
`aarch64-linux-musl-ar`, alle rond `python3 -m ziglang cc|ar`) en bestaan al —
zet die map op `PATH`.

```bash
export PATH="$HOME/.cargo/bin:$HOME/.local/bin:$PATH"
export CC_aarch64_unknown_linux_musl="$HOME/.local/bin/aarch64-linux-musl-gcc"
export AR_aarch64_unknown_linux_musl="$HOME/.local/bin/aarch64-linux-musl-ar"
```

### Het linkerpad is de valkuil

`rust-lld` moet uit de **host**-rustlib komen, niet uit de target-rustlib:

```bash
export CARGO_TARGET_AARCH64_UNKNOWN_LINUX_MUSL_LINKER=\
"$HOME/.rustup/toolchains/1.96.0-aarch64-unknown-linux-gnu/lib/rustlib/aarch64-unknown-linux-gnu/bin/rust-lld"
export RUSTFLAGS="-C linker-flavor=ld.lld -C link-self-contained=yes"
```

Er staat een tweede, aannemelijker ogend binary onder
`~/.rustup/toolchains/1.96.0-aarch64-unknown-linux-**musl**/lib/rustlib/aarch64-unknown-linux-musl/bin/rust-lld`.
Dat pad bestaat, maar faalt met `linker not found`. Pak het niet omdat het
"bij het target hoort" — de actieve toolchain is de gnu-host, en die heeft
onder zijn eigen `rustlib/aarch64-unknown-linux-musl/` wél een std maar géén
`bin/`. Controleer bij twijfel:

```bash
ls "$(rustc --print sysroot)/lib/rustlib/$(rustc -vV | sed -n 's/^host: //p')/bin/rust-lld"
```

## De wettencap

⚠︎ *Fixbaar defect — zie [Wat hiervan hoort te verdwijnen](#wat-hiervan-hoort-te-verdwijnen), punt 1.*

De engine weigert boven de **100 geladen wetten** met `Maximum law count exceeded`
(`packages/engine/src/resolver.rs`), en **elke versie telt afzonderlijk mee**.
Het volledige corpus (± 22.000 YAML's) laadt dus maar deels; alles wat na de cap
komt geeft `Law not found: <wet>`.

**Diagnoseregel: bij `Law not found` eerst aan de cap denken, niet aan de YAML.**
De melding noemt de wet die je zoekt, niet de wet die de cap opvulde — hij wijst
per definitie naar de verkeerde plek.

### Mini-corpus

Draai tegen een selectie: per wet alleen de nieuwste versie ≤ peildatum, plus
`status.yaml` en de hele map `scenarios/`. `mini-corpus.sh` in deze skill doet dat:

```bash
.claude/skills/regelrecht-bdd-lokaal/mini-corpus.sh \
  <corpus-repo>/regulation <peildatum> /tmp/mini \
  wet/<wet-a> wet/<wet-b> amvb/<besluit>
```

### Welke wetten horen erin

1. De wetten waar je scenario's over gaan.
2. Elke wet die daaruit wordt aangeroepen via een `source.regulation` — anders
   valt de cross-law-keten om op `Law not found`, wat leest als een modelleerfout.
   Uit te lezen met:
   ```bash
   grep -rho 'regulation: *"\?[a-z_0-9]*' <corpus>/nl/wet/<wet>/*.yaml \
     | sed 's/.*: *"\?//' | sort -u
   ```
3. Elke wet of AMvB die via `implements` een open term van een van de bovenstaande
   invult — laat je die weg, dan valt de open term stil terug op zijn default en
   klopt de uitkomst nét niet.

Herhaal stap 2 en 3 op de wetten die je erbij haalt, tot de lijst niet meer groeit.
Blijf ruim onder de 100 versies; met een handvol wetten per dossier is dat geen
beperking.

Het script logt per wet welke versie het pakt bij die peildatum. Lees dat na — een
wet die op een oudere versie blijft steken dan je verwacht is een veelvoorkomende
verklaring van een onverwacht rode run.

### De nl-laag is verplicht

⚠︎ *Fixbaar defect — punt 3.*

`REGULATION_PATH` moet wijzen op een map die een `nl/`-**submap** bevat, niet op de
`nl/`-map zelf. Wijs je hem één niveau te diep, dan falen *álle* scenario's op
`Regulation directory not found: <pad>/nl`. Dat leest als tientallen
modelleerfouten en is er nul.

### Versieschaduw

⚠︎ *Fixbaar defect — punt 2.*

De output- en hook-index worden gebouwd uit de **nieuwste** versie van een wet.
Een nieuwere versie zonder `machine_readable` overschaduwt daarmee een oudere die
hem wel heeft — ook als de scenario-datum vóór die nieuwe versie ligt.

Concreet: de Awb `2026-07-01` heeft 565 artikelen en nul `machine_readable`. Zet je
hem in dezelfde map als de Awb-stub uit het testcorpus van de repo, dan verdwijnen
de hooks uit de index en valt `Hook outputs are included when requesting a
BESCHIKKING output` om — met een melding over een ontbrekende output, niet over een
versie. Meng daarom geen twee corpora in één map; kies er één per run.

## Twee bronnen: wetten en features

⚠︎ *Fixbaar defect — punt 4.*

De runner haalt zijn twee dingen uit twee verschillende plekken:

| | Waar vandaan | Bucket |
|---|---|---|
| Wetten (YAML) | `REGULATION_PATH` | — |
| Features | `<repo>/corpus/regulation/**/scenarios/*.feature` | A (law validation) |
| Features | `<repo>/bdd/conformance/*.feature` | B (engine conformance) |

`corpus/regulation` is in deze checkout **geen symlink** maar het eigen kleine
testcorpus van de repo (25 YAML's, 5 features). Om een corpus uit een andere repo
te draaien moet je die map tijdelijk vervangen door een symlink naar het
mini-corpus **en daarna terugzetten**. `corpus/` is tracked.

**Draai dan alleen bucket A**, met `BDD_BUCKET=corpus` (`all` is de default,
`conformance` is bucket B; zie `Bucket::from_env` in
`packages/engine/tests/bdd/main.rs`). Bucket B toetst de engine tegen de
synthetische `test_*`-wetten uit het testcorpus van de repo — precies de wetten die
je zojuist hebt weggesymlinkt. Laat je hem meedraaien, dan faalt hij op
`Law not found: test_date_operations` en lijkt jouw corpus rood terwijl er niets
mis mee is.

## Draaien

Zet de omwisseling in een script met een `trap`, zodat de repo ook bij een
onderbroken run terugkomt:

```bash
set -u
REPO=<pad-naar-deze-repo>
MINI=/tmp/mini
LOG=/tmp/bdd.log

cd "$REPO"
restore() { rm -f "$REPO/corpus/regulation"; git -C "$REPO" checkout -- corpus/regulation; }
trap restore EXIT

mv "$REPO/corpus/regulation" /tmp/regulation.orig
ln -s "$MINI" "$REPO/corpus/regulation"

cd "$REPO/packages/engine"
REGULATION_PATH="$MINI" BDD_BUCKET=corpus \
  cargo test --target aarch64-unknown-linux-musl --test bdd > "$LOG" 2>&1
CODE=$?

cd "$REPO"
rm -f "$REPO/corpus/regulation"
mv /tmp/regulation.orig "$REPO/corpus/regulation"
trap - EXIT
echo "exitcode=$CODE"
```

### Exitcodes

**Schrijf de uitvoer naar een logbestand en lees de exitcode apart.**
`cargo test ... | tail` levert de exitcode van `tail`, niet van `cargo` — een
gefaalde build ziet er dan groen uit. Dit is in de praktijk misgegaan: twee
mislukte runs zijn als "build groen" gemeld.

Cucumber panic't aan het eind bij falende steps; de run eindigt dan op **101**, niet
op 1. Alleen `exitcode=0` is groen.

```bash
tail -6 "$LOG"      # de samenvatting
grep -n "✘" "$LOG"  # de falende steps
```

## Na afloop

```bash
git status --short   # corpus/regulation moet weg zijn uit de lijst
ls -ld corpus/regulation   # een map, geen symlink
```

Blijft er een symlink of een wijziging staan, dan is de restore niet gelopen —
zet hem terug vóór je iets commit.

## Referentiegetallen

Twee getallen zijn vast en gelden voor iedereen op deze commit; de derde leg je
zelf vast. Ze verschuiven zodra iemand een feature toevoegt — controleer ze op je
eigen HEAD voordat je een afwijking als defect behandelt.

| Run | features | scenarios | steps | exitcode |
|---|---|---|---|---|
| Rookproef, `corpus/regulation`, `BDD_BUCKET=corpus` | 6 | 45 | 462 | 0 |
| Bucket B, `BDD_BUCKET=conformance` | 8 | 51 | 274 | 0 |
| Jouw mini-corpus | — | — | — | 0 |

Noteer de aantallen van je eigen selectie bij de eerste groene run — in de
dossier-README of bovenaan het runscript — en vergelijk daar elke volgende run mee.
Wijkt het **aantal** features of scenario's af zonder dat je een scenario hebt
toegevoegd, dan is er iets mis met de selectie, niet met het model: een wet die uit
het mini-corpus is gevallen neemt zijn hele `scenarios/`-map mee, en die scenario's
falen niet — ze verdwijnen.

## Veelgemaakte fouten

| Fout | Gevolg |
|---|---|
| `cargo test` zonder `--target aarch64-unknown-linux-musl` | Valt terug op de gnu-host en zoekt `cc` |
| `rust-lld` uit de musl-toolchain | `linker not found`, terwijl het pad bestaat |
| Volledig corpus in `REGULATION_PATH` | Cap-overschrijding; willekeurige wetten "bestaan" niet |
| `REGULATION_PATH` op de `nl/`-map zelf | Alle scenario's rood op een pad-fout |
| `BDD_BUCKET` weglaten bij een omgesymlinkt corpus | Bucket B rood op `Law not found: test_*`, ziet eruit als jouw fout |
| Twee corpora in één map mengen | De nieuwste versie wint de output- en hook-index; scenario's vallen om op een ontbrekende output |
| Uitvoer door `tail` pipen | Exitcode gaat verloren; rood wordt als groen gemeld |
| `corpus/regulation` niet teruggezet | Een symlink of een lege map belandt in een commit |

## Wat hiervan hoort te verdwijnen

Vier van de secties hierboven beschrijven geen kennis maar een defect: de code
laat een symptoom zien dat naar de verkeerde plek wijst, en deze skill vertaalt
dat terug. Zodra de fix landt, vervalt de sectie — schrap hem dan ook, want een
omweg die blijft staan wordt nagevolgd.

| # | Sectie | Wat er werkelijk mis is | Fix |
|---|---|---|---|
| 1 | [De wettencap](#de-wettencap) | `tests/bdd/helpers/regulation_loader.rs:44` logt elke mislukte `load_law` als `tracing::warn!` en gaat door. De cap-fout (`src/resolver.rs:189`) is duidelijk, maar komt nooit in beeld; de run valt tientallen scenario's later om op `Law not found`. | Overgeslagen bestanden tellen en falen, of één samenvattende regel printen |
| 2 | [Versieschaduw](#versieschaduw) | `src/resolver.rs:728-731` bouwt output-, implements-, hook-, override- en procedure-index uit `versions.first()` — de nieuwste versie — terwijl de evaluatie de versie kiest die geldt op de rekendatum. | Indexeren per geldende versie, of de index op datum bevragen |
| 3 | [De nl-laag is verplicht](#de-nl-laag-is-verplicht) | `tests/bdd/world.rs:75` panic't bij een onbereikbare map, en cucumber bouwt een `World` per scenario. Eén padfout levert daardoor evenveel panics als er scenario's zijn. | Pad één keer valideren vóór de run, met één melding |
| 4 | [Twee bronnen](#twee-bronnen-wetten-en-features) | `tests/bdd/main.rs:126` zoekt features onder `<repo>/corpus/regulation`, terwijl de wetten uit `REGULATION_PATH` komen. Daardoor is het omsymlinken van een tracked map nodig. | Bucket A zijn features onder `REGULATION_PATH` laten zoeken, of een aparte `FEATURE_PATH` |

De toolchain-secties zijn een ander verhaal: geen mold, geen systeem-`cc` en het
rust-lld-pad liggen buiten deze repo. Die blijven staan.
