# Modus 2 — de versie-diff

Een beleidsdocument wordt herzien zonder dat er een norm verandert. De diff tussen twee
registers laat zien wat er dan feitelijk is gewijzigd: welke uitspraken zijn toegevoegd,
verdwenen, of van strekking veranderd. Dat is het beleid dat verschoof buiten het zicht van
elk versiebeheer om.

## De regel die de diff bruikbaar maakt

> **Her-ontgin blind.** Verwerk het nieuwe document volledig (fasen 0–7) zonder het vorige
> register open te hebben. Pas daarna matchen.

De verleiding is het vorige register langs te lopen en per statement te kijken wat er in de
nieuwe versie van geworden is. Dat is sneller en het is fout: je erft elke omissie van de
vorige ronde, en "toegevoegd" betekent dan niet *nieuw in het document* maar *ditmaal wel
opgemerkt*. De hele waarde van de categorie `added` staat of valt hiermee.

Praktisch: doe de nieuwe extractie in een aparte werkmap, en open het oude register pas bij
stap 2 hieronder.

### Wél meteen: de mechanische tekst-diff

De blind-regel geldt voor de **register**-diff, niet voor een `diff` van de twee
`canonical.md`-bestanden. Dat is een ander soort ding: hij is *compleet by construction* —
elk gewijzigd teken staat erin, hij kan niets missen, en hij bevat geen enkel oordeel. Draai
hem dus gerust als eerste; hij vertelt je hoe groot de klus is.

```bash
diff -u v1/canonical.md v2/canonical.md
```

Twee dingen om te weten:

- **Hij vervangt het her-ontginnen niet.** Als het vorige register maar een deel van het
  document dekte, moet v2 alsnog volledig gelezen worden — anders meet je de vorige ronde.
- **Hij is de scherpste test op de dekking van het vorige register.** Vallen de gewijzigde
  regels buiten élk statement van v1, dan zou een register-diff "niets veranderd" hebben
  gezegd terwijl het document wél wijzigde. Dat is geen theoretisch geval: een halfjaarlijks
  geïndexeerde normbedragen-bijlage is precies zo'n sectie — hoog wijzigingstempo, laag
  leestempo, en daardoor stelselmatig het eerste wat een selectieve lezer overslaat.

## Statement-identiteit

Gelaagd, omdat elke laag iets anders overleeft:

| Laag | Rol | Overleeft |
|---|---|---|
| `slug` | de sleutel (`tweede-vrijlating-50pct`) | herformulering, herordening, hernummering |
| `anchor` | het zoekmiddel (`{exact, prefix, suffix}`) | verplaatsing binnen het document |
| `S<n>` | weergavenummer in één register | niets — puur presentatie |

Nooit paginanummer of volgnummer als identiteit: een herzette PDF verschuift alles.

De slug wordt bij de eerste registratie toegekend en daarna **overgedragen**, niet opnieuw
bedacht. Een statement dat in v2 anders is geformuleerd maar dezelfde uitspraak doet, houdt
zijn slug — anders leest de diff als "verdwenen + toegevoegd" terwijl er een zin is herschreven.

## Het algoritme

1. **Her-ontgin v2 blind.** Volledig register, eigen gates, eigen dekkingscijfers.

2. **Resolveer v1-ankers tegen `canonical(v2)`.** Per v1-statement:

   | Uitkomst | Categorie |
   |---|---|
   | exacte treffer van `prefix+exact+suffix` | `unchanged` |
   | geen exacte treffer, fuzzy score ≥ drempel | `reworded` |
   | niets boven de drempel | `candidate-removed` |

   Voor fuzzy matching geldt de RFC-005-methode (`docs/src/content/rfcs/rfc-005.md`):
   gewogen Levenshtein-similarity `exact × 0,5 + prefix × 0,25 + suffix × 0,25`, drempel 0,7.
   De engine heeft die resolver al (`packages/engine/src/annotation/resolver.rs`); gebruik
   hem in plaats van een tweede implementatie te schrijven.

3. **Match v2 op v1** via slug en anker. Elk v2-statement zonder tegenhanger is `added`.

4. **Oordeel de `reworded`.** Twee mogelijkheden, en het onderscheid is niet mechanisch:

   - `same-strekking` — redactionele herschrijving. Zelfde eis, andere woorden.
   - `changed-strekking` — de uitspraak zelf is veranderd: een ander bedrag, een andere
     termijn, een verdwenen "tenzij", een `hard` die `soft-default` is geworden of andersom.

   Een `changed-strekking` krijgt **altijd** menselijke bevestiging en is **altijd** een
   bevinding, ook als het model niet verandert. Let in het bijzonder op de bindendheid: een
   zin die van *"bedraagt ten hoogste"* naar *"bedraagt in de regel ten hoogste"* gaat, ziet
   er als tekstwijziging onbeduidend uit en verandert de norm die je modelleert.

5. **Beoordeel `candidate-removed` met de hand.** Een verdwenen statement is óf geschrapt
   beleid, óf verplaatst naar een ander document, óf het anker was te specifiek. Alleen de
   eerste is een bevinding; de derde is een defect in het vorige register en hoort daar
   gerepareerd te worden.

## Wat de diff oplevert

Per categorie een tabel met slug, beide citaten en het oordeel. De vier die ertoe doen:

- **`added` met bucket LETTER-vs-TOELICHTING** — nieuw beleid dat geen normgrondslag heeft.
  Het sterkste signaal dat de diff kan geven: beleid dat is aangescherpt zonder dat de
  regeling meebewoog.
- **`candidate-removed` met bucket letter-getrouw** — een uitleg die is weggehaald terwijl de
  norm bleef. Vaak een teken dat de praktijk is veranderd.
- **`changed-strekking` op een kwantiteit** — bedrag of termijn gewijzigd; controleer of het
  model die waarde hardcodeert.
- **`changed-strekking` op bindendheid** — de stilste van allemaal.

Noteer ook wat er *niet* veranderde: een statement dat in beide versies `niet-gevonden` is,
is een jurist-vraag die een herziening lang heeft overleefd. Dat is zelf een bevinding.

## Dekkingsvergelijking

Zet de dekkingscijfers van beide registers naast elkaar (% betegeld, aantal
signaalnet-treffers, aantal statements, aantal disposities). Springt het aantal statements
omhoog terwijl het document niet groeide, dan meet je niet het document maar de vorige
ronde — zeg dat dan ook zo in het register, in plaats van het als beleidswijziging te
presenteren.
