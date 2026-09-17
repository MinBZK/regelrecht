---
id: specificaties-iv-de-ontbrekende-delen-bouwen
titel: 'Specificaties IV: de ontbrekende delen bouwen'
faseId: wat
disciplineId: techniek
prioriteit: hoog
omvang: XL
categorie: bet
capability: basis
capaciteit: analist per concept, meerdere concepten tegelijk (kanban), externe
  juridische expertise per concept
toelichting: |-
  Wat Specificaties III aan gaten oplevert wordt hier verdeeld in blokken, en per
  blok geleverd als schema, engine, conformance-scenario's en een geversioneerd
  document dat zegt wat de taal nu kan. Een blok is af als een tweede
  implementatie er genoeg aan heeft om hetzelfde te doen.

  De werkverdeling is kanban: één analist per concept, meerdere concepten
  tegelijk, en per concept externe juridische expertise erbij om te toetsen of
  het concept het recht raakt dat het zegt te raken.

  **Kandidaten op grond van de huidige stand**

  Tijd en wetshistorie, waar een verwijzing nu geen datum kent en het corpus geen
  geschiedenis. Datumonderdelen en afkapping. Markeringen en open normen, in het
  verlengde van de splitsing uit Specificaties II. De procedurele kant van de Awb,
  die deels staat. Engine policy, dat nog geen regel heeft. En de aansluiting van
  schema op model: de conformance-suite meet nu vier vormen die het schema afwijst
  en het model accepteert, grotendeels door dezelfde oorzaak, ongetagde enums en
  ontbrekende strengheid op onbekende velden.

  Deze lijst is de stand van vandaag. De volgorde volgt uit wat de enricher op
  schaal tegenkomt, want een concept dat in honderd wetten ontbreekt gaat voor een
  concept dat in één wet ontbreekt.
volgorde: 1300
onderzoeksvragen:
  - Wat maakt een blok af? Een wijziging aan de taal die geen conformance-scenario
    achterlaat, is voor een tweede implementatie niet te volgen.
  - Hoe groot mag een blok zijn voordat het niet meer door één analist met één
    jurist te overzien is?
  - Waar laten we het model bewust ruimer dan het schema, en waar is dat een fout
    die hersteld moet worden?
  - vraag: >-
      Welke informatie over bevoegdheden, delegaties en tijdstippen moet het
      systeem vastleggen om technische controle op een gecodeerde wet mogelijk te
      maken?
    paper: sec:depgraphs
onderzoek: open
bouw: deels
rfcs:
  - 8
  - 15
  - 20
  - 30
  - 31
  - 32
samenhangIds:
  - specificaties-i-documentatie-op-orde
  - specificaties-iii-gaten-vinden-met-de-enricher
  - specificaties-v-beproeving-van-de-taal
---
