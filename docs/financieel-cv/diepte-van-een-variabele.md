# Hoe diep gaat een variabele

**Datum:** 23 september 2026 · **Uitgewerkt voorbeeld:**
`is_uitgesloten_beschut_werk_pwet_10b`, de chapeau-uitsluiting van Wfsv 38b

Een parameter in een `machine_readable`-blok is een bodem die je zelf legt. Die
bodem kan op vier plaatsen liggen, en waar hij ligt bepaalt of een uitkomst
herleidbaar is tot de wet of tot de aanleveraar. Dit document trekt één
variabele helemaal uit, omdat de keuzes daaronder voor alle 120 gegevens
hetzelfde zijn.

## De ladder

| Niveau | Wat er ligt | Stand |
|---|---|---|
| 0 | Wfsv 38b leest de waarde als kale parameter, bron "gemeente" | vandaag |
| 1 | Participatiewet 10b berekent `is_uitsluitend_aangewezen_op_beschut_werk` | gemodelleerd, niet aangesloten |
| 2 | Pwet 10b rust op drie parameters, alle drie vaststellingen | bodem van de wettekst |
| 3 | Pwet 10b lid 2: het UWV adviseert op grond van bij AMvB gestelde regels | open term, AMvB niet in het corpus |
| 4 | Besluit SUWI 3.2 lid 3: de registratie eindigt de dag ná de vaststelling | ingewonnen, niet gemodelleerd |

De drie parameters van niveau 2 zijn `behoort_tot_doelgroep_10b_lid_1`,
`college_heeft_vastgesteld_uitsluitend_beschut_werk` en
`heeft_dienstbetrekking_beschut_werk`.

## Wat niveau 1 aansluiten kost

De Ziektewet doet dit al, in artikel 29b:

```yaml
- name: verricht_arbeid_in_beschut_werk
  type: boolean
  source:
    regulation: participatiewet
    output: verricht_arbeid_in_beschut_werk
    parameters:
      bsn: $bsn
      behoort_tot_doelgroep_10b_lid_1: $behoort_tot_doelgroep_10b_lid_1
      college_heeft_vastgesteld_uitsluitend_beschut_werk: $is_uitgesloten_beschut_werk_pwet_10b
      heeft_dienstbetrekking_beschut_werk: $heeft_dienstbetrekking_beschut_werk
```

Daar staat de prijs in. Een `source`-aanroep haalt de waarde uit de bronwet, maar
de parameters van die bronwet moet de aanroeper alsnog meeleveren. Voor Wfsv 38b
betekent dat: één parameter eruit, drie erin. De parameterlijst groeit van
vijftien naar zeventien.

Dat is geen argument tegen de aanroep, maar het weerlegt wel de verwachting dat
dieper modelleren vanzelf minder invoer oplevert. Wat het oplevert is
**herleidbaarheid**: de uitsluiting rust dan op het gemodelleerde artikel met
zijn `legal_basis`, en niet op een boolean waarvan alleen de naam zegt waar hij
vandaan komt. De invoer verschuift van een conclusie naar de feiten waaruit die
conclusie volgt, en dat is precies de verschuiving die een trace leesbaar maakt.

Minder invoer ontstaat pas een niveau lager, wanneer de AMvB van niveau 3 de
vaststelling zelf berekent. Zolang die ontbreekt, verplaatst de aanroep het
oordeel en neemt het niet weg.

## De asymmetrie die hieruit volgt

Twee vaststellingen staan naast elkaar in hetzelfde stelsel, en zijn verschillend
kenbaar:

| | Criterium vastgelegd? | Waar |
|---|---|---|
| Kan de persoon het WML verdienen? (Wfsv 38b gronden a en e) | **ja** | Besluit SUWI 3.5: het UWV toetst het arbeidsvermogen aan drempelfuncties; lid 6 en 7 geven de beslisregel, met een houdbaarheid van ten minste zes maanden |
| Is de persoon uitsluitend aangewezen op beschut werk? (Pwet 10b lid 1) | **nee** | Pwet 10b lid 2 delegeert naar een AMvB die niet in het corpus staat |

Beide beslissingen sluiten iemand in of uit het doelgroepregister. De eerste kan
een burger natrekken, de tweede niet. Dat verschil staat in geen van beide
wetteksten en valt pas op wanneer je de ladder uittrekt.

Dit scherpt beslispunt B9. De vraag is niet alleen of "naar het oordeel van het
college" toetsbaar is, maar waarom de ene vaststelling een gepubliceerde
methodiek heeft en de andere niet.

## Wat dit betekent voor de andere 119 gegevens

De ladder is per gegeven anders diep, en drie vragen bepalen hoe diep:

1. **Bestaat er een wet die deze waarde produceert, en staat die in het corpus?**
   Zo ja, dan is niveau 1 een `source`-aanroep en verder niets.
2. **Delegeert die wet het criterium naar lagere regelgeving?** Zo ja, dan bepaalt
   de aanwezigheid van die regeling in het corpus of niveau 3 bereikbaar is.
3. **Is wat overblijft een oordeel of een feit?** Een feit kan een gegevenslevering
   worden. Een oordeel blijft een oordeel, hoe diep je ook graaft; dan is de
   winst dat de vindplaats bekend is, niet dat het gegeven verdwijnt.

De indeling in [`gegevensherkomst.md`](gegevensherkomst.md) beantwoordt vraag 3
voor alle 120. Vraag 1 en 2 zijn per gegeven nog niet beantwoord.
