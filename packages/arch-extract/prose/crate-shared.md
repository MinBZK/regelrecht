---
node: crate:shared
fingerprint: cca90a318914fc2a
---
**Wat.** Canonieke domeintypen en gedeelde utilities die over de crates heen
gebruikt worden.

**Waarom.** De fundering van de laaggraaf: `shared` hangt van geen enkele andere
workspace-crate af, zodat iedereen erop kan bouwen zonder cycles. Eén plek voor
gedeelde typen voorkomt duplicatie en het uiteenlopen van definities tussen
componenten.
