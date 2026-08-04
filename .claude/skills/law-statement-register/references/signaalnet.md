# Het signaalnet — de recall-net achter "niets overslaan"

Het signaalnet is een set deterministische detectoren over `canonical.md`, onafhankelijk van
de lezer. Elke zin die het net raakt moet gedekt zijn door een statement, óf in een segment
staan met een niet-normatieve `disposition`. Alles wat overblijft is een stil overgeslagen
norm, en dat is een gate-failure.

Het net vervangt de lezer niet — het controleert hem. Een lezer die "haal de regels eruit"
krijgt, laat weg wat hij niet interessant vindt, en niemand kan achteraf zien wát. Het net
maakt die weglating zichtbaar zonder te oordelen of het weggelatene relevant was.

## Waarom het bewust ruw is

Het net kent geen grammatica en geen context. Een inhoudsopgave-regel "2.1 Wie kan er een
aanvraag indienen?" is een treffer, omdat er "kan" in staat. Dat is geen bug: die
treffer kost je één regel in het register (`disposition: navigational`), terwijl een gemiste
uitzondering in een voetnoot je een verkeerd model kost. De asymmetrie is de ontwerpkeuze.

Verwacht daarom veel treffers en veel disposities. Op een toelichting van 17 pagina's is
ruwweg honderd treffers over enkele tientallen zinnen normaal.

## Het lexicon

| Categorie | Waarom | Patroon (regex, case-insensitive) |
|---|---|---|
| `deontisch` | draagt de verplichting, het recht of de bevoegdheid — de kern van elke norm | `moet(en)?`, `dient/dienen`, `verplicht`, `mag/mogen`, `kan/kunnen`, `wordt geacht`, `bevoegd`, `recht op`, `in aanmerking` |
| `conditioneel` | markeert de voorwaarde of de uitzondering; "tenzij" is de klassieke weglating | `indien`, `tenzij`, `mits`, `voor zover`, `behoudens`, `met dien verstande`, `wanneer` |
| `zachtheid` | markeert een `soft-default` of discretie; wordt anders als harde regel gemodelleerd | `in beginsel`, `in de regel`, `doorgaans`, `zoveel mogelijk`, `naar oordeel van`, `maatwerk`, `bijzondere omstandigheden`, `schrijnend` |
| `kwantiteit` | bedragen, percentages en termijnen zijn de meest modelleerbare én meest verouderende statements | `€ n`, `n%`, `n (euro\|procent\|dagen\|weken\|maanden\|jaar)` |
| `definitie` | een begripsbepaling in een beleidsstuk stuurt de uitleg van de norm | `wordt verstaan onder`, `geldt als`, `hieronder valt`, `wordt aangemerkt als` |
| `verwijzing` | koppelt het statement aan een norm of een ander document; bron voor de verankering | `artikel n`, `bijlage x`, `zie hoofdstuk/paragraaf/artikel` |

De canonieke definitie staat in `scripts/statement_gates.py` (`DEFAULT_LEXICON`); deze tabel
legt uit waaróm elke categorie er staat.

## Uitbreiden

```bash
python3 scripts/statement_gates.py signaalnet \
    --canonical canonical.md --ledger statements.yaml --lexicon mijn-lexicon.yaml
```

Het bestand is een platte `naam: regex`-mapping en **vervangt** het standaardnet, dus neem
de standaardcategorieën over als je alleen wilt aanvullen.

Domein-uitbreidingen die zich in de praktijk lonen:

```yaml
bewijs: '\b(overleggen|aantonen|bewijsstuk|kopie|verklaring|formulier)\b'
actor: '\b(college|dagelijks bestuur|behandelaar|ambtenaar|aanvrager|belanghebbende)\b'
sanctie: '\b(niet in behandeling|afgewezen|buiten behandeling|terugvordering|boete)\b'
temporeel: '\b(peildatum|met ingang van|met terugwerkende kracht|vervalt)\b'
```

Voeg een categorie toe zodra je in de tegenlees-pass een gemist statement vindt dat het
bestaande net niet had kunnen vangen. Dat is de enige groeiregel die je nodig hebt: het net
groeit door zijn eigen misses, niet door voorspelling.

## Wat het net niet ziet

Drie blinde vlekken die je met de hand moet afdekken; noteer ze in de tegenlees-pass:

1. **Normen zonder signaalwoord.** *"De aanvraag gaat vergezeld van de laatste drie
   loonstroken."* Geen modaal werkwoord, wel een harde eis. De `bewijs`-uitbreiding hierboven
   vangt dit; zonder die uitbreiding niet.
2. **Tabellen.** Een normbedragen-tabel bevat de kwantiteiten in cellen zonder zin. Het net
   ziet de getallen wel maar de zinsafbakening klopt niet, waardoor één treffer een halve
   tabel als "zin" rapporteert. Behandel tabellen in de sweep als aparte modaliteit.
3. **Afbeeldingen en beslisbomen.** Tekst in een plaatje staat niet in `canonical.md` en is
   voor het net onzichtbaar. Markeer zulke segmenten `non-textual` met een reden, en laat de
   inhoud met de hand transcriberen — nooit stil OCR-en en dat verbatim noemen.
