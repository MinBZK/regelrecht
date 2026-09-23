# Modellering-fixes-plan — Financieel CV

**Datum**: 22 september 2026 · **Scope**: de zeven wetten van het Financieel CV plus
het Reintegratiebesluit · **Bron**: de vier-weg-classificatie in
[`classificatie-untranslatables.md`](classificatie-untranslatables.md)

> Dit document bevat **modellering-fouten**: gevallen waar onze YAML afwijkt van de
> correcte wettekst. Voor fouten in de wet zelf, zie
> [`wetgevingsfouten-analyse.md`](wetgevingsfouten-analyse.md).

## Bevindingen die niet opnieuw onderzocht hoeven worden

### Kritieke fouten alleen in YAML

| # | Wet / artikel | Fout |
|---|---|---|
| 1 | Wfsv 38b | De omschrijving van `datum_opname_doelgroepregister` noemt een LKV-periode van drie jaar. Wtl artikel 2.12 kent die termijn niet meer: het loonkostenvoordeel doelgroep banenafspraak loopt door zolang de dienstbetrekking en de voorwaarden bestaan. |
| 2 | Wfsv 38b | De chapeau-uitsluiting beschut werk komt binnen als parameter `is_uitgesloten_beschut_werk_pwet_10b`, terwijl Participatiewet artikel 10b de output `verricht_arbeid_in_beschut_werk` al levert. RFC-007 is aanvaard en geimplementeerd. |
| 3 | Pwet 10c | De evenredig verminderde loonkostensubsidie wordt niet afgerond; de untranslatable noemde dat een taalgat. RFC-023 en RFC-024 over bedragen en afronding zijn voorgesteld en geimplementeerd, dus de engine kan dit. |
| 4 | ZW 29b | Onder de Ziektewet staat een untranslatable die vastlegt dat er geen lagere regelgeving is. Een bevestigde leegte is geen untranslatable. |
| 5 | Pwet 10c en 10d | De loonkostensubsidieberekening staat in het `machine_readable`-blok van artikel 10c, met omschrijvingen die naar "lid 4" verwijzen. Artikel 10c kent twee leden en gaat over de doelgroepvaststelling; de berekening staat in artikel 10d lid 4, dat geen `machine_readable`-blok heeft. |
| 6 | Wet WIA 37 | De wet vraagt haar eigen conclusie als invoer. `heeft_recht_op_wia_uitkering` is de OR van de rechten die artikel 47 en 54 al berekenen; de Ziektewet rekent die OR vandaag uit en de WIA vraagt hem op. Gevonden met de dubbelencontrole van 23 september. |

### Structurele problemen

| Probleem | Bestanden |
|---|---|
| Geen. De acht dossierbestanden staan sinds 22 september 2026 alle op schema v0.7.0 en valideren schoon. | — |

### Wel getrouw, niet aanraken

- De 25 gemodelleerde artikelen zijn tekstueel gelijk aan de wettekst op de datum die de YAML zelf claimt.
- De vier Wajong-artikelnummers zijn correct: de bestanden dragen `2:15`, `2:20`, `2:22` en `2:24`. De getallen 135, 140, 142 en 144 ontstaan alleen bij het lezen met PyYAML. Zie [`pyyaml-valkuil.md`](pyyaml-valkuil.md).

## Fix-plan

| Fix | Wet/artikel | Wat | Validatie na fix |
|---|---|---|---|
| F1 | Wfsv 38b | **Gedaan 23 september.** Omschrijving noemt nu wat Wtl 2.12 wel zegt | `script/validate.sh` 8/8 OK |
| F2 | Wfsv 38b | **Gedaan 23 september.** De parameter is vervangen door een `input` met `source.regulation: participatiewet`, output `is_uitsluitend_aangewezen_op_beschut_werk`. De drie parameters van Pwet 10b worden doorgegeven, dus de lijst van 38b gaat van 15 naar 17. Zie [`diepte-van-een-variabele.md`](diepte-van-een-variabele.md) | `script/validate.sh` 8/8 OK; BDD 76/76 groen |
| F3 | Pwet 10c | Afronding van `hoogte_lks_eurocent_per_maand` toepassen volgens RFC-023 en RFC-024 | `just bdd` op `loonkostensubsidie.feature` |
| F4 | ZW 29b | **Gedaan 22 september** bij de migratie | `script/validate.sh` 8/8 OK |
| F5 | Pwet 10c en 10d | `machine_readable` van de LKS-berekening verplaatsen naar artikel 10d, of ten minste de omschrijvingen corrigeren | `just validate` + `just bdd` |
| F6 | Wet WIA 37 | De parameter `heeft_recht_op_wia_uitkering` vraagt om een recht "op grond van deze wet". De WIA rekent het zelf uit: `heeft_recht_op_iva_uitkering` (art. 47) OF `heeft_recht_op_wga_uitkering` (art. 54). Nu rekent de Ziektewet die OR uit en vraagt de WIA hem aan de aanroeper. **Mijn eerste voorstel was fout:** niet de OR van 47 en 54. Artikel 5 zegt "doch die **niet** volledig en duurzaam arbeidsongeschikt is", artikel 47 lid 1 b zegt het tegenovergestelde. Een IVA-gerechtigde is geen adressaat van artikel 37. Juiste binding: `is_gedeeltelijk_arbeidsgeschikt AND heeft_recht_op_wga_uitkering`. Zie [`fideliteitsaudit.md`](fideliteitsaudit.md) | `script/validate.sh` + `just bdd` op de WIA-scenario's |

## Open vraag

- Accepteert wetten.overheid.nl het ankerformaat `#Artikel220` voor artikelen met een dubbele punt? Het corpus gebruikt die vorm voor alle 472.245 ankers in de Wajong en de Awb. Werkt zij niet, dan is het een corpusbrede harvester-fix en geen dossierfix. Vergt een netwerkcontrole.

- Hoort `heeft_recht_op_wia_uitkering` als output in de Ziektewet thuis? De
  Ziektewet berekent daar een recht op grond van een andere wet. Na F6 heeft de
  Wet WIA die waarde zelf, en kan de Ziektewet hem via `source` lezen in plaats
  van zelf samen te stellen.
