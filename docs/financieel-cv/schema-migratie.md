# Schema-migratie v0.5.4 naar v0.7.0 — Financieel CV

**Datum**: 22 september 2026 · **Scope**: de zeven wetten van het Financieel CV plus
het Reintegratiebesluit, op `traject/financieel-cv-validatie-df48ddd1`
**Status**: voltooid lokaal, niet gecommit

## Doel

Alle acht bestanden van schema v0.5.4 naar v0.7.0, validerend onder het nieuwe schema.
Aanleiding is RFC-031: v0.7.0 verwijdert `untranslatables` en vervangt het door
`markings` voor een taalgat en `open_terms` voor inhoud die elders wordt ingevuld.
De 64 untranslatables van dit dossier moesten daarom hoe dan ook door een sorteerslag.

## Mechanische pass

- `$schema`: `schema-v0.5.4/schema/v0.5.4/schema.json` naar `schema-v0.7.0/schema/v0.7.0/schema.json`, in alle acht bestanden.
- `delegation_type: VERORDENING` naar `GEMEENTELIJKE_VERORDENING`, vier keer in de Participatiewet. In v0.5.4 was `delegation_type` vrije tekst; v0.7.0 sluit de enum.

## Semantische pass

De 64 untranslatables zijn eerst vier-weg geclassificeerd volgens
`regelrecht-stelselanalyse/references/classification.md` en daarna omgezet.

| Bestemming | Aantal | Vorm |
|---|---|---|
| `markings` | 21 | Het formaat heeft geen vorm voor de constructie |
| `open_terms` | 23 | De inhoud wordt elders ingevuld |
| Uit de YAML | 20 | Vastgelegd in de doc-producten |
| **Totaal** | **64** | |

Alle 21 markings dragen `resolution: model` op een na: de kalenderjaartermijn van
Wfsv 38f lid 1 draagt `resolution: operation`, omdat daar een YEAR-bewerking ontbreekt
en niet een vorm. Alle 21 dragen `target: []`, wat volgens het schema een uitspraak is
en geen omissie: het artikel blijft uitvoerbaar en alleen de verantwoording is
onvolledig. Dat klopt hier, want geen van de gemarkeerde artikelen laat een output weg.

Van de 23 nieuwe open terms dragen er zes `delegated_to` met `delegation_type`, omdat
de wet een invuller aanwijst. De overige zeventien dragen `decided_per_case_by`: de wet
wijst geen regeling aan, maar wel een gezag dat per geval oordeelt. Dat veld bestaat
sinds v0.7.0 en is precies voor dit geval bedoeld; in v0.5.4 was er geen plaats voor.

## Wat uit de YAML is gehaald

Twintig entries passen in geen van beide velden. RFC-031 is daar expliciet over:
"Verder past er niets in dit veld." Ze zijn verwijderd uit de wetsbestanden en
vastgelegd in de doc-producten.

| # | Wet | Artikel | Constructie | Waarom eruit | Waar het nu staat |
|---|---|---|---|---|---|
| 3 | Wfsv | 38b | definitie verloonde uren (38b.4) — alleen begripsbepaling | Begripsbepaling zonder eigen waarde; het aantal verloonde uren komt uit de polisadministratie. | [`classificatie-untranslatables.md`](classificatie-untranslatables.md) |
| 4 | Wfsv | 38b | datum_opname_doelgroepregister als invoer (geen berekening) | Modellering-fout: de omschrijving noemt een LKV-periode van drie jaar die Wtl artikel 2.12 niet meer kent. | [`modellering-fixes-plan.md`](modellering-fixes-plan.md) F1 |
| 5 | Wfsv | 38b | chapeau-uitsluiting beschut werk (Pwet 10b lid 1) | Modellering-fout: de chapeau-uitsluiting hoort een source-aanroep naar Participatiewet artikel 10b te zijn. | [`modellering-fixes-plan.md`](modellering-fixes-plan.md) F2 |
| 6 | Wfsv | 38f | berekeningsformule quotumpercentage (38f.2 — variabelen A t/m H) | Scope-vraag S2: de quotumformule werkt op sectorniveau en valt buiten een regelhulp voor een burger. | [`classificatie-untranslatables.md`](classificatie-untranslatables.md) |
| 10 | Wtl | 2.1 | doelgroepverklaring-vereiste binnen 3 maanden (art. 2.3, 2.6) | Procedurenorm; bij juristvalidatie september 2026 belegd als disclaimer in de tool. | [`classificatie-untranslatables.md`](classificatie-untranslatables.md) |
| 18 | WIA | 23 | verkorte wachttijd op aanvraag (lid 6) | Scope-vraag S3: verkorte wachttijd op aanvraag. | [`classificatie-untranslatables.md`](classificatie-untranslatables.md) |
| 19 | WIA | 23 | verlenging van de wachttijd (artikel 24, 25 en 26) | Scope-vraag S3: verlenging van de wachttijd via de artikelen 24, 25 en 26. | [`classificatie-untranslatables.md`](classificatie-untranslatables.md) |
| 27 | WIA | 47 | ingangsdatum van het recht (lid 2) | Modelleerkeuze: dit artikel toetst of op de peildatum recht bestaat, niet vanaf wanneer. | [`classificatie-untranslatables.md`](classificatie-untranslatables.md) |
| 28 | WIA | 54 | ingangsdatum van het recht (lid 2) | Modelleerkeuze: dit artikel toetst of op de peildatum recht bestaat, niet vanaf wanneer. | [`classificatie-untranslatables.md`](classificatie-untranslatables.md) |
| 29 | WIA | 54 | samenstelling van de WGA-uitkering (lid 3 en 4) | Scope-vraag S1: hoogte en samenstelling van de WGA-uitkering. | [`classificatie-untranslatables.md`](classificatie-untranslatables.md) |
| 30 | Pwet | 8a | de verordening zelf | Gedekt door de bestaande open_terms van artikel 8a. | [`classificatie-untranslatables.md`](classificatie-untranslatables.md) |
| 32 | Pwet | 10 | "overeenkomstig de verordening, bedoeld in artikel 8a" | Gedekt door de bestaande open_terms van artikel 10. | [`classificatie-untranslatables.md`](classificatie-untranslatables.md) |
| 35 | Pwet | 10 | onderzoeksplicht en besluitvorming (lid 4) | Procedurele norm over onderzoek en besluitvorming; geen rekenregel. | [`classificatie-untranslatables.md`](classificatie-untranslatables.md) |
| 40 | Pwet | 10c | afronding van het evenredig verminderde subsidiebedrag | Modellering-fout: RFC-023 en RFC-024 over bedragen en afronding zijn geimplementeerd, dus de engine kan dit. | [`modellering-fixes-plan.md`](modellering-fixes-plan.md) F3 |
| 42 | Pwet | 10c | EU-woonplaatsverplaatsing (lid 10) | Uitvoeringsregel over bevoegdheid bij verhuizing binnen de EU. | [`classificatie-untranslatables.md`](classificatie-untranslatables.md) |
| 45 | Pwet | 10e | kan-bepaling zonder ingevulde AMvB | Notitie dat de AMvB onder artikel 10e er niet hoeft te zijn; het artikel draagt de open_terms al. | [`classificatie-untranslatables.md`](classificatie-untranslatables.md) |
| 51 | ZW | 29b | doelgroepverklaring-procedure — geen lagere regelgeving | Modellering-fout: bevestigd dat er geen lagere regelgeving is; een bevestigde leegte is geen untranslatable. | [`modellering-fixes-plan.md`](modellering-fixes-plan.md) F4 |
| 55 | Wajong | 2:15 | ontstaan op grond van artikel 8:10 lid 4 (lid 5) | Scope-vraag S5: het Wajong-overgangsrecht via artikel 8:10 lid 4. | [`classificatie-untranslatables.md`](classificatie-untranslatables.md) |
| 57 | Wajong | 2:20 | vermindering naar evenredigheid | Gedekt door de bestaande open_term dispensatiepercentage. | [`classificatie-untranslatables.md`](classificatie-untranslatables.md) |
| 62 | Wajong | 2:22 | onderdelen a en b van lid 2 (vervoer, intermediaire activiteiten) niet gemodelleerd | Scope-vraag S4: de overige voorzieningen van lid 2 onderdelen a en b. | [`classificatie-untranslatables.md`](classificatie-untranslatables.md) |

## Discoveries — schema

1. **De migratie was kleiner dan verwacht.** Buiten `untranslatables` en de ene enum
   raakte v0.7.0 niets in dit dossier. `construct`, `enables`, `defaults`, `interface`
   en `structural_choices` komen er niet in voor; `suggestion` een keer. Het
   Reintegratiebesluit valideerde zonder enige wijziging al schoon onder v0.7.0.
2. **`legal_text_excerpt` is verplicht bij een marking, en dat is lastiger dan het
   lijkt.** Slechts 31 van de 64 untranslatables droegen er een. Voor veertien
   markings moest een citaat uit de wettekst worden gehaald. Bij vier daarvan staat de
   bepaling in een ander artikel dan het artikel dat de marking draagt: de
   loonkostensubsidie-markings hangen aan Participatiewet 10c terwijl de tekst in 10d
   staat. Dat is hoe modellering-fout F5 boven water kwam.
3. **`decided_per_case_by` verandert de sortering.** In v0.5.4 was een discretionaire
   norm zonder aangewezen regeling nergens onder te brengen en werd zij untranslatable.
   In v0.7.0 is zij een open term. Zeventien van de 64 verhuizen om die reden.
4. **PyYAML leest dit corpus verkeerd.** `number: 2:20` wordt 140 onder YAML 1.1. De
   migratie is uitgevoerd met een loader waarin de sexagesimale resolvers uit staan.
   Zie [`pyyaml-valkuil.md`](pyyaml-valkuil.md).

## Validatie

- JSON-schema-validatie van alle acht bestanden tegen `schema/v0.7.0/schema.json` met `jsonschema` 4.26.0 → **8/8 schoon, 0 fouten**.
- Tellingen na migratie: 21 `markings`, 39 `open_terms` (16 bestaande plus 23 nieuwe), 0 `untranslatables`.

## Open punten

- `just validate` en `just bdd` zijn nog niet gedraaid. De schema-validatie hierboven is een losse controle en niet de projectpoort.
- De vijf fixes uit het modellering-fixes-plan zijn nog niet uitgevoerd.
- De engine leest `markings` volgens RFC-031 in dezelfde vier modi als `untranslatables`. Dat is niet in deze omgeving nagegaan.
