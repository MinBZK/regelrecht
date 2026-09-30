# Modelleerplan na sessie 1

Wat er gemodelleerd of geharvest moet worden om de bevindingen uit de
SZW-sessie (`szw/2026-07-23-juristvalidatie-notities.md`) te sluiten.

**Kernbevinding voor de planning:** het meeste werk is **modelleren**, niet
harvesten. De wetteksten staan grotendeels al in de corpus — alleen de
`machine_readable` ontbreekt. Twee stukken lagere regelgeving moeten wél
opgehaald worden.

---

## Spoor A — modelleren, tekst staat er al

Geen harvest nodig; deze artikelen zitten al in de corpus en missen alleen
executielogica.

### A1 · Wajong-voorzieningen als tegenhanger van WIA 35
**Sluit:** bevinding 2 (dubbele info, "lijkt geen recht")
**Artikelen:** `2:22.1` t/m `2:29.2` — **al aanwezig als tekst** (2:22, 2:23,
2:24, 2:27, 2:28, 2:29)
**Werk:** `machine_readable` toevoegen op de voorzieningen-artikelen, met
outputs die spiegelen aan WIA 35 (`heeft_recht_op_jobcoaching`,
`heeft_recht_op_werkplekaanpassing`).
**Waarom prioriteit 1:** dit is de enige bevinding waar het CV nú een
**misleidend antwoord** geeft — "geen recht" terwijl het recht via een andere
wet bestaat.

### A2 · Wajong-regimes (drie tijdperken)
**Sluit:** bevinding 3
**Artikelen:** de Wajong-YAML kent **hoofdstuk 1a, 2 en 3** — dat zijn de drie
regimes. Alle drie al aanwezig als tekst.
**Werk:** eerst uitzoeken (met de jurist) wat elk regime voor de
LDP-berekening betekent; daarna een regime-bepaling modelleren en de
berekening daarop laten aftakken.
**Blokkade:** inhoudelijk nog onduidelijk — wacht op toelichting.

### A3 · LKS naar rato bij < 36 uur
**Sluit:** open vraag 4 uit de vervolgsessie-agenda
**Artikel:** `10d.4` (slotzinnen) — aanwezig, nu `untranslatable`
**Werk:** parameter `overeengekomen_arbeidsduur_uren_per_week` toevoegen en de
subsidie schalen met `arbeidsduur / 36`. Let op de bepaling dat de
"overeengekomen arbeidsduur" maximaal de in de sector gebruikelijke volledige
werkweek is.
**Inschatting:** klein — één parameter en één vermenigvuldiging.

### A4 · LKS 50%-regeling (lid 5)
**Sluit:** open vraag 3
**Artikelen:** `10d.1.b` + `10d.5` — beide aanwezig, lid 5 nu `untranslatable`
**Blokkade:** de huidige operatie-set kan één dienstverband niet in twee
tariefperiodes splitsen (engine-limitatie).
**Voorstel om er tóch omheen te komen:** niet als tijd-split modelleren, maar
als **twee losse outputs** — `hoogte_lks_eerste_zes_maanden_eurocent` (50% van
WML+VB) naast het structurele bedrag. Dan toont het CV beide zonder dat de
engine hoeft te knippen. Vergt geen engine-wijziging.

---

## Spoor B — eerst harvesten

Deze staan **niet** in de corpus. Ophalen via de harvester (geen hand-edits van
wettekst).

### B1 · Ministeriële regeling werkgeverslasten
**Sluit:** bevinding 1 en 3 uit de notities (de werkgeverslasten-knoop)
**Grondslag:** Pwet `10d.4` verwijst naar *"een bij ministeriële regeling
vastgestelde vergoeding voor werkgeverslasten"*
**Status corpus:** afwezig. Onder `ministeriele_regeling/` staan nu alleen
`regeling_standaardpremie` en een energie-regeling.
**Blokkade:** we weten nog **niet welke regeling** het precies is — dat is de
vraag die bij UWV uitstaat.
**Werk daarna:** harvesten, dan als `implements` koppelen aan de `open_term`
`werkgeverslastenvergoeding_eurocent`.

### B2 · Reïntegratiebesluit (BWBR0018394)
**Sluit:** de untranslatables rond WIA 35 lid 2.c/2.d ("in overwegende mate op
individu afgestemd", "noodzakelijke persoonlijke ondersteuning")
**Grondslag:** AMvB onder WIA `35 lid 5`
**Status corpus:** afwezig; in onze YAML aangeroepen als `open_term`
`nadere_regels_voorzieningen_artikel_35`
**Werk:** harvesten en als `implements` aansluiten. Geeft jobcoaching en
werkplekaanpassing een echte grondslag in plaats van een open norm.
**Niet blokkerend** — kan parallel.

---

## Spoor C — versie-drift bijwerken

### C1 · Wtl: categorie d geschrapt per 2026
Ons model kent vier LKV-categorieën (a/b/c/d); in de 2026-tekst is
**`2.1.d` — herplaatsen arbeidsgehandicapte — vervallen**, en zijn `2.10`/`2.11`
geherstructureerd. De modellering moet mee.

### C2 · Pwet: doelgroep uitgebreid met `10d.2.c`
Nieuw onderdeel per 2026: leer-werktraject zonder startkwalificatie (verwijst
naar art. 7a lid 3). Onze LKS-doelgroepbepaling mist dit.

> Beide zijn feitelijke wetswijzigingen, geen interpretatie. Wel eerst met de
> jurist bevestigen dát ze zo gelden.

---

## Spoor D — opruimen

### D1 · LIV uit scope
**Besluit uit de sessie.** Verwijderen: de LIV-scenario's (nu bewust falend),
de LIV-vermeldingen in pre-read, host-briefing en persona-diagrammen, en de
historische Wtl-hoofdstuk-3-verwijzingen.
**Inschatting:** klein, maar raakt veel bestanden — in één keer doen.

---

## Volgorde

| Fase | Wat | Waarom nu |
|---|---|---|
| **1** | A1 (Wajong-voorzieningen) | enige misleidende uitkomst; niet geblokkeerd |
| **1** | D1 (LIV eruit) | besluit genomen, opruimen voordat er meer op stapelt |
| **2** | A3 (naar rato 36 uur) | klein, direct effect op het bedrag |
| **2** | B2 (Reïntegratiebesluit) | parallel, niet geblokkeerd |
| **3** | A4 (50%-regeling via twee outputs) | vergt akkoord op de modelleer-truc |
| **3** | C1 + C2 (drift) | na bevestiging door de jurist |
| **4** | B1 (werkgeverslasten) | **wacht op UWV** |
| **4** | A2 (Wajong-regimes) | **wacht op toelichting** |

**Kritiek pad:** fase 1 kan meteen. Fase 4 ligt volledig stil tot het
UWV-antwoord er is — plan daar niet omheen.

## Aanpak per stuk

Voor het modelleerwerk (spoor A) ligt de route vast in de bestaande skills:
`law-generate` voor de `machine_readable`, daarna `law-reverse-validate` als
hallucinatie-check, en scenario's erbij zodat elke nieuwe output door BDD gedekt
is. Voor spoor B: `law-download` / de harvester, nooit met de hand.

Elke toevoeging krijgt scenario's in het bijbehorende `scenarios/`-bestand —
onder **dezelfde wet als waar de logica op hangt**, anders voert de editor ze
tegen de verkeerde wet uit.
