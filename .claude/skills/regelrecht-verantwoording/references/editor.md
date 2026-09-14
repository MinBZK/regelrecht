# De editor — wat er al is, wat gereserveerd is, wat ontbreekt

Deze skill schrijft in het bestand dat de editor leest, en niet via de API. Dat is geen
noodgreep maar de architectuur: skills en editor delen één git-repo, en de
annotaties-sidecar op de trajectbranch is het scharnier. Wat hieronder staat is de kaart van
die aansluiting — gemeten op de code, niet op de bedoeling — zodat een lezer weet wat vandaag
werkt, wat als haak klaarligt, en wat een schemabesluit vergt.

## Het scharnier: de sidecar

| | |
|---|---|
| bestand | `annotations/{law_id}/annotations.yaml`, wortel van de corpusrepo, op de trajectbranch |
| leest | de editor, via `useNotes` → `resolveNotes` (WASM); resolver meldt *found · ambiguous · orphaned* |
| schrijft | de editor via `save_annotations` (`corpus_handlers.rs`), en deze skill via `Edit` |
| regel | **append-only.** Nieuwe items achter de bestaande bytes; nooit herbouwen. De editor dedupliceert op inhoud, valideert het samengevoegde bestand tegen het schema en weigert bij fout. `target.source` moet naar de wet in het pad resolven |
| zichtbaarheid | `regelrecht:visibility: personal` → Postgres, nooit git. Weglaten = publiek = git |
| gezag | wordt **afgeleid** bij tonen: `authoritative` als `creator` het bevoegd gezag is, anders `advisory` / `generated` / `personal` (`notes-and-annotations.md`). Dat is de afleiding die een bevoegde-check nodig heeft — hij bestaat al, alleen niet als poort |

## Wat de methode nodig heeft, en waar het staat

| nodig | bestaat | gereserveerd, zonder lezer | ontbreekt |
|---|---|---|---|
| **claim bij de tekst** | de sidecar; citaat-anker; resolver; federatie; motivation `questioning` (RFC-018 voor open normen) | | de body kan geen structuur dragen: `TextualBody` is één string, `SpecificResource` één link, allebei `additionalProperties: false`. Kwestie, lezing, grond en alternatief zijn daarom losse bodies met een `purpose` |
| **stand van een claim** | `workflow: open \| resolved`; het `tagging`-patroon met een vocabulaire dat waarschuwt en niet faalt (`_vocabulary/ambiguity.yaml`, RFC-018) | | een standen-vocabulaire (`voorgesteld · tijdelijk_vastgesteld · uitgesproken · bewaakt`). Tot dan: tag `stand:…`, geeft een waarschuwing |
| **bevoegde per claim** | `open_terms[].delegated_to` in schema en model — "wie mag deze term invullen" | issue #1300: het veld wordt in geen codepad gebruikt | tag `bevoegd:…` tot het schema een veld heeft |
| **termijn** | | | tag `uiterlijk:<mijlpaal>`. De taken-tabel heeft geen deadline-veld (`taskCategories.js` zegt dat zelf) |
| **verwijzing van uitspraak naar claim** | | | een notitie heeft **geen `id`** in het schema. Een `replying` kan alleen hetzelfde citaat targeten; de koppeling is per tekstpassage, niet per claim |
| **asynchrone uitspraak door de bevoegde** | motivation `replying` in het schema; gezag-afleiding uit `creator` | | verdict-vocabulaire; afdwingen dat de replier bevoegd is |
| **taak voor een rol** | taken bestaan (`job_review`, `job_failed`), zichtbaar in de editor | `tasks.assignee_account_id` is nullable met de comment "NULL = taak voor het hele traject" (migratie 0028) — geen query leest die toestand; `traject_role` heeft de comment "Future roles (reviewer, …)" (migratie 0015) | een wachtrij per rol; een taaktype `uitspraak_gevraagd` (TEXT + CHECK: een gewone migratie) |
| **invulling wijst naar claim** | `implements.gelet_op` | | `implements.verantwoording` — `implements` staat op `additionalProperties: false`. Schemabesluit: RFC-034 |
| **oordeel bij verrijking draagt een grond** | `enrich_review.rs`: één commit per verrijking, oordeel = `approved \| rejected` + optioneel content | PR #1351 noemt "de beoordelingsview met overzicht en herkomst" als vervolg | een veld voor de claim-verwijzing naast `action` |
| **mijlpaal blokkeert op open punten** | `trajects.status = afgerond` | RFC-034: "de poort die aftekenen tegenhoudt […] bestaat alleen in een methodedocument"; issue #1047 | een voorwaarde op afronden. Vandaag heeft `afgerond` nul voorwaarden en blijft een afgerond traject schrijfbaar |
| **trajectvoortgang** | | RFC-035 (gereserveerd) | |

## Grote wetten — gemeten op de Awb

Getest op de publieke corpusrepo (09-09): een claim en een uitspraak op Awb 4:84, geschreven
langs de Werkstroom, valideren tegen het schema, worden door de resolver gevonden, en laten
de bestaande bytes ongemoeid. Twee grenzen kwamen boven:

- **De engine laadt geen wet met meer dan 1.000 artikelen** (`MAX_ARRAY_SIZE`, een
  DoS-bescherming met een test die de bovengrens vastpint). De Awb heeft er 1.745. Elke
  notitie op de volledige Awb meldt daardoor *selector not checked* — in de validator én in
  de editor. Dat is een engine-besluit, geen methode-vraag; hij staat hier omdat wie een claim
  op zo'n wet legt, moet weten dat de resolver zwijgt en niet dat de claim fout is.
- **De fuzzy-zoekslag heeft een budget** (`MAX_FUZZY_SCAN_CHARS`). Exact matchen is
  ongebudgetteerd en komt eerst; alleen als het citaat niet letterlijk klopt valt de resolver
  terug op fuzzy, en die kan op een groot hoofdstuk *not searched* melden. De remedie is niet
  een groter budget maar een citaat dat klopt — zie stap 1 van de Werkstroom.

## Wat dit betekent voor wie de editor bouwt

Alles in de rechterkolom is al ergens benoemd — als comment, nullable kolom of gereserveerd
RFC-nummer. Niets hiervan is nieuw terrein. De kortste route naar een editor die de methode
draagt:

1. **Vocabulaire** (geen schemabump): standen en verdicten als tags, naast `ambiguity.yaml`.
   Werkt vandaag met een waarschuwing; met het vocabulaire zonder.
2. **Notitie-id** (schemabump, klein): een `id` op de annotatie, zodat een uitspraak naar een
   claim kan wijzen in plaats van naar dezelfde passage.
3. **`implements.verantwoording`** (schemabump): de invulling wijst naar haar claim. Dan hoeft
   geen poort meer te raden op naam.
4. **Taak voor een rol** (migratie): `assignee NULL` als wachtrij-toestand met een lezer, en
   `traject_role` uitgebreid met de gereserveerde `reviewer`. Dan ziet een bevoegde "wat wacht
   op mij" zonder dat iemand hem persoonlijk aanwijst.
5. **Voorwaarde op afronden**: de poort uit `poort.md` als check vóór `status = afgerond`.

De eerste stap kan zonder iemand te vragen. De tweede en derde horen in RFC-034 (issue
#1047), samen met de vraag of de blokkerende notitie en het concept-anker onder één nummer
blijven. De vierde en vijfde zijn editor-werk dat pas zin heeft als de eerste drie staan.
