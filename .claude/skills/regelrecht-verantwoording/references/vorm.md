# De vastleggingsvorm, veld voor veld

Een claim is een **procesrelatief feit**: niet "de premie is X", maar "op moment T
heeft actor A vastgesteld dat de premie X is, op deze grond". Dat onderscheid is
de hele truc. Een toestand die je overschrijft verliest haar geschiedenis; een feit
dat je toevoegt niet.

Een herziening is dus een **nieuw** feit dat naar het vorige verwijst, nooit een
wijziging van het bestaande. Wie dat omdraait, krijgt vanzelf veldnamen als
`herziening_2026_08_12` — een datum in een sleutel is het signaal dat het schema
geen plek heeft voor "op dat moment gebeurde dit".

## De velden

| veld | verplicht | wat erin staat |
|---|---|---|
| `kwestie` | ja | de vraag, in de woorden van de tekst zelf; citeer waar het op aankomt |
| `lezing` | ja | de gekozen uitleg, in één zin te begrijpen |
| `grond` | ja | wettekst · wetsgeschiedenis · systematiek · uitvoeringspraktijk. Meer dan één mag |
| `alternatieven` | ja, ≥1 | de verworpen lezing, **mét gevolg** |
| `effect` | ja | wie merkt het, en welke kant op — begunstigend of belastend |
| `stand` | ja | `voorgesteld` · `tijdelijk_vastgesteld` · `uitgesproken` · `bewaakt` |
| `door` | ja | wie deze stand zette |
| `voorstel_van` | als het van een model kwam | `model` of `mens` |
| `bevoegd` | vanaf `tijdelijk_vastgesteld` | wie moet spreken |
| `uiterlijk` | vanaf `tijdelijk_vastgesteld` | een mijlpaal, geen datum |

`voorstel_van` naast `door` lijkt een detail en is het niet: het maakt meetbaar hoe
vaak een analist een voorstel van een model ongewijzigd overneemt. Dat is
automation bias, en zonder dit veld blijft het een zorg in plaats van een getal.

`uiterlijk` verwijst naar een mijlpaal en niet naar een datum, omdat een datum
verschuift en een mijlpaal een gebeurtenis is.

## Voorbeeld — de logische vorm

Dit is hoe je over een claim *denkt*. De **opslagvorm** is de notitie in `SKILL.md`, stap 3:
dezelfde velden, maar als bodies van één annotatie, omdat het notitieschema geen eigen velden
voor kwestie, lezing of alternatief kent. Schrijf nooit de vorm hieronder letterlijk in
`annotations.yaml`; hij valideert niet.

Een open term in de zorgtoeslagketen die van buiten wordt gevuld:

```yaml
kwestie: >-
  De regeling noemt de standaardpremie zonder te zeggen op welk moment in het
  jaar zij wordt vastgesteld; twee lezingen leiden tot een ander bedrag voor wie
  in de loop van het jaar instroomt.
lezing: peildatum is 1 januari van het berekeningsjaar
grond:
  - systematiek: de omliggende bepalingen rekenen alle op jaarbasis
  - uitvoeringspraktijk: zo wordt het nu ook uitgevoerd
alternatieven:
  - lezing: peildatum is het moment van aanvraag
    gevolg: >-
      Instromers krijgen een ander bedrag dan wie het hele jaar verzekerd is;
      belastend voor wie later in het jaar instroomt.
effect: neutraal voor het jaargeval, belastend voor instromers onder de tweede lezing
stand: tijdelijk_vastgesteld
door: analist
voorstel_van: model
bevoegd: de normsteller
uiterlijk: mijlpaal-2
```

De uitspraak is daarna een eigen feit dat hiernaar terugwijst:

```yaml
stand: uitgesproken
door: de normsteller
verdict: bekrachtigd        # of: gecorrigeerd · weerlegd · onbeslist_gelaten
reden: ...                  # verplicht bij onbeslist_gelaten
over: <verwijzing naar het feit hierboven>
```

## Waar dit terechtkomt

Bij de tekst waar het over gaat, als stand-off notitie — niet in een eigen bestand. Het
ritme waarin dat gebeurt (werkronde, uitspraak, verwerken, mijlpaal) is van `regelrecht-traject`. Het notitieschema kan vandaag nog niet alles hierboven dragen: `workflow`
kent alleen `open` en `resolved`, er is geen bevoegde en geen termijn, en de body
is één string of één verwijzing, dus gestructureerde alternatieven passen er niet
in. Tot dat opgelost is: leg de kwestie, de lezing, de grond en het alternatief in
de body, en de stand als tag uit een vocabulaire.

Dat is een werkhypothese van de methode zelf, en zij valt onder dezelfde regel als
elke andere: hij is vastgelegd, hij heeft een bevoegde, en hij moet dicht.
