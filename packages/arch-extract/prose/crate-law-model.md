---
node: crate:law-model
fingerprint: c87c2981628fba66
---
**Wat.** De canonieke Rust-representatie van het wet-YAML-documentformaat:
artikelen, velden, bronnen en verwijzingen als getypeerd model.

**Waarom.** Het hand-geschreven `schema.json` is het taal-agnostische contract;
`law-model` is één implementatie die daaraan moet *conformeren* (geen van beide
wordt uit de ander gegenereerd). Door het model apart te houden van de engine
kunnen meerdere consumenten (engine, editor, tooling) dezelfde wet-structuur
delen.
