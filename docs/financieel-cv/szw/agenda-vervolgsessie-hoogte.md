# Vervolgsessie — de hoogte van de bedragen

**Voor:** SZW-jurist · **Voorstel duur:** 60 min · **Status:** agenda, nog te plannen

## Waarom dit blok

De eerste sessie heeft vooral **het recht** gevalideerd: wie komt waarvoor in
aanmerking, via welk lid. Dat staat nu redelijk. Wat níét gevalideerd is, is
**hoeveel** iemand krijgt.

Zes open vragen raken allemaal de hoogte. Bij elkaar bepalen ze of het
LKS-bedrag van onze casus **€766 of €1.078 per maand** is — een verschil van
ruim 40% op hetzelfde dienstverband.

Vier vragen gaan over de LKS, één over de LKV, één over de Wajong.

---

## De casus als rekenlat

Koen — Pwet-doelgroep, loonwaarde 60% van WML+VB, **32 uur** per week.

| Wat ons model nu zegt | |
|---|---|
| WML + vakantiebijslag | 215.500 ec |
| loonwaarde | 129.300 ec |
| bruto subsidie (het verschil) | **86.200 ec = €862 / maand** |
| maximum (70% van WML+VB) | 150.850 ec — niet bindend |

Elk van de vragen hieronder verschuift dit bedrag.

---

## 1 · Werkgeverslasten — tellen ze mee in de grondslag?

**Wettekst (Pwet 10d lid 4):** het maximum is 70% van WML+VB *"**vermeerderd met
een bij ministeriële regeling vastgestelde vergoeding voor werkgeverslasten**"*.

**Wat wij nu doen:** niets — `werkgeverslastenvergoeding_eurocent` staat als
`open_term` en is niet uitgewerkt. Wij rekenen de 70%-cap **zonder**
werkgeverslasten.

**De vraag:** worden de werkgeverslasten opgeteld bij WML+VB (en verhogen ze dus
zowel de subsidie als het maximum), of staan ze daarbuiten? En **welke**
ministeriële regeling stelt het bedrag vast?

> Dit is de vraag die de jurist bij UWV uitzet. Zonder antwoord blijven vraag 2
> en het maximum onbepaald.

## 2 · Loonwaarde — mét of zonder vakantiebijslag?

**Wettekst (10d lid 4):** het verschil tussen WML *"vermeerderd met de aanspraak
op vakantiebijslag"* en de loonwaarde *"**vermeerderd met de voor die persoon
naar rato van de loonwaarde rechtens geldende vakantiebijslag**"*. Dus **beide**
termen inclusief VB.

**Wat wij nu doen:** onze twee parameters zijn asymmetrisch benoemd —
`minimumloon_plus_vakantiebijslag_…` zegt het expliciet,
`loonwaarde_eurocent_per_maand` zegt er niets over. De waarde die wij gebruiken
ís VB-inclusief (129.300 = 60% van 215.500), dus de som klopt — maar de naam
dekt het niet.

**De vraag:** klopt onze lezing? En zo ja: moet de parameter niet
`loonwaarde_plus_vakantiebijslag_…` heten, zodat een gemeente-medewerker de
kale loonwaarde niet kán invullen? Doet die dat wél, dan valt de subsidie
**structureel te hoog** uit.

## 3 · De 50%-regeling van lid 5 — een tweede route

**Wettekst (10d lid 1):** twee routes.
- **1.a** — *"nadat het college **eerst de loonwaarde** heeft vastgesteld"* → lid 4, het verschil
- **1.b** — *"nadat het college in overleg met de werkgever heeft vastgesteld dat de **vaststelling van de loonwaarde achterwege kan blijven**"* → lid 5

**Wettekst (lid 5):** dan geldt *"gedurende een periode van maximaal de eerste
zes maanden … **50 procent** van het totale bedrag van WML + vakantiebijslag"*.

**Wat wij nu doen:** alleen de 1.a-route. Lid 5 staat als **untranslatable**
(`accepted: true`) — de engine kan één dienstverband niet in twee tariefperiodes
splitsen.

**Wat het scheelt voor Koen:** 50% × 215.500 = **107.750 ec = €1.077,50/maand**
in plaats van €862 — **€215,50 per maand**, ruim €1.290 over die zes maanden.

**De vraag:** is 1.b in de praktijk uitzondering of hoofdroute? En moet het
Financieel CV beide tonen (*"eerste 6 maanden €1.077,50, daarna €862"*)?

## 4 · Naar rato bij minder dan 36 uur

**Wettekst (10d lid 4, slot):** *"De loonkostensubsidie wordt naar evenredigheid
verminderd of vermeerderd, indien de overeengekomen arbeidsduur korter of langer
is dan 36 uren per week."*

**Wat wij nu doen:** niets — er is **geen arbeidsduur-parameter**. De korting
staat als untranslatable.

**Wat het scheelt voor Koen:** hij werkt **32 uur**. Ons €862 is het
36-uurs bedrag; naar rato zou het ≈ **€766** zijn.

**De vraag:** bevestig de rekenwijze (32/36), en of de "overeengekomen
arbeidsduur" ook geldt als de sector een andere volledige werkweek kent — de
wet noemt dat expliciet.

## 5 · LKV — welke categorie wint bij meerdere?

**Situatie:** Sadee voldoet aan **twee** categorieën.

| | bedrag |
|---|---|
| categorie b — arbeidsgehandicapt | 305 × 1664 = **€5.075,20** |
| categorie c — banenafspraak | 101 × 1664 = €1.680,64 |

**Wat wij nu doen:** wij passen *"hoogste bedrag wint"* toe op grond van
**Wtl art. 4.1 lid 3**. De MvT zegt hier **niets** over de volgorde.

**De vraag:** klopt die voorrangsregel? Het verschil is **€3.394,56 per jaar** op
hetzelfde dienstverband — puur door één extra status.

**Bijvraag:** wij nemen aan dat een Wajonger automatisch categorie b is
(`is_arbeidsgehandicapte_werknemer` is bij ons een gewone parameter, geen
afleiding). Klopt dat, of vergt het altijd een aparte doelgroepverklaring?

## 6 · De drie Wajong-tijdperken

**Uit de eerste sessie:** de Wajong kent drie regimes, en die zijn **belangrijk
voor de berekening**.

**Wat wij nu doen:** ons model maakt **geen onderscheid** tussen regimes.

**De vraag:** welke drie precies, en wat betekent elk voor de
loondispensatie-berekening? Welk regime geldt voor onze casus?

> Dit punt is nog het minst uitgewerkt — hier hebben we vooral toelichting nodig
> voordat we kunnen modelleren.

---

## Wat het samen betekent

De vragen stapelen niet allemaal op elkaar (1.a en 1.b zijn alternatieven), maar
de bandbreedte voor Koens LKS is:

| Scenario | Bedrag / maand |
|---|---|
| lid 4-route, 36 uur *(wat wij nu tonen)* | €862,00 |
| lid 4-route, naar rato 32 uur | ≈ €766 |
| lid 5-route, eerste 6 maanden, 36 uur | €1.077,50 |
| lid 5-route, naar rato 32 uur | ≈ €958 |

Plus het effect van de werkgeverslasten, dat op alle vier doorwerkt.

**Kernboodschap voor de sessie:** *wie* recht heeft is redelijk gevalideerd,
*hoeveel* nog niet. Voor een regelhulp die werkgevers een bedrag toont, is dat
tweede minstens zo belangrijk.

## Voorgestelde volgorde

1. Vraag 1 + 2 samen (beide 10d lid 4, samen de grondslag) — **20 min**
2. Vraag 3 en 4 (de twee correcties op het bedrag) — **20 min**
3. Vraag 5 (LKV-voorrang) — **10 min**
4. Vraag 6 (Wajong-regimes, toelichtend) — **10 min**

**Vooraf nodig:** het antwoord van UWV op de werkgeverslasten-vraag. Zonder dat
blijft vraag 1 hangen en daarmee ook het maximum in vraag 2.
