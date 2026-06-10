# Casus-assen — van platte leaf-lijst naar dimensies

Een endpoint met veel leaf-parameters geeft een baseline van tientallen platte booleans
(meestal allemaal op de neutrale waarde) waarin elk scenario er een paar flipt. Die
lijst is niet scanbaar: je vindt een casus niet terug, en je ziet niet welke combinaties
betekenisvol zijn. De oplossing is **dimensie-reductie**: groepeer de leafs tot een
handvol onafhankelijke *casus-assen*.

## Waarom assen werken

De meeste leafs zijn niet vrij-combineerbaar. Ze clusteren rond een paar onderliggende
vragen over de persoon/situatie, en binnen een cluster zijn waarden vaak wederzijds
uitsluitend of alleen samen relevant. Een casus is dan een **coördinaat** op enkele
assen in plaats van een speld in een hooiberg van losse vlaggen.

## Een as herkennen

Een as is een groep leafs die samen één onderliggende vraag beantwoorden. Heuristieken:

- **Gedeeld onderwerp** — leafs die over hetzelfde gaan (status, relatie, plaats, een
  registratie, een tijdvak) horen op één as.
- **Wederzijdse uitsluiting** — als hoogstens één van een groep tegelijk waar kan zijn,
  is die groep waarschijnlijk één as met meerdere waarden.
- **Gezamenlijke relevantie** — leafs die alleen iets doen in elkaars aanwezigheid
  (A telt alleen mee als B) horen bij elkaar.
- **Voedt dezelfde tussen-norm** — leafs die in de YAML naar dezelfde tussen-knoop leiden
  (zie `keten-checkpoints.md`) vormen een natuurlijke as.

Mik op een **klein** aantal assen (richtlijn: een handvol). Te veel assen = je hebt de
platte lijst alleen hernoemd; te weinig = je verliest betekenisvol onderscheid.

## Twee soorten assen

- **Categorische as** — één-uit-N waarden (een statussoort, een relatietype). Modelleer
  als één keuze, niet als N losse booleans waarvan er meerdere tegelijk waar kunnen
  staan (dat zou onmogelijke casussen toelaten).
- **Onafhankelijke vlag** — een echt losstaand ja/nee dat met alles combineert. Houd
  deze apart; dwing ze niet in een as.

## De baseline groeperen (de goedkoopste winst)

Nog vóór je persona's bouwt: herorden de baseline-parameterlijst per as, met een kopje
per as. Dezelfde 60 booleans worden dan 6 leesbare groepen. Dit is puur volgorde +
commentaar — geen gedragswijziging — en maakt de lijst meteen scanbaar.

```
# === AS: <onderwerp 1> ===
<leaf_a> | <neutrale waarde>
<leaf_b> | <neutrale waarde>

# === AS: <onderwerp 2> ===
...

# === Losse vlaggen ===
...
```

## Onmogelijke-combinatie-bewaking

Door assen expliciet te maken zie je welke combinaties juridisch onmogelijk zijn (twee
elkaar uitsluitende waarden op één categorische as). Markeer die als verboden — een
scenario dat ze toch zet, test een casus die niet bestaat. Dit is ook input voor de
twin-persona's in `persona-bibliotheek.md`: een twin verschilt op precies één as-waarde.

## Resultaat

De assen zijn de coördinaten-taal voor de rest van de skill: persona's worden benoemde
punten in de assen-ruimte (`persona-bibliotheek.md`), en de casus-matrix indexeert op
assen (`templates/casus-matrix.md`). Leg de assen-definitie vast bij het corpus, niet in
de skill.
