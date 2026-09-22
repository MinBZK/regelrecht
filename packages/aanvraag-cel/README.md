# aanvraag-cel

Een proof of concept bij RFC-022: een cel die een ingediende aanvraag vastlegt
als chronolexogram, en er via een reductie een lexostatus van maakt waarmee een
artikel (een `TOETS`) beoordeelt of de aanvraag volledig is.

De code noemt geen casus. Stroom, lexostatus-definities en corpus komen uit
configuratie; de tests draaien op de generieke fixtures in `tests/fixtures/`.
De docs-pagina `docs/src/content/docs/components/aanvraag-cel.md` beschrijft
dezelfde opzet, met de afwijkingen van RFC-022 en de open vragen.

## Starten

```bash
just aanvraag-cel          # cel op :7170, frontend op :7171, op de fixtures
```

## De vier lagen

1. **Lexogram.** De regelingen onder `REGULATION_PATH`, ongewijzigd. Een
   artikel declareert welke parameters het nodig heeft.
2. **Stroomdefinitie** (`stroom`, schema `schema/chronolex/v0.1.0/stream.json`).
   Welke feiten de cel vastlegt, door wie, in welke kroniek en op welke
   `grondslag` (een lijst). Een veld bindt aan `$intake.*` (wie en langs welke
   weg), aan `$external.*` (de inhoud zoals ingediend) of is een constante van
   de stroom. Een `$external`-waarde mag meer dan een veld voeden. Een
   tabelveld declareert zijn kolommen:
   `{tabel: $external.<pad>, kolommen: [...]}`. Een
   indiening van soort `aanvraag` heeft `fields.kern` (Awb 4:2 lid 1: aanvrager
   met naam en adres, dagtekening, gevraagde beschikking, ondertekening) en
   `fields.inhoud`. `niet_gereduceerd` noemt met reden de velden die geen
   afleiding leest.
3. **Reductie tot lexostatus** (`reductie`, schema `lexostatus.json`). Een
   `lexostatus_definitions`-regel kiest met `filter` en `kies: laatste` een gram
   uit de kroniek en leidt per parameter een waarde af. De woordenschat:
   `veld`, `gevuld`, `gelijk`, `tabel` met `elke_regel` of `een_regel` (en
   `alleen_waar`), en `moment`. Een afleiding waarover het gram niets zegt,
   levert niets op; de cel vult nooit aan.
4. **Het gram** (`kroniek`, schema `gram.json`). Een JSON-regel per gram in
   `DATA_DIR/<chronicle>.jsonl`, alleen toevoegen. Een gram draagt `kind`,
   `type`, `soort`, `name`, `chronicle`, `recording_actor`, `grondslag`,
   `op_moment`, `zaakkenmerk`, `stroom {id, sha256}` en `fields`. Een
   niet-ingevuld veld staat erin als `null`: ook een onvolledige aanvraag wordt
   vastgelegd. Een tabelregel krijgt elke gedeclareerde kolom, een ontbrekende
   als `null`. Wat niet in de vorm van de stroom past (een onbekend veld, een
   onbekende sleutel in een genest `$external`-object, een onbekende kolom, of
   een lijst waar een enkele waarde hoort) weigert de cel met 400 en het
   veldpad, bijvoorbeeld `organen[1].kleur`. Elke indiening opent een nieuwe
   zaak.

## Controles bij het opstarten

`controle` weigert te starten, met een melding die het veld of de parameter
noemt, als:

1. stroom of celconfiguratie niet valideert tegen het schema;
2. een afleiding wijst naar iets wat niet bestaat: een parameter die geen
   artikel uit de grondslag van het gefilterde event kent, of een veldpad dat
   het event niet heeft, of een tabelafleiding die geen tabelveld leest of een
   kolom die het tabelveld niet declareert;
3. er een weesveld is: een veld dat geen afleiding leest en dat niet in
   `niet_gereduceerd` staat;
4. een parameter meer dan een afleiding krijgt;
5. het `portaal`-blok naar een event, lexostatus of uitkomst wijst die er niet
   is, of naar een uitkomst van een artikel buiten de grondslag van het event.

## Configuratie

| Variabele | Betekenis |
|---|---|
| `REGULATION_PATH` | Map met regelingen; elk YAML-bestand met `$id` en `articles` wordt geladen. |
| `CHRONICLES_PATH` | Een stroombestand, of een map met stroombestanden. |
| `CELL_CONFIG_PATH` | De celconfiguratie: `cel`, `lexostatus_definitions` en `portaal`. |
| `DATA_DIR` | Map voor de kronieken. |
| `AANVRAAG_CEL_PORT` | Poort, standaard 7170. De cel luistert op `0.0.0.0`. |

Het `portaal`-blok in de celconfiguratie:

```yaml
portaal:
  stroom: <$id van de stroom>
  event: <event dat een indiening wordt>
  toets: {lexostatus: <naam>, regeling: <$id>, uitkomst: <output>}
  formulier: {pad: <relatief aan dit bestand>, scherm: <id>}   # optioneel
```

Het formulierbestand levert alleen labels, soorten en volgorde. Een veld dat
het formulier niet kent, krijgt zijn veldnaam; een veld dat de stroom niet
kent, wordt overgeslagen. Voor de kolommen van een tabelveld geldt hetzelfde,
en of een veld een tabel is, bepaalt de stroom.

## Modules

| Module | Taak |
|---|---|
| `stroom` | stroomdefinitie laden en valideren, gram bouwen uit intake en external |
| `reductie` | celconfiguratie laden, kroniek reduceren tot lexostatus |
| `kroniek` | append-only opslag en lezen per zaak |
| `controle` | de controles bij het opstarten |
| `eherkenning` | nep-login (KvK, gemachtigde, machtiging `volledig`) en sessies |
| `toets` | lexostatus als parameters aan de engine, een uitkomst evalueren; `ontbreekt` noemt de aanwezigheidsafleidingen (`gevuld`, `tabel` met `elke_regel`) die onwaar zijn |
| `api` | de routes |
| `regelingen`, `formulier`, `schema`, `config` | laden en valideren |

## De engine en een losse uitkomst

De engine voert bij een gevraagde uitkomst het hele artikel uit: elke actie en
elke invoer, ook wat die uitkomst niet nodig heeft. Een toets is dus alleen te
beoordelen als alles wat het artikel aanraakt aanwezig is, ook feiten van de
instantie zelf. De cel vult dan niets aan en meldt "niet te beoordelen: mist
<parameter>". De test `engine_eist_het_hele_artikel_bij_een_uitkomst` in
`src/toets.rs` legt dit gedrag vast.
