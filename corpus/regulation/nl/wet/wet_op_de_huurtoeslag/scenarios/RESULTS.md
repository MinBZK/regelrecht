# Resultaat van de verzegelde run (2026-09-03)

Encoding geschreven NA verzegeling van ORACLE.md (commit cd0d233d),
zonder de verwachte bedragen te raadplegen als bron van condities.
Suite: `cargo test --test bdd` in `packages/engine`; alle 82 bestaande
scenario's slagen, de drie orakelscenario's falen alle drie:

| Voorbeeld (36 608, nr. 6) | Beloofd | Engine (wet 2026) | Delta |
|---|---|---|---|
| 1. alleenstaande, huur 700, inkomen 25.000 | € 355,18 | € 330,10 | − € 25,08 |
| 2. gezin 3+, huur 800, inkomen 25.000 | € 463,00 | € 484,70 | + € 21,70 |
| 3. alleenstaande oudere, huur 1.000, inkomen 30.000 | € 315,00 | € 343,84 | + € 28,84 |

Duiding: de nota naar aanleiding van het verslag zegt zelf dat de
parameters voor 2025/2026 "op dit moment nog niet zijn vastgesteld" en
rekent zijn 2026-beloftes daarom met 2024-materiaal (de tabellen noemen
de 2024-ijkpunten). De wet zoals die op 2026-01-01 geldt draagt andere,
geïndexeerde bedragen (Stcrt. 2025, 39783) en sinds nr. 7 ook een
gewijzigde verlaging. Geen van de drie beloofde bedragen is tegen de
geldende wettekst reproduceerbaar; de afwijking heeft geen vast teken en
loopt tot 9%. Dit is de verouderingslimiet uit het paper, nu gemeten op
de nieuwste voorbeelden in het record, voor een wet die nog niet eens in
werking was toen ze werden beloofd.

De ijkpunt-inputs van de scenario's volgen het document (2024-waarden);
alleen de wettelijke bedragen van 2026 wijken daarvan af. Wie de
2024-grenzen en -normhuur óók invoert, toetst een wet die op de
rekendatum niet gold; die variant staat bewust niet in dit bestand.
