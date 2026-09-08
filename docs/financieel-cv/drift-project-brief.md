# Projectbrief — Financieel CV drift-fix (nette versie)

**Werkbranch (corpus-repo):** `traject/financieel-cv-validatie-df48ddd1`
**Traject in de editor:** `/trajecten/financieel-cv-validatie-df48ddd1`
**Basis:** schoon afgetakt van `origin/main` — alle 2026-versies en alle 103
Pwet-versies intact, nog géén `machine_readable`.

## Waarom dit project

Voor de sessie van 23 juli hebben we op de vórige traject-branch
(`financieel-cv-0bc401e0`) een reeks **temp-fixes** gedaan om het demo-klaar te
krijgen. Die werken, maar zijn niet houdbaar. Hier bouwen we de nette versie —
op de wet zoals die nú geldt, zonder trucs.

## Temp-fix → nette fix

| Wat we tijdelijk deden | Wat hier moet gebeuren |
|---|---|
| Peildatum scenario's op **2026-06-01** gezet als workaround | Scenario's op een reële, actuele datum; wetten resolven vanzelf naar de geldende versie |
| **2025-logica op 2026-teksten geplakt** (6 wetten) | `machine_readable` autheren/verifiëren **op de 2026-versies**, met correctie voor de echte tekstwijzigingen |
| **5 Pwet-versies verwijderd** om de engine-limiet te omzeilen | Alle versies behouden; de engine-limiet echt oplossen (zie hieronder) |
| Pwet **schema-declaratie** opgehoogd | Nette schema-afhandeling per versie |

## Twee harde blokkades die eerst opgelost moeten

### 1. Pwet > 1000 artikelen (engine-limiet)
De Participatiewet is per 2026 gegroeid naar **1018 artikelen**; de engine
weigert alles boven **1000** (`Too many articles`). Daardoor is geen enkele
2026-Pwet-versie laadbaar — de reden dat we op de vorige branch de versies
moesten wéghalen.

**Dit is een engine-limitatie, geen corpus-fout.** De nette route is de limiet
in de engine verhogen (of de laadstrategie aanpassen), niet de corpus snoeien.
→ Aparte engine-issue nodig; blokkeert het modelleren van LKS op de actuele Pwet.

### 2. Waarom de v0.5.1-versies niet in de browser laadden
Op de vorige branch laadde de gedeployde WASM-engine onze 2025-versies niet,
terwijl de native engine (BDD) dat wél deed en het bestand valide was. Oorzaak
niet gevonden — nu omzeild via de 2026-versies. Voordat we hier op één
schema-versie standaardiseren, moet dit begrepen zijn (pod-logs / WASM-build
vergelijken met native).

## De echte drift (feitelijke wetswijzigingen 2025 → 2026)

Bevestigen met de jurist, dan verwerken:

- **Wtl:** categorie **d** (herplaatsen arbeidsgehandicapte) is **geschrapt**;
  `2.10`/`2.11` geherstructureerd. Ons model kent nog vier LKV-categorieën.
- **Pwet:** doelgroep uitgebreid met **`10d.2.c`** (leer-werktraject zonder
  startkwalificatie, verwijst naar art. 7a lid 3).
- Overige mr-dragende artikelen (Ziektewet 29b, Wajong 2:20, WIA 35, WW 76a,
  Wfsv 38b) zijn tekstueel **identiek** tussen 2025 en 2026 — die kunnen 1-op-1
  mee, mits geverifieerd.

## Samenhang met het inhoudelijke werk

De **modellering** van nieuwe outputs (Wajong-voorzieningen, Wajong-regimes,
LKS naar rato / 50%-regeling, werkgeverslasten, Reïntegratiebesluit) staat in
`modelleerplan-vervolg.md`. Dat werk landt óók op deze branch. Volgorde en
blokkades (o.a. wachten op UWV) staan daar.

## Aanpak

1. **Engine-limiet Pwet** oplossen of omzeilen zonder versies te verwijderen —
   eerst dit, anders kan LKS niet op de actuele Pwet.
2. **FCV-`machine_readable` autheren op de 2026-versies**, per wet:
   `law-generate` → `law-reverse-validate` → scenario's. Voor de identieke
   wetten is dit een gecontroleerde port; voor Wtl en Pwet mét drift-correctie.
3. **Scenario's** per wet (zoals nu), reële peildatum, geen workaround.
4. **`just bdd` groen** houden als poortwachter.
5. Pas daarna het inhoudelijke modelleerwerk uit `modelleerplan-vervolg.md`.

## Wat NIET meenemen van de vorige branch

- de peildatum-workaround (2026-06-01)
- de verwijderde Pwet-versies
- de op-2026-geplakte 2025-logica (wordt hier opnieuw en geverifieerd gedaan)

De **scenario-inhoud** (persona's Koen/Sadee, de assertions) en de
**structuur** (één bestand per wet) blijven wél de referentie — die zijn goed.
