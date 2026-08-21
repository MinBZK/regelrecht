---
node: crate:editor-api
fingerprint: d613f3d0baa19cc4
---
**Wat.** Een lichte Axum-server die als backend dient voor de law-editor
frontend, inclusief een proxy naar de harvester-admin API
(`/api/harvest-admin/*`).

**Waarom.** Houdt de editor-frontend dun: bewerken, valideren en het benaderen
van corpus- en harvest-functionaliteit lopen via één backend, met SSO ervoor.
