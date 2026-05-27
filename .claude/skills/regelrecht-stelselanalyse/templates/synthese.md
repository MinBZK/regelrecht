# Synthese expert-review — {datum}

**Reviewers**: {N parallelle assen/agents} · **Scope**: {assen}
**Resultaat in één zin**: {bijv. X gefixt + Y gedocumenteerd, rest naar volgende cyclus}

## Wat is gefixt deze ronde

- **{KRITIEK/MINOR} — {titel}** (as {x}): {bug + fix in één alinea}. Commit `{ref}`.
  Tests blijven groen ({N/N}).

## Wat is gedocumenteerd, niet gefixt

| As / agent | Aantal | Classificatie | Discovery |
|---|---|---|---|
| {as} | {n} | {classificatie} | {korte omschrijving} |

## Discoveries uitgelicht

### D1 — {titel}
{Wat, op basis waarvan, en het gevolg.}

## Meta-bevindingen

- **Features vs YAML**: {maken ze dezelfde fout? consequentie voor de BDD-suite}.
- {andere stelsel-brede observatie}

## Telling (geactualiseerd)

{Aantal wetgevings-fouten per categorie/ernst na deze ronde. Consistent houden met de
fouten-analyse en het eindrapport.}

## Engine-correctheid behouden

- `{validatie-commando}` → {N/N OK}
- `{scenario-commando}` → {N/N PASS}

---

# Heroverweging twijfel-claims *(zelfde of apart document)*

Per onzekere bevinding: houdt de claim stand?

| Claim | Oordeel | Gevolg |
|---|---|---|
| {claim} | {gehandhaafd / weerlegd / verplaatst} | {telling −1 / categorie gewijzigd / —} |

**Geschrapte claims** blijven zichtbaar in de bronnen-documenten (doorgestreept + reden);
tellingen aangepast. Geen stille verwijdering.
</content>
