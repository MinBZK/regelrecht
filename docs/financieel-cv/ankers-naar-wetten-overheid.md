# De ankers in `url` wijzen naar een positie die niet bestaat

**Vastgesteld:** 23 september 2026 · **Status:** corpusbrede harvester-bevinding,
geen dossierfout.

## Wat er gebeurt

Elk gemodelleerd artikel draagt een `url` van de vorm
`https://wetten.overheid.nl/<bwb_id>/<peildatum>#Artikel<nummer>`. Dat anker
bestaat niet op de pagina waarnaar het verwijst. Wetten.overheid.nl zet voor elk
anker het structuurpad, dus hoofdstuk, afdeling en paragraaf, en het corpus laat
dat weg.

Gemeten op 23 september 2026 tegen de geldende pagina's:

| Wet | Anker in het corpus | Werkelijk `id` op de pagina | Kaal anker gevonden |
|---|---|---|---|
| Ziektewet 29b | `#Artikel29b` | `AfdelingTweede_HoofdstukII_Artikel29b` | 0 |
| Participatiewet 10d | `#Artikel10d` | `Hoofdstuk2_Paragraaf2.1_Artikel10d` | 0 |
| Wfsv 38b | `#Artikel38b` | `Hoofdstuk3_Afdeling4_Paragraaf4a_Artikel38b` | 0 |
| Wajong 2:20 | `#Artikel220` | `Hoofdstuk2_Afdeling5_Artikel2:20` | 0 |

Drie verschillende nummerstijlen, drie keer hetzelfde resultaat. Het raakt dus
niet een wet of een notatie, maar elk anker in het corpus.

## Wat het gevolg is

De link werkt: de browser opent de juiste regeling op de juiste peildatum. Hij
springt alleen niet naar het artikel, maar blijft bovenaan de pagina staan. Bij
de Participatiewet is dat enkele honderden artikelen scrollen.

Voor een walk-through met een jurist is dat de plek waar het opvalt, want daar
wordt een verwijzing natrekken hardop gedaan.

## Wat het níét is

**Geen gevolg van de dubbelepuntnotatie.** De Wajong-variant `#Artikel220` valt
extra op omdat het nummer ook nog verminkt oogt, maar de Ziektewet en de
Participatiewet, die geen dubbele punt kennen, hebben exact hetzelfde probleem.
Zie [`pyyaml-valkuil.md`](pyyaml-valkuil.md) voor waar die verwarring vandaan
komt: PyYAML leest `2:20` als 140, en dat leidde eerder tot de onjuiste
conclusie dat de Wajong-nummers in het bestand fout stonden.

**Geen dossierfout.** De acht bestanden van het Financieel CV doen hetzelfde als
de rest van het corpus. Repareren in dit dossier zou een afwijking maken.

## Wat eraan te doen is

De ankervorm wordt door de harvester geschreven, dus daar hoort de reparatie.
Twee dingen zijn nodig:

1. **De harvester neemt het structuurpad over** uit de bron in plaats van alleen
   het artikelnummer. Het pad staat in de geconsolideerde weergave zelf, in het
   `id`-attribuut waar het anker naar verwijst.
2. **Een controle op de uitvoer.** Een anker is met een netwerkcontrole te
   toetsen: haal de pagina op en kijk of het `id` erin voorkomt. Dat is dezelfde
   soort poort als de bewijs-poort op de engine-limitaties: een verwijzing telt
   pas als zij is nagegaan.

Tot die tijd is de peildatum in de URL het bruikbare deel, en het anker niet.

## Hoe dit is vastgesteld

Per wet de geldende pagina opgehaald en geteld hoe vaak het kale anker als `id`
voorkomt, en welke volledige vorm er wel staat. Nul treffers op het kale anker in
alle vier de gevallen, telkens één treffer op de vorm met structuurpad.

Deze controle stond eerder als open punt genoteerd met de reden dat er geen
netwerk zou zijn. Dat was onjuist: `wetten.overheid.nl` is vanuit deze omgeving
bereikbaar.
