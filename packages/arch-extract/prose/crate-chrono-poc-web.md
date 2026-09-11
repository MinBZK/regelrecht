---
node: crate:chrono-poc-web
fingerprint: d06e93de282db18a
---
**Wat.** De HTTP-laag om de chronolexografie-simulator: axum achter de bestaande
OIDC-login, die bij het starten een wereldbestand en een regelingencorpus ophaalt
en per browsersessie één `World` in geheugen houdt, opvraagbaar en bespeelbaar als
JSON.

**Waarom.** De simulator kent geen HTTP, geen sessie en geen casus, en dat hoort zo
te blijven: de opstelling moet in `just test` kunnen draaien zonder server. Wat
daar dan nog aan ontbreekt is dat een mens haar kan bespelen, en dat is precies
wat deze crate toevoegt — elke route is één aanroep op `World`, er is geen
database, en de casus zit in een wereldbestand dat uit de omgeving komt in plaats
van in de code. Twee dingen bepalen de vorm: er is geen totaalbeeld om te bewaren
(een wereld is een gedachte-experiment, geen dossier, dus sessies en werelden staan
in geheugen), en een `World` is niet `Send` (een cel houdt haar engine in een
`RefCell` met een `Rc` erin), dus elke sessie krijgt een eigen thread die haar
wereld bezit in plaats van een slot om een gedeelde map.
