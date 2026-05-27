# Schema-migratie {oude versie} → {nieuwe versie}

**Datum**: {datum} · **Scope**: {welke wetten/bestanden}
**Status**: {voltooid lokaal / gepusht}

## Doel

Alle {N} bestanden van schema {oud} naar {nieuw}, validerend onder het nieuwe schema +
de engine.

## Mechanische pass

Zoek-en-vervang-achtige wijzigingen, per bestand:

- {schema-URL bump: `{oud}` → `{nieuw}`}
- {hernoemde operaties: `{A}` → `{B}`}
- {gewijzigde syntax: `{oude vorm}` → `{nieuwe vorm}`}
- {ontbrekende headers toevoegen aan serde-only bestanden}

## Semantische pass

Nieuwe schema-features benutten, per artikel beoordeeld:

- {nieuw veld/structuur} toegepast op {welke artikelen} omdat {reden}
- …

## Discoveries — schema

*Het waardevolste deel: welke aannames over het schema klopten of niet.*

1. {bevinding — bijv. een enum bleek gesloten, een veld zit op een andere plek dan gedacht}
2. …

## Validatie

- `{validatie-commando}` → {N/N OK}
- `{engine-scenario-commando}` → {N/N PASS}

## Open punten

- {wat een vervolg vereist — bijv. een engine-PR voor een ontbrekende operatie}
</content>
