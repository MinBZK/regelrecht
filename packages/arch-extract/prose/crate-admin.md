---
node: crate:admin
fingerprint: 9af5324067d92548
---
**Wat.** De harvester-admin API: een standalone Axum-service voor harvest-jobs en
corpus-beheer. De bijbehorende dashboard-UI leeft inmiddels in de editor (de
sectie "Corpusinwinning"), bereikt via de editor-api-proxy.

**Waarom.** De API blijft onafhankelijk aanspreekbaar voor scripts en services,
ook nu de UI verhuisd is — zodat geautomatiseerde consumenten niet via de editor
hoeven te gaan.
