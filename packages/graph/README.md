# regelrecht-graph — de corpusgraaf

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

## Waarom een eigen crate en geen bin in `pipeline`

De voor de hand liggende plek was `packages/pipeline/src/bin/`, naast de
workers. Dat is hier niet gedaan, om drie redenen.

De bouwstap heeft geen database nodig. Hij is een pure functie van een
corpus-checkout naar een bestand: dezelfde invoer geeft dezelfde bytes, en dat
is precies de eigenschap waar de stabiliteitseis op rust. In `pipeline` zou hij
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
| `graph_metric(node_id, in_degree, out_degree, pagerank_rev, cluster_id, is_framework_law)` | `nodes[].weight`, `out`, `rank`, `cluster`, `framework` |
| `graph_cluster(cluster_id, size, dominant_law)` | af te leiden uit `nodes[].cluster` plus `rank` |
| `snapshot_id` | de header; een inhoudshash over knopen, posities en kanten, dus twee gelijke bouwsels krijgen hetzelfde id |

Wat er nog niet is en wat het ontwerp wel noemt: `graph_marking` en
`graph_open_term`. Het geoogste corpus is nog nergens verrijkt, dus die twee
tabellen zouden vandaag leeg zijn. De kanttypen die erbij horen (`source`,
`delegation`, `expected_delegation`) worden wel gebouwd en gewogen, en zijn in
de tests gedekt tegen een corpus dat ze wel heeft.

## Het formaat

Zie de moduledocumentatie van [`payload`](src/payload.rs). Daar staat de
bytelayout, de sectietabel en een klein leesbaar voorbeeld.
