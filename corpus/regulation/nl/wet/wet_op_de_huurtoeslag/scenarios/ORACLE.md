# Verzegeld orakel: huurtoeslag-rekenvoorbeelden uit dossier 36608

Dit bestand is verzegeld VOORDAT de machine_readable-encoding van de
Wet op de huurtoeslag (2026-01-01) is geschreven. De encoder mag de
verwachte bedragen hieronder niet wijzigen en niet als bron voor
condities gebruiken (voorrangsregel: bij divergentie geldt de wettekst).
Na het encoderen worden deze voorbeelden mechanisch getranscribeerd naar
een featurebestand; de verwachte waarden komen uitsluitend uit de
brondocumenten hieronder.

## Bron 1: Kamerstukken II 2024/25, 36 608, nr. 6 (nota naar aanleiding
## van het verslag), par. 3.1, "rekenvoorbeelden" (bedragen per maand,
## afgerond op hele euro's tenzij anders vermeld)

Vooraf, zelfde document (par. 2.3): "De huurtoeslagparameters voor 2025
en 2026 kunnen niet gedeeld worden omdat deze op dit moment nog niet
zijn vastgesteld." De 2026-bedragen hieronder zijn dus door het
ministerie berekend met parameters van 2024-vintage; de tabellen noemen
zelf de gebruikte ijkpunten van 2024.

### Voorbeeld 1 — alleenstaande (eenpersoonshuishouden, niet-oudere)
- kale huur € 700; servicekosten € 20 (2026: niet subsidiabel);
  rekenhuur 2026 = € 700; rekeninkomen € 25.000
  ("ongeveer 120% van het minimuminkomensijkpunt")
- VERWACHT 2026: huurtoeslag € 355,18 per maand (+€ 26 t.o.v. € 329,20 in 2024)

### Voorbeeld 2 — gezin (meerpersoonshuishouden, 3+ personen, niet-oudere)
- kale huur € 800; servicekosten € 0; rekenhuur € 800;
  rekeninkomen € 25.000; document noemt minimuminkomensijkpunt
  meerpersoons € 26.975 (2024): inkomen ONDER het ijkpunt, geen afbouw
- VERWACHT 2026: huurtoeslag € 463 per maand (+€ 87 t.o.v. € 376 in 2024)

### Voorbeeld 3 — alleenstaande oudere
- kale huur € 1.000; rekeninkomen € 30.000 ("circa 140% van het
  minimuminkomensijkpunt dat geldt voor alleenstaande ouderen
  (€ 22.025 in 2024)"); rekenhuur 2026 in de tabel: € 880
  (deel boven de maximale huurgrens niet gesubsidieerd)
- VERWACHT 2026: huurtoeslag € 315 per maand (2024: geen recht,
  huur boven de maximale huurgrens)

## Bron 2: Kamerstukken II 2024/25, 36 608, nr. 7 (nota van wijziging),
## tabellen 1-2 (beslagvrije voet; kolom "Nieuw" = na dit wetsvoorstel)

- Alleenstaande, bijstandsniveau € 15.702: huurtoeslag nieuw € 374
  (referentie "maximale huurtoeslag op bijstandsniveau": € 374)
- Laag-middeninkomen € 23.000: huurtoeslag nieuw € 352
- (Tabel 2) bijstandsniveau € 22.341: huurtoeslag nieuw € 374;
  modaal € 44.500: huurtoeslag nieuw € 97
  (huishoudtype niet expliciet; tabellen betreffen de bvv-rekenmodule)

## Statutaire parameterwaarden (vastgelegd bij verzegeling, met bron)

Wet zoals geldend 2026-01-01 (BWBR0008659, toestand 2026-01-01_0,
geïndexeerd bij Stcrt. 2025, 39783):
- kwaliteitskortingsgrens € 498,20; aftoppingsgrens € 713,02 (1-2 pers.)
  / € 764,14 (3+); maximale huurgrens € 932,93; normhuur bij
  minimum-inkomensijkpunt € 252,49, verlaagd met € 1,82 (eenpersoons)
  of € 3,63 (meerpersoons); afbouw Y×(27%|22%)/12; percentages 65/40
  (Besluit op de huurtoeslag, art. 7)
- Overzicht huurtoeslagparameters ministerie (2026): minimum basishuur
  € 202,52 (eenpersoons) / € 200,71 (meerpersoons); "verhoging normhuur
  −€ 48,15"

Regeling huurtoeslagparameters 2024 (Stcrt. 2023, 31878), door de
documenten als rekenbasis gebruikt: minimum-inkomensijkpunten
€ 20.700 (eenpersoons) / € 26.975 (meerpersoons) / € 22.025
(eenpersoonsouderen) / € 29.325 (meerpersoonsouderen); normhuur bij
minimum-inkomensijkpunt € 226,67; maximale huurgrens € 879,66.

## Voorspelling bij verzegeling

Een handberekening op geen enkele van beide parametersets reproduceert
€ 355,18 exact. Of de engine-uitkomsten de beloofde bedragen halen, en
zo niet, hoe groot het gat is en welke parameteraanname het kleinst
maakt, is wat de run meet.
