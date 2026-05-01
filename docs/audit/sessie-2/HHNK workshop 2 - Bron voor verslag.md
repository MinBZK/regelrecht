In workshop 2 zijn we verder aan de slag gegaan met kwijtschelding.
Ik had van alles voorbereid (slidedeckje, workshopdoc en audit doc weer) maar zijn er niet aan toe gekomen om langs die precieze lijn te werken.

Aanwezig: 
-Project initiator met kennis invordering
-Dossier controleur intern HHNK (andere dan vorige kwijtscheldingsexpert)

Wat we hebben gedaan:
## 1. Introductie kennismaking en recap scope bepaling
	1. uitgelegd aan hand van briefjes dat logica meer in art. 11 URI terecht is gekomen
	2. Voorbeelden van punten waar we in de eerste sessie aan hebben geraakt met name rondom art 26 leidraad HHNK (Betalingscapaciteit, autowaarde etc.)
## 2. Procesflow opgesteld over hoe en waar welke informatie vandaan komt en hoe proces er uit ziet 
(doel: begrip Daan waar rol HHNK nu in zit)
![[Scherm­afbeelding 2026-05-01 om 08.51.19.png]]
Geleerd: trace met enkele losse aanpassingen is best een nuttige manier om te kijken naar HHNK werkwijze: zij krijgen afwijzersadvies van HHNK en gaan op dat punt vooral toetsen. het proces van traces bouwen rondom die enkele 

## 3. Editor: scenario(s) doorlopen
In de editor zijn we aan de slag gegaan met geoormerkt scenario 'Volledige kwijtschelding voor iemand zonder inkomen'

Ik heb deze lokaal gewijzigd naar pensioengerechtigde leeftijd = true
default was vrijwel alles verder 0 (inkomen etc.)

Alles wat opviel:
1. Niet logisch dat iemand kwijtschelding krijgt wanneer netto besteedbaar inkomen 0 is. Als je een inkomen hebt is dat lastig. 
	1. Bij pensioengerechtigd zou in dit geval verwacht AOW nu mee moeten wegen in dit scenario (hoewel niet gespecificeerd. dan worden vragen gesteld over hoe iemand in leven kan voorzien.
	2. BC = 0 kan, netto besteedbaar inkomen = 0 kan niet
	3. Die AOW: complexer dan een standaard normbedrag (dat kan minder zijn, daar moet vakantiegeld bij worden opgeteld bijv.)
	4. AOW staat wel in de trace maar dus niet in inkomenshoek.
2. kostennorm staat nu op 0.9 --> in regeling minstens 0.8 dus logisch. in praktijk hhnk met lagere regelgeving 0.4. kostennorm staat hoe dan ook waarschijnlijk niet op de goede plek (eerst berekening nodig van hele bedrag daar wordt kostennorm over berekend)
3. Normbedragen klopten op meerdere plekken niet. lijken oud ingevuld waar ze naar nieuwe normbedragen zouden moeten verwijzen.
	1. Bijv. woonlasten / huurnorm/kostgangersbedrag moet goed naar worden gekeken
	2. Bedragen in art. 15 URI verwijzen naar wet huurtoeslag maar die koppeling is nu niet gemaakt met die bedragen: nu een shortcut in genomen
4. Reminder: BC = (0.8(inkomsten-uitgaven))/2**in HHNK geval
5. Verschillende typen belastingschulden komen voor: zijn die goed verwerkt nu?
6. logica moet op aantal plekken worden aangepast om voorbeelden verder in te vullen

## Vervolg
- Goed om volgende keer de leidraad te hebben verwerkt te hebben om te zien of we dichter bij kloppende logica kunnen komen
- De HHNK collega's zorgen voor dat spreadsheet met berekening bij ons komt: kan handig startpunt zijn om langs onze eigen logica te houden in scenario's
