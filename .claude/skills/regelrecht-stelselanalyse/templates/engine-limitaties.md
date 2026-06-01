# Engine-executie-limitaties — {engine + versie}

**Datum**: {datum}

> Dit document bevat **engine-limitaties**: gevallen waar wet én onze modellering
> kloppen, maar de engine de uitvoering (nog) niet ondersteunt. Géén wetgevings- of
> modellering-fout — track apart zodat het corpus niet onterecht "fout" lijkt.

> **Bewijs-poort (verplicht).** Een limitatie mag pas hier worden opgenomen ná een
> **reproduceerbare engine-run die het falen aantoont** — met het scenario/commando en de
> foutuitkomst in het veld **Bewijs**. Een *onbewezen aanname* ("de engine kan vast geen
> X") is géén limitatie: noteer die als **open vraag** in de cyclus (of weerleg hem door
> het juist wél te binden en te testen). Veelgemaakte foute aanname: "de engine kan niet
> meerdere `source:`-bindingen per artikel resolveren" — dat is onjuist (schema v0.5.2
> ondersteunt dit); zo'n geval is een modellering-fout, geen limitatie. Zonder Bewijs-veld
> hoort een regel niet in dit document.

## Limitaties

### {nr} — {korte titel}
**Geraakt**: {welke outputs / wetten / artikelen} ({aantal}).
**Wat de engine niet kan**: {concrete beschrijving — bijv. een operatie/vergelijking die
ontbreekt}.
**Bewijs**: {het scenario/commando dat faalt + de foutuitkomst}.
**Workaround** *(indien)*: {tijdelijke modellering-omweg, en de prijs daarvan}.
**Voorgestelde engine-actie**: {engine-PR / feature die dit oplost — hoort in de engine,
niet in het corpus}.

*(herhaal per limitatie)*

## Samenvatting

| Limitatie | Geraakte outputs | Engine-actie |
|---|---|---|
| {nr} | {n} | {PR-voorstel} |
