# SZW — notities juristvalidatie Financieel CV

**Datum:** 23 juli 2026 · **Bron:** aantekeningen van de sessie met de
SZW-jurist · **Status:** ruwe vangst, gestructureerd — nog niet verwerkt in de
corpus.

Verwijzingen naar wetsartikelen zijn toegevoegd op basis van de gemodelleerde
corpus; de inhoudelijke punten zijn van de jurist.

---

## Bevestigd

### Nietigheid is goed gemodelleerd
Onze modellering van **Wajong art. 2:20 lid 2** — het beding tot lagere
beloning is nietig, in het model als harde constante `true` — is akkoord
bevonden.

> Hiermee is een van onze eigen open vragen beantwoord: *"lid 2 nietigheid
> modelleren wij als harde constante — akkoord?"* → **ja**.

---

## Bevindingen die om aanpassing vragen

### 1. Werkgeverslasten: waar staan ze, en tellen ze mee in de grondslag?
**Pwet art. 10d lid 4** noemt de vergoeding voor werkgeverslasten, maar
**verwijst door naar een ministeriële regeling** — het bedrag staat dus niet in
de wet. De vraag die daaruit volgt: moeten die werkgeverslasten worden
**opgeteld bij WML + vakantiebijslag** (en dus meetellen in de grondslag voor
het 70%-maximum), of staan ze daarbuiten?

Dit is dezelfde knoop als punt 3 hieronder: *loon → subsidie + werkgeverslasten*
lijkt in 10d lid 4 te zitten, maar dat artikel delegeert weer verder.

**Status in ons model:** `werkgeverslastenvergoeding_eurocent` staat als
`open_term` en is **niet uitgewerkt**. De 70%-cap rekenen wij nu zonder
werkgeverslasten.

**Consequentie:** zolang dit open staat, kan het LKS-bedrag afwijken — zowel
het subsidiebedrag zelf als het maximum.

### 2. WIA 35 vs Wajong — dezelfde voorzieningen, dubbel geregeld
**Wet WIA art. 35 lid 4.a** sluit Wajong-gerechtigden uit. Maar in de **Wajong**
zijn **precies dezelfde voorzieningen** opgenomen. Het is dus geen uitsluiting
van de voorziening, maar een **andere vindplaats** — dubbele informatie in twee
wetten.

**Probleem voor de gebruiker:** vanuit de WIA bezien lijkt het alsof de
betrokkene (in onze casus Sadee) **geen recht** heeft op jobcoaching en
werkplekaanpassing. Dat is misleidend: het recht bestaat wél, alleen via de
Wajong.

**Actie:** de Wajong-voorzieningen alsnog modelleren, zodat het Financieel CV
niet "geen recht" toont waar in werkelijkheid een andere route geldt.

### 3. Wajong kent drie tijdperken
De Wajong kent **drie regimes**, en die zijn **belangrijk voor de berekening**.
Ons model maakt dat onderscheid nu niet.

**Consequentie:** de LDP-uitkomst kan per regime verschillen; welke regime van
toepassing is, bepaalt mede de berekening. Dit moet in de modellering terugkomen.

---

## Besluiten

### LIV gaat eruit
Het lage-inkomensvoordeel wordt **uit de scope gehaald**. De regeling is per
2025 afgeschaft (Wet 36458); wij hielden 'm nog als historisch artefact aan.

**Gevolg:** de LIV-scenario's (die nu bewust falen met een *"Output not found"*)
kunnen worden verwijderd, en LIV kan uit de documentatie en de diagrammen.

---

## Acties

| # | Actie | Bij wie |
|---|---|---|
| 1 | Navragen bij **UWV** hoe het zit met de openstaande onduidelijkheden | SZW-jurist |
| 2 | Wajong-voorzieningen (tegenhanger van WIA 35) modelleren | ons |
| 3 | Drie Wajong-tijdperken in de berekening verwerken | ons |
| 4 | LIV uit scope halen: scenario's, docs en diagrammen | ons |
| 5 | Werkgeverslasten-vraag uitzoeken zodra de ministeriële regeling helder is | ons, na actie 1 |

---

## Nog niet beantwoord

Deze open punten uit de pre-read zijn in deze sessie **niet** aan bod gekomen en
staan dus nog:

- **LKS:** loonwaarde mét of zonder vakantiebijslag (de parameternaam dekt het
  niet) — hangt samen met bevinding 1
- **LKS:** de 50%-regeling van lid 5 (route via lid 1 onderdeel b), eerste zes
  maanden
- **LKS:** naar-rato-korting bij arbeidsduur < 36 uur — Koen werkt 32 uur, ons
  bedrag is het 36-uurs bedrag
- **LKV:** voorrang bij meerdere categorieën ("hoogste bedrag wint", art. 4.1
  lid 3) — de MvT zwijgt hierover
- **NRP:** beschut werk (Pwet 10b) — in MvT 34194 uitgesloten, nu via lid 2.f
  ingesloten
- **Versie-drift:** LKV-categorie d geschrapt per 2026, LKS-doelgroep uitgebreid
  met Pwet 10d.2.c
