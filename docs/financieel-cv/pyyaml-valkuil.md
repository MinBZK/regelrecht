# De PyYAML-valkuil bij artikelnummers met een dubbele punt

**Vastgesteld:** 9 september 2026 · **Status:** geen corpusfout, wel een
gereedschapsval die tot een onterechte bevinding heeft geleid.

## Wat er gebeurt

De Wajong en de Awb nummeren hun artikelen met een dubbele punt: `2:20`,
`3:46`, `6:7`. In **YAML 1.1** is zo'n token een getal in **grondtal 60** — het
oude sexagesimale type voor tijdsduren. PyYAML implementeert YAML 1.1, dus:

```python
>>> yaml.safe_load('n: 2:20')['n']
140          # 2 x 60 + 20
>>> yaml.safe_load('n: 3:40')['n']
220
>>> yaml.safe_load('n: 1a:1')['n']
'1a:1'       # ontsnapt: geen geldig getal
```

De formule is `hoofdstuk x 60 + artikel`. Nummers met een letter erin
(`1a:1`, `3:66a`, `2:34a`) ontsnappen, want die zijn geen geldig getal — vandaar
dat een bestand er half verminkt uitziet in plaats van helemaal.

## Waarom dit geen corpusfout is

De bestanden op schijf zijn correct: daar staat `- number: 2:20`. De engine leest
ze met `serde_yaml`, en dat implementeert **YAML 1.2**, waar de sexagesimale
notatie is afgeschaft. Daar is `2:20` gewoon de string `"2:20"`.

Aangetoond met een kopie van het Wajong-bestand waarin één `number: 3:40` is
vervangen door de echte integer `340`:

```
OK:   .../wajong-orig.yaml (schema v0.5.4)
FAIL: .../wajong-int.yaml
  - /articles/118/number: 340 is not of type "string"
```

Het schema eist `"type": "string"` voor `number`, en `Article.number` is `String`
in `packages/law-model/src/model.rs`. Zou het bestand een integer dragen, dan zou
validatie falen. Dat doet het niet.

## Waarom het gevaarlijk is

Niet alleen omdat de nummers fout worden — **ze botsen met bestaande nummers.**

| PyYAML leest | Werkelijk artikel | Maar `220` bestaat ook als |
|---|---|---|
| `140` | Wajong 2:20 | — |
| `142` | Wajong 2:22 | — |
| `220` | Wajong 3:40 | een eigen, ander artikel |

Een analyse die op die getallen zoekt vindt dus artikelen die er niet zijn, en
kan uitkomen bij een artikel dat inhoudelijk niets met de vraag te maken heeft.

## Wat het heeft gekost

Op 8 september is op basis hiervan gemeld dat de `implements`-koppeling van het
Reïntegratiebesluit naar Wajong art. 2:22 stil doodloopt, omdat dat artikel
`142` zou heten. Die bevinding is **onjuist** en op 9 september ingetrokken; zie
`relaties-per-regeling.md`, bevinding 1. Ook de bewering dat Awb 3:46, 6:7 en 6:8
"niet onder die nummers bestaan" was fout — ze bestaan alle drie.

## Hoe je het vermijdt

Bij analyse van dit corpus vanuit Python, één van beide:

1. **Lees `number` uit het anker.** Elk artikel draagt een `url` van de vorm
   `https://wetten.overheid.nl/BWBR…/…#ArtikelNNN`. Dat anker is niet door de
   parser aangetast en geeft het werkelijke nummer.
2. **Gebruik een YAML 1.2-lezer.** `ruamel.yaml` met `YAML(typ='safe')` leest de
   1.2-regels en laat `2:20` een string.

Een snelle zelfcontrole: als een wet met dubbele-punt-nummering opeens integers
in `number` heeft, lees je met de verkeerde parser.

## Welke wetten dit raakt

| Wet | Artikelen met dubbele punt |
|---|---|
| Algemene wet bestuursrecht | 176 van 565 |
| Wajong | 51 van 173 |
| Ziektewet, Participatiewet, Wtl, WW, Wfsv | geen — ongevoelig |

De rest van het corpus is niet onderzocht; ga ervan uit dat elke wet met
hoofdstuk-gebonden nummering hetzelfde gedrag vertoont.
