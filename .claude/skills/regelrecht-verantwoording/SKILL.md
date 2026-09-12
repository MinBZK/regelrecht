---
name: regelrecht-verantwoording
description: >
  Legt vast wie een interpretatiebeslissing nam, op welke grond, met welk
  alternatief, en wie erover moet spreken voordat het punt dicht kan — als
  stand-off notitie bij de tekst, in het bestand dat de editor leest. Gebruik dit
  wanneer een open term wordt ingevuld, een open norm een lezing krijgt, of een
  bepaling meerdere kanten op kan; en als bevoegde om asynchroon een uitspraak te
  doen over zo'n claim. Casus-agnostisch. Het ritme waarin dit gebeurt staat in
  regelrecht-traject; de poort die het afdwingt in references/poort.md.
allowed-tools: Read, Glob, Grep, Bash, Edit, Write
user-invocable: true
---

# Verantwoording — de vorm waarin een lezing telt

Een verrijking die een open term invult, kiest een lezing. Zonder deze skill verdwijnt
die keuze: de invulling komt in het corpus, de reden staat hoogstens in proza ernaast, en
na een maand is niet meer te zien wie het besloot of waarom.

Deze skill legt de vorm vast waarin zo'n keuze wél telt, en schrijft haar weg op de enige
plek waar zowel een agent als de editor haar vindt: de annotaties-sidecar van het traject.
Hij bouwt geen register. Het ritme — werkronde, uitspraak, verwerken, mijlpaal — is van
`regelrecht-traject`; deze skill is wat je op elk van die momenten pakt.

## Wanneer je hem pakt

- de verrijking vult een `open_term` of kiest tussen twee lezingen van een bepaling
- een `untranslatable` krijgt alsnog een invulling
- een analist neemt een voorstel van een model over, of verwerpt het
- **als bevoegde**: je spreekt je uit over een claim die op jou wacht — asynchroon, in de
  editor of via deze skill, zonder op een sessie te wachten

Niet: een gewone opmerking bij een tekst. Zie *Wat géén claim is*, onderaan.

## Het contract — zes vragen

Een opmerking telt pas als claim wanneer zij deze zes beantwoordt. Kan zij dat niet, dan
is het een opmerking en verder niets.

| | vraag | waar het vandaan komt |
|---|---|---|
| 1 | **wat** is vastgesteld | de lezing zelf |
| 2 | **door wie** | de actor die de stand zette |
| 3 | **op welke grond** | wettekst, wetsgeschiedenis, systematiek, uitvoeringspraktijk |
| 4 | **wanneer** | het moment, niet de peildatum van een bestand |
| 5 | **wie moet erover spreken** | de bevoegde — afgeleid, zie stap 2 |
| 6 | **vóór wanneer** | een mijlpaal, geen datum |

Vraag 1–4 zijn de vastleggingselementen; git levert er drie van als je per claim commit.
Vraag 5 en 6 kan git niet weten en zijn dus altijd velden.

**Het alternatief hoort er verplicht bij.** Een lezing zonder haar verworpen alternatief is
een bewering, geen oordeel. De bevoegde kan niets bekrachtigen als hij niet ziet waartegen
gekozen is, en het burger-effect is pas te wegen als beide kanten er staan. Noem per
alternatief het gevolg.

## De standen hangen aan de cyclus

| moment (`regelrecht-traject`) | stand | wie zet hem |
|---|---|---|
| werkronde — machine-laan | `voorgesteld` | model |
| werkronde — desk | `tijdelijk_vastgesteld` | analist; **rekent mee** |
| uitspraak — asynchroon of sessie | `uitgesproken` | de bevoegde |
| mijlpaal | `bewaakt` | de poort |

Elke overgang vergt een andere actor; sluiten vergt een bevoegde. Dat is de dragende regel
en zij is een gevolg van de cyclus, geen afspraak. De verrijking stopt nooit op een open
rechtsvraag: er komt een werkende versie uit met een werkhypothese, de keten rekent door, en
alleen de mijlpaal blokkeert.

## Werkstroom

Het bestandssysteem is de interface. Elke stap laat een bestand achter dat de volgende stap
en de editor lezen. Werk op de trajectbranch.

### Stap 1 — anker vinden

Zoek de passage waar de keuze aan hangt en neem haar **letterlijk** over uit de geldende
toestand van de wet (`regulation/…/{law_id}/{valid_from}.yaml`, veld `text` van het
artikel). Kies een citaat dat één keer voorkomt; komt het vaker voor, neem dan prefix en
suffix mee (tien tot veertig tekens ieder).

Twee dingen die je alleen ontdekt als het misgaat:

- **Toets tegen de geparste tekst, niet tegen het bestand.** Het `text`-veld is omgebroken:
  een prefix die een regeleinde kruist bevat een `\n` waar jij een spatie typt, en exact
  matchen eist gelijkheid. Laad de YAML, pak `text`, en controleer dat
  `prefix + exact + suffix` er letterlijk in staat. Houd prefix en suffix binnen één regel.
- **Een artikelhint helpt niet.** `regelrecht:hint` mag in het schema, maar de resolver
  negeert hem bewust (een hint kan een concurrerende treffer verbergen). Het anker is het
  citaat, en alleen het citaat. Een nummer overleeft hernummering niet; een citaat wel.

Rapporteer: *"Anker: '{citaat}' in {law_id} art {n}, toestand {valid_from}."*

### Stap 2 — de bevoegde afleiden

Twee bronnen, in deze volgorde:

1. Is het een open term? Lees `open_terms[].delegated_to` op het artikel dat hem
   declareert — dat veld zegt letterlijk wie hem mag invullen.
2. Anders: het soort vraag. Feit → wie de regeling uitvoert. Lezing → wie over de tekst
   gaat (jurist). Bevoegdheid of normconflict → jurist of bestuur. Ontbrekend beleid → de
   normsteller.

Klopt de afleiding niet, overschrijf haar dan mét reden in de claim. Die reden is zelf een
bevinding: waar de afleiding faalt, zit een gat in het model van bevoegdheid.

### Stap 3 — de claim schrijven als notitie (append-only)

Het bestand is `annotations/{law_id}/annotations.yaml` in de wortel van de corpusrepo, op de
trajectbranch. Bestaat het niet, maak het aan met alleen `annotations:` erin.

**Append-only is een harde regel.** Voeg het nieuwe item achteraan de lijst toe, met dezelfde
inspringing als de bestaande items. **Parse en herschrijf het bestand nooit** — dat herordent
sleutels, gooit de motivering-commentaren weg en hangt elke regel aan de laatste schrijver
(`git blame`). De editor doet exact hetzelfde: hij plakt nieuwe notities achter de bestaande
bytes en weigert een herbouwd bestand.

De vorm, binnen het huidige notitieschema (één notitie, meerdere bodies):

```yaml
  - type: Annotation
    motivation: questioning
    creator: <wie de stand zet — naam of tool>
    created: '<ISO-datum>'
    workflow: open
    target:
      source: regelrecht://<law_id>
      selector:
        type: TextQuoteSelector
        exact: '<het citaat uit stap 1>'
        prefix: '<10-40 tekens ervoor, binnen één regel>'
        suffix: '<10-40 tekens erna, binnen één regel>'
    body:
      - type: TextualBody
        purpose: questioning
        value: '<de kwestie — wat kan twee kanten op, in de woorden van de tekst>'
      - type: TextualBody
        purpose: assessing
        value: '<de lezing — in één zin te begrijpen>'
      - type: TextualBody
        purpose: commenting
        value: 'grond: <wettekst · wetsgeschiedenis · systematiek · praktijk>'
      - type: TextualBody
        purpose: commenting
        value: 'alternatief: <de verworpen lezing> — gevolg: <wie merkt het, welke kant op>'
      - type: TextualBody
        purpose: tagging
        value: <een tag uit annotations/_vocabulary/ambiguity.yaml, bv. needs-uitvoeringsbeleid>
      - type: TextualBody
        purpose: tagging
        value: stand:tijdelijk_vastgesteld
      - type: TextualBody
        purpose: tagging
        value: bevoegd:<uit stap 2>
      - type: TextualBody
        purpose: tagging
        value: uiterlijk:<mijlpaal-id>
      - type: TextualBody
        purpose: tagging
        value: voorstel_van:<model|mens>
```

Waarom zo: `questioning` is de motivation die RFC-018 voor een open norm reserveert; de
`tagging`-bodies dragen de stand, de bevoegde en de termijn omdat het schema daar nog geen
velden voor heeft (zie `references/editor.md`). Een tag buiten het vocabulaire geeft een
**waarschuwing, geen fout** — dat is het RFC-018-gedrag, en precies waarom deze route nu al
werkt. `regelrecht:visibility` weglaten: dan is de notitie publiek en gaat zij in git.

Bij een `voorgesteld` (uit een model): dezelfde vorm, `creator` is de tool, `stand:voorgesteld`,
en geen `uiterlijk` — die komt pas als een analist hem overneemt.

### Stap 4 — valideren

```
just validate-annotations <pad naar annotations.yaml>
```

vanuit de monorepo, of de corpuspoort van het traject als die er is. Faalt het schema, dan
had de editor de save óók geweigerd — los het op vóór je commit. Meldt de resolver
`ambiguous`, verleng prefix of suffix; meldt hij `orphaned`, dan klopt het citaat niet met
de geldende toestand. Onbekende tags zijn een waarschuwing en horen erbij.

Rapporteer: *"Claim vastgelegd: {law_id} art {n} · stand {stand} · bevoegd {bevoegd} ·
uiterlijk {mijlpaal} · resolver {found|ambiguous|orphaned}."*

### Stap 5 — als bevoegde: de uitspraak vastleggen (asynchroon)

Een uitspraak is een **nieuwe** notitie, nooit een wijziging van de claim. Zelfde bestand,
zelfde `target` (hetzelfde citaat — het schema kent geen notitie-id om naar te wijzen),
motivation `replying`:

```yaml
  - type: Annotation
    motivation: replying
    creator: <de bevoegde>
    created: '<ISO-datum>'
    target: <identiek aan de claim>
    body:
      - type: TextualBody
        purpose: tagging
        value: verdict:<bekrachtigd|gecorrigeerd|weerlegd|onbeslist_gelaten>
      - type: TextualBody
        purpose: commenting
        value: 'reden: <verplicht bij onbeslist_gelaten en gecorrigeerd; gewenst altijd>'
```

Niets zeggen is geen uitspraak. `gecorrigeerd` opent een nieuw punt voor de analist: terug
naar stap 3 met de correctie als kwestie. `onbeslist_gelaten` zonder reden bestaat niet.

### Stap 6 — de poort draaien (adviserend)

In een traject met een `just poorten`: draai hem. Anders het script dat de verantwoording
meet. Hij zegt welke invullingen niet naar een claim wijzen, welke claims nog op een
uitspraak wachten en van wie, en wat over zijn `uiterlijk` heen is. Tussen mijlpalen is dat
advies; op de mijlpaal blokkeert het — zie `references/poort.md`.

### Stap 7 — rapporteren

```
Verantwoording — {law_id}
  claims vastgelegd:      {N}  (voorgesteld {a} · tijdelijk_vastgesteld {b})
  uitspraken vastgelegd:  {N}  (bekrachtigd {a} · gecorrigeerd {b} · weerlegd {c} · onbeslist {d})
  wacht op uitspraak:     {N}  → {bevoegde}: {aantal}, …
  over uiterlijk heen:    {N}
  resolver:               {found}/{ambiguous}/{orphaned}
  bestand:                annotations/{law_id}/annotations.yaml (+{regels})
```

## Waar de claim woont — het anker beslist

- over **wettekst** → de sidecar, zoals hierboven. Citaat-anker, resolver, federatie.
- over het **model** — een dode parameter, een output die niemand leest, een open term die
  nergens aan hangt — dan is er geen citaat. Die hoort bij het model, in de YAML.

Een taak is een verwijzing, geen bewaarplaats. Een overzicht is een reductie. Niets is twee
keer canoniek.

## De invulling wijst naar haar verantwoording — nog niet

Het sluitstuk ontbreekt: een `implements`-blok dat een open term vult, hoort naar de claim
te wijzen, en `implements` staat op `additionalProperties: false` met vier velden. Zolang dat
veld er niet is, kan een poort alleen op naam raden — en raden faalt in beide richtingen.
Zet daarom in de `gelet_op`-beschrijving van het `implements`-blok naar welke notitie hij
verwijst (citaat + creator + datum). Werkhypothese van de methode zelf; zie
`references/editor.md` voor wat het schema moet gaan dragen.

## Wat géén claim is

- een opmerking bij een tekst zonder gevolg voor de uitvoering
- een leeg antwoordveld. **Openheid is een bewering, geen afwezigheid.** Zet de stand.
- een verwijzing naar een register-id uit één traject. Die bestaat nergens anders.

## Verder

- `regelrecht-traject` — de cyclus, de escalatieladder, en welke skill wanneer
- `references/poort.md` — de vijf regels die een mijlpaal blokkeren
- `references/vorm.md` — de velden veld voor veld, met een uitgewerkt voorbeeld
- `references/editor.md` — wat de editor al draagt, wat gereserveerd is, wat ontbreekt
