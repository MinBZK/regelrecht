# Fixture en gate-tests

```bash
bash tests/run.sh
```

Bewijst dat elke gate zijn eigen defect pakt. Een gate die faalt op een ándere reden dan de
zijne telt als testfout — anders zou een kapotte gate groen kunnen lijken zolang er *iets*
misgaat.

## De fixture

`canonical.md` is een **verzonnen** toelichting op een **verzonnen** regeling, in een
**verzonnen** norm-corpus. Dat is een harde eis, geen gemak: deze repo is publiek, en de
leak-guard `script/check-skills-no-casus.sh` bewaakt dat skills dossier-agnostisch blijven.
Gebruik nooit een echt dossier-document, echt citaat of echte organisatienaam in deze map.

Het document is klein maar bevat met opzet elk ding dat de methode moet aankunnen:

| In de fixture | Waarom |
|---|---|
| een disclaimerzin ("geen rechten worden ontleend") | documentstatus die alle statements erin bindt |
| een colofon en een titelregel | segmenten die `informative` / `navigational` moeten worden |
| "in de regel ten hoogste 1.200 euro" | `soft-default`: de bindendheid-vervlakking-val |
| een tweede vrijlating die in geen norm staat | `niet-gevonden` → LETTER-vs-TOELICHTING |
| het woord "vrijgelaten" dat twee keer voorkomt | maakt een ambigu anker mogelijk |
| een gevolg-zin met "Indien ..." | normzin die het signaalnet moet redden als de lezer hem mist |

## De twee ledgers

`statements.clean.yaml` betegelt `canonical.md` voor 100% en haalt alle vier gates. Het is
ook het uitgewerkte voorbeeld van het ledger-formaat: segmenten met `disposition`, statements
met anker, verankering, bindendheid en bucket.

`statements.broken.yaml` is hetzelfde bestand met vijf opzettelijke defecten:

| | Defect | Gate |
|---|---|---|
| A | S4 citeert "twintig procent" waar het document "20%" zegt | `verbatim` |
| B | het colofon-segment ontbreekt | `coverage` |
| C | S5 ankert op het kale woord "vrijgelaten" (twee treffers) | `anchor` |
| D | S8 ontbreekt, de "Indien ..."-zin is stil overgeslagen | `signaalnet` |
| E | S3 noteert `niet-gevonden` zonder zoektermen | `verbatim` |
| F | S6 draagt een `bindingness` buiten het vocabulaire | `ledger` |

A en E zitten allebei op de verbatim-gate omdat ze dezelfde belofte breken: wat het register
beweert over de tekst is niet na te lopen. D is de belangrijkste van de zes — het is de enige
die niemand opmerkt zonder gate, want het register ziet er volledig uit.

F is er bijgekomen nadat bleek dat een ledger met vier verzonnen vocabulaire-waarden schoon
door alle gates kwam. De ernstigste variant is een typefout in `anchoring.status`: de
zoektermen-eis kijkt naar de string `niet-gevonden`, dus `nietgevonden` schakelt hem uit
zonder een woord. Daarom draait de ledger-gate bij élke aanroep, en test `run.sh` dat ook
apart — een losse `anchor`-aanroep moet nog steeds de LEDGER-regel tonen.

## Bij het aanpassen van de fixture

Verander je `canonical.md`, dan moeten beide ledgers mee: de segmentteksten worden letterlijk
in de canonieke tekst gezocht en moeten elkaar aansluitend opvolgen. Draai daarna
`tests/run.sh` én yamllint (`--strict -c .yamllint`) — de fixture-YAML's vallen onder dezelfde
pre-commit-hooks als de rest van de repo.
