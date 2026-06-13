# Golden traces — de volledige keten vastleggen tegen regressie

Keten-checkpoints (`keten-checkpoints.md`) asserten de knopen die je *bewust* hebt
gekozen. Een **golden trace** legt de *volledige* engine-trace per scenario vast — alle
tussen-knopen + welke tak vuurde — als snapshot. Een wijziging die de endpoint én je
checkpoints ongemoeid laat maar elders in de keten iets verschuift, wordt dan een
trace-diff. Dit is de sterkste verdediging tegen stille ketenfouten.

## Wat je vastlegt

Per scenario een genormaliseerd trace-snapshot: de geëvalueerde knopen, hun waarden, en
welke tak/conditie waar werd. Normaliseer wat ruis is (volgorde, timestamps, interne
ids) zodat de diff alleen betekenisvolle veranderingen toont. Sla de snapshots bij het
corpus op (bijv. naast de scenario's), niet in de skill.

## De workflow

1. **Genereer** de trace via de runner/engine voor elk scenario.
2. **Normaliseer** tot een stabiele, diff-bare vorm.
3. **Review & commit** de snapshot als "golden" zodra je 'm hebt geverifieerd tegen de
   verwachte keten (niet blind: een golden trace is pas goud na menselijke/expert-check).
4. **Diff** bij elke YAML-/feature-wijziging. Een onverwachte diff = mogelijk een
   ketenfout; een verwachte diff = bewust herijken (update de golden + leg uit waarom).

> Een golden trace die je nooit tegen de wettekst hebt geverifieerd, bevriest alleen je
> huidige interpretatie. Verifieer vóór je 'm tot goud verklaart — anders is het
> regressie-bescherming van een mogelijk foute keten (zie de meta-check in
> `keten-checkpoints.md`).

## Branch-coverage over de wet-logica

De trace vertelt ook welke **takken** je scenario's raken. Voor elke conditie/operatie in
de YAML: wordt zowel de waar- als de onwaar-kant ergens geraakt? Een nooit-geraakte tak
is een plek waar een ketenfout zich ongezien kan verstoppen.

- Inventariseer de takken per endpoint (uit de keten-kaart).
- Markeer per tak: geraakt-waar / geraakt-onwaar / ongetest.
- Ongeteste takken → nieuwe persona's of twin-scenario's om ze te dekken.
- Rapporteer de dekking in `templates/golden-trace-review.md`.

## Wanneer wel, wanneer niet

- **Wel**: cross-law-ketens, endpoints met veel takken, alles wat je regressie-bestendig
  wilt houden over cycli heen.
- **Lichter**: een kleine, stabiele keten heeft vaak genoeg aan keten-checkpoints alleen.
  Golden traces kosten onderhoud (elke bewuste wijziging vraagt een herijking) — zet ze
  in waar de regressie-winst dat waard is.

## No silent caps

Snapshot je maar een deel van de scenario's of dek je niet alle takken → log dat
expliciet in het review-rapport. Stille truncatie leest als "alles gedekt" terwijl dat
niet zo is.
