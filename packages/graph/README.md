# regelrecht-graph, de corpusgraaf

De datalaag onder de corpusgraaf van issue #1082. Leest een corpus-checkout,
maakt er knopen en kanten van, rekent centraliteit, gemeenschappen en een
3D-layout uit, en schrijft een payload die een renderer rechtstreeks in typed
arrays kan lezen.

```bash
cargo run --release -p regelrecht-graph --bin regelrecht-graph-build -- \
    --corpus ../../regelrecht-corpus --out corpusgraaf.rrgraph
```

`--format json` geeft dezelfde graaf leesbaar; `--articles` voegt de
artikelknopen toe; `--help` geeft de rest.

## Plaats in de repository

De voor de hand liggende plek was `packages/pipeline/src/bin/`, naast de
workers. Deze crate staat er los van, om drie redenen.

De bouwstap heeft geen database nodig. Hij is een pure functie van een
corpus-checkout naar een bestand: dezelfde invoer geeft dezelfde bytes, en de
stabiliteitseis vraagt niets anders. In `pipeline` zou hij
`sqlx`, `testcontainers` en een draaiende Postgres binnen bereik hebben en zou
die scheiding binnen een maand vervagen.

De bouwstap heeft de engine niet nodig. `pipeline` hangt aan
`regelrecht-engine` met de `validate`-feature; deze crate hangt aan serde,
`serde_yaml_ng` en `walkdir` en verder aan niets. Dat scheelt in de
testomlooptijd en het houdt de graaf los van schemawijzigingen die de engine
raken maar de verwijzingsstructuur niet.

En er komen meerdere afnemers. De endpoints uit sectie 6 van het ontwerp landen
in editor-api of admin, de bouwstap hoort in een nachtelijke job, en de
lijstweergave leest dezelfde knopen. Een bibliotheek met een dunne binary
erbovenop bedient die alle drie; een bin in `pipeline` bedient er één.

De binary staat wel in de workspace en draait in `cargo test`, dus herhaalbaar
en getest is hij evengoed.

## Grenzen aan de layout

De kaart mag berekenbaar gemaakt worden en niet mooier dan het recht is. Die
regel sluit een paar aantrekkelijke ingrepen uit.

Er is geen demping van verwijzingen naar veelaangehaalde wetten en geen aparte
behandeling van sterknopen. De Awb komt op het echte corpus uit op de
0,1e percentiel van de straal, dus vrijwel in het midden, en dat is waar
867 verwijzingen hem zetten. Verzwak je die kanten om de kaart leesbaarder te
maken, dan verdwijnt de afstand tot dat midden. Die afstand is informatie: het
bestuursrecht is niet alles, privaatrecht en strafrecht vallen er grotendeels
buiten, en waar die uitkomen wil een jurist zien.

Twee maatregelen mogen wel aan de rekenkant zitten, omdat ze het antwoord met
rust laten. De spectrale inbedding start op de schaal die de krachten zelf
impliceren, en een stap is begrensd. Geen van beide verplaatst het vaste punt.

Twee andere zijn gebouwd, gemeten en weer weggehaald, want ze veranderden het
antwoord. Logaritmische aantrekking en een logaritmisch gedempte
afstotingsmassa maakten het beeld beter en verschoven de wetten ten opzichte
van elkaar: rangcorrelatie 0,59 tegen het gewone model, dat prima convergeert
zonder.

Er staat ook een hiërarchische initialisatie in, standaard uit. Die plaatst
eerst de gemeenschappen en dan de wetten daarbinnen, en is gebouwd als
convergentiemaatregel. Hij haalt die lat niet: op het geoogste corpus komt hij
na drieduizend iteraties uit op een rangcorrelatie van 0,35 tegen het gewone
model. Hij bereikt dus niet sneller hetzelfde antwoord maar een ander, waarin de
gemeenschappen keurig uit elkaar liggen omdat ze daar zijn neergezet. De vlag
blijft staan om het verschil te kunnen zien, nooit als standaard.

De bouwer print hoe ver de layout is uitgeconvergeerd, want op het echte corpus
is dat niet volledig en dat hoort de lezer te weten in plaats van te moeten
aannemen.

## Kaderwetten

Een kaderwet is een juridische kwalificatie en geen drempelverschijnsel. De
lijst staat in `kaderwetten.yaml` naast het corpus, met per wet het
toepassingsbereik, het herkenningspatroon en wat er meegenomen moet worden. De
bouwer leest hem; wat er niet op staat mist hij, en die beperking is zichtbaar
voor wie het rapport leest. Zie `src/kaderwet.rs` voor de motivering en
`corpus/kaderwetten.yaml` in deze repository voor de vorm.

De tweede route loopt via de data: een `applicability`-kant is wat een kaderwet
maakt, ongeacht het aantal. Uit een geoogst `references`-blok is die relatie
niet af te leiden, dus vandaag levert die route niets op en komt de
kwalificatie van de lijst alleen.

Het aantal verwijzende wetten blijft de bouwer printen, als werkmateriaal voor
wie de lijst vult. Het is geen oordeel.

## Postgres

Er wordt niets in Postgres geschreven. Het ontwerp wil de layout in de database
en dat blijft juist; het is alleen niet wat de graaf vandaag tegenhoudt, en een
bestand is als snapshot even immutabel als een rij-set. Wanneer de tabellen er
komen, is dit de vertaling vanuit de payload:

| ontwerp | komt uit |
|---|---|
| `graph_node(id, node_kind, law_id, version, article_ref, label, …)` | `nodes[]`: `id`, `kind`, `bwb_id`, `valid_from`, `label`, `parent` |
| `graph_edge(src_node, dst_node, edge_type, …)` | `edges[law_edge_count..]` (artikelniveau) |
| `graph_law_edge(src_law, dst_law, edge_type, n)` | `edges[..law_edge_count]`, met `count` als `n` |
| `graph_layout(node_id, x, y, z, parent_id, layout_version)` | `nodes[].x/y/z`, `parent`, plus `layout_version` uit de header |
| `graph_metric(node_id, in_degree, out_degree, pagerank_rev, cluster_id, is_framework_law)` | `nodes[].weight`, `out`, `citers`, `rank`, `cluster`, `framework` |
| `graph_cluster(cluster_id, size, dominant_law)` | af te leiden uit `nodes[].cluster` plus `rank` |
| `snapshot_id` | de header; een inhoudshash over knopen, posities en kanten |

De verrijkingsstatus (`node_enrichment`, `node_articles`,
`node_articles_enriched`, `node_activity`) wordt een eigen tabel, apart van de
layout. Diezelfde scheiding zit al in de payload: die vier secties staan
achteraan en vallen buiten de `snapshot_id`, zodat de status ververst kan worden
zonder de layout aan te raken. De layout kost tientallen seconden en de status
verandert per minuut, dus die scheiding scheelt bij elke verversing een volledige
herberekening.

Twee tabellen uit het ontwerp ontbreken nog, `graph_marking` en
`graph_open_term`. Het geoogste corpus is nergens verrijkt, dus ze zouden vandaag
leeg zijn.

## Formaat

Zie de moduledocumentatie van [`payload`](src/payload.rs). Daar staat de
bytelayout, de sectietabel en een klein leesbaar voorbeeld.
